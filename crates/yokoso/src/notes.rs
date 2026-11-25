//! What the shell keeps of notifications: the history, newest last, kept on
//! disk for its owner alone; whether it is quiet; and what a new one comes to,
//! by the rules and the quiet times: a toast, the history alone, or nothing.

use std::path::PathBuf;

use crate::{
    config::{self, Act},
    notify::{Notification, Urgency},
};

/// What becomes of a new notification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fate {
    /// A toast, and the history.
    Toast,
    /// The history alone.
    Quiet,
    /// Nothing at all.
    Gone,
}

/// What a notification comes to, by `rules` and whether it is `quiet` now;
/// and the command a rule runs for it, if one does.
pub fn fate(notification: &Notification, rules: &[config::Rule], quiet: bool) -> (Fate, Option<Vec<String>>) {
    let lower = |text: &str| text.to_lowercase();
    let rule = rules.iter().find(|rule| {
        let app = rule.app.is_empty()
            || lower(&rule.app) == lower(&notification.app)
            || lower(&rule.app) == lower(&notification.desktop);
        let words = lower(&rule.text);
        let text = rule.text.is_empty()
            || lower(&notification.summary).contains(&words)
            || lower(&notification.body).contains(&words);
        (!rule.app.is_empty() || !rule.text.is_empty()) && app && text
    });
    let critical = notification.urgency == Urgency::Critical;
    let run = rule.filter(|rule| rule.action == Act::Run).map(|rule| rule.run.clone());
    let fate = match rule.map(|rule| rule.action) {
        Some(Act::Hide) => Fate::Gone,
        Some(Act::Quiet) => Fate::Quiet,
        Some(Act::Through) => Fate::Toast,
        _ if quiet && !critical => Fate::Quiet,
        _ => Fate::Toast,
    };
    (fate, run)
}

/// Whether `now` ("HH:MM") is within `from` and `until`, across midnight
/// when `until` comes first. Not when either is empty.
pub fn within(now: &str, from: &str, until: &str) -> bool {
    if from.is_empty() || until.is_empty() {
        return false;
    }
    if from <= until {
        from <= now && now < until
    } else {
        now >= from || now < until
    }
}

/// One kept on disk: what can be shown again after a restart. Pictures are
/// found again by name; actions are gone with the app that heard them.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Kept {
    pub app: String,
    #[serde(default)]
    pub desktop: String,
    pub summary: String,
    pub body: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub urgency: Urgency,
    pub at: i64,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct File {
    #[serde(default)]
    quiet: bool,
    #[serde(default)]
    kept: Vec<Kept>,
}

/// The history and the quiet switch.
#[derive(Default)]
pub struct Store {
    /// Newest last.
    pub list: Vec<Notification>,
    /// Come since the history was last looked at.
    pub unread: usize,
    /// "Do not disturb", by hand.
    pub quiet: bool,
    path: Option<PathBuf>,
}

impl Store {
    /// The store kept in `path`, as it was left; empty where there is none.
    pub fn open(path: PathBuf) -> Self {
        let file: File = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        let list = file
            .kept
            .into_iter()
            .enumerate()
            .map(|(n, kept)| Notification {
                // Ids of their own, below any the server gives.
                id: u32::MAX - n as u32,
                app: kept.app,
                desktop: kept.desktop,
                summary: kept.summary,
                body: kept.body,
                icon_name: kept.icon,
                urgency: kept.urgency,
                at: kept.at,
                ..Notification::default()
            })
            .collect();
        Self {
            list,
            unread: 0,
            quiet: file.quiet,
            path: Some(path),
        }
    }

    /// Where it is kept: `$XDG_STATE_HOME/yokoso/notifications.json`.
    pub fn path() -> PathBuf {
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        state.join("yokoso/notifications.json")
    }

    /// Takes `notification` in, in place of the one it replaces, the oldest
    /// going past `most`. A transient one is not kept.
    pub fn add(&mut self, notification: Notification, most: usize) {
        if let Some(old) = self.list.iter_mut().find(|old| old.id == notification.id) {
            *old = notification;
        } else if !notification.transient {
            self.list.push(notification);
            self.unread += 1;
        }
        let over = self.list.len().saturating_sub(most);
        self.list.drain(..over);
        self.save();
    }

    pub fn remove(&mut self, id: u32) -> Option<Notification> {
        let at = self.list.iter().position(|n| n.id == id)?;
        let gone = self.list.remove(at);
        self.save();
        Some(gone)
    }

    /// All of an app's, or every one for none.
    pub fn clear(&mut self, app: Option<&str>) -> Vec<u32> {
        let (gone, kept): (Vec<_>, Vec<_>) = self.list.drain(..).partition(|n| app.is_none_or(|app| n.app == app));
        self.list = kept;
        self.save();
        gone.into_iter().map(|n| n.id).collect()
    }

    pub fn set_quiet(&mut self, quiet: bool) {
        self.quiet = quiet;
        self.save();
    }

    /// Onto disk, readable by its owner alone; a failure said once in the log
    /// and otherwise let be: the shell goes on without.
    fn save(&self) {
        let Some(path) = &self.path else { return };
        let file = File {
            quiet: self.quiet,
            kept: self
                .list
                .iter()
                .map(|n| Kept {
                    app: n.app.clone(),
                    desktop: n.desktop.clone(),
                    summary: n.summary.clone(),
                    body: n.body.clone(),
                    icon: n.icon_name.clone(),
                    urgency: n.urgency,
                    at: n.at,
                })
                .collect(),
        };
        let write = || -> std::io::Result<()> {
            use std::{io::Write, os::unix::fs::OpenOptionsExt};
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let fresh = path.with_extension("json.new");
            let mut out = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&fresh)?;
            out.write_all(serde_json::to_string(&file).unwrap_or_default().as_bytes())?;
            std::fs::rename(&fresh, path)
        };
        if let Err(error) = write() {
            tracing::warn!(%error, path = %path.display(), "notifications not kept on disk");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(id: u32, app: &str, summary: &str) -> Notification {
        Notification {
            id,
            app: app.into(),
            summary: summary.into(),
            ..Notification::default()
        }
    }

    fn rule(app: &str, text: &str, action: Act) -> config::Rule {
        config::Rule {
            app: app.into(),
            text: text.into(),
            action,
            run: vec!["true".into()],
        }
    }

    #[test]
    fn rules_and_quiet_decide_what_becomes_of_one() {
        let rules = [
            rule("Chat", "", Act::Quiet),
            rule("", "build failed", Act::Through),
            rule("spam", "", Act::Hide),
            rule("Mail", "", Act::Run),
        ];
        assert_eq!(
            fate(&note(1, "chat", "hi"), &rules, false).0,
            Fate::Quiet,
            "by app, any case"
        );
        assert_eq!(
            fate(&note(1, "CI", "Build FAILED"), &rules, true).0,
            Fate::Toast,
            "through quiet"
        );
        assert_eq!(fate(&note(1, "Spam", "x"), &rules, false).0, Fate::Gone);
        let (to, run) = fate(&note(1, "Mail", "x"), &rules, false);
        assert_eq!((to, run), (Fate::Toast, Some(vec!["true".to_owned()])));
        assert_eq!(fate(&note(1, "Other", "x"), &rules, true).0, Fate::Quiet, "quiet");
        let mut critical = note(1, "Other", "x");
        critical.urgency = Urgency::Critical;
        assert_eq!(fate(&critical, &rules, true).0, Fate::Toast, "critical through quiet");
        assert_eq!(
            fate(&note(1, "Other", "x"), &[rule("", "", Act::Hide)], false).0,
            Fate::Toast,
            "an empty rule takes none"
        );
    }

    #[test]
    fn quiet_hours_go_across_midnight() {
        assert!(within("23:30", "23:00", "08:00"));
        assert!(within("07:59", "23:00", "08:00"));
        assert!(!within("08:00", "23:00", "08:00"));
        assert!(within("13:00", "12:00", "14:00"));
        assert!(!within("13:00", "", "14:00"));
    }

    #[test]
    fn the_history_is_kept_on_disk_for_its_owner_and_comes_back() {
        let dir = std::env::temp_dir().join(format!("yokoso-notes-{}", std::process::id()));
        let path = dir.join("notifications.json");
        let mut store = Store::open(path.clone());
        store.add(note(1, "A", "one"), 2);
        store.add(note(2, "B", "two"), 2);
        store.add(note(3, "A", "three"), 2);
        assert_eq!(
            store.list.iter().map(|n| n.id).collect::<Vec<_>>(),
            [2, 3],
            "the oldest goes past two"
        );
        store.add(note(3, "A", "three again"), 2);
        assert_eq!(store.list[1].summary, "three again", "replaced in place");
        assert_eq!(store.unread, 3);
        let mut transient = note(4, "A", "gone");
        transient.transient = true;
        store.add(transient, 2);
        assert_eq!(store.list.len(), 2, "a transient one is not kept");
        store.set_quiet(true);
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        let again = Store::open(path.clone());
        assert!(again.quiet);
        assert_eq!(
            again.list.iter().map(|n| n.summary.as_str()).collect::<Vec<_>>(),
            ["two", "three again"]
        );
        let mut again = again;
        assert_eq!(again.clear(Some("A")).len(), 1);
        assert_eq!(again.list.len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}
