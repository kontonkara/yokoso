//! The bell on the bar and the history that opens from it. The bell says how
//! many came since the history was last looked at, and whether it is quiet;
//! the middle button turns quiet on and off. The history is a list of rows
//! one height each: an app's row heads its notifications, newest first, and
//! folds them away on a click; the apps in the order of their newest.

use std::collections::HashSet;

use yokoso_ui::{
    el::{Align, ButtonStyle, Edges, El, Ink, Size, Type},
    theme::Theme,
};

use super::Msg;
use crate::notify::Notification;

pub const WIDTH: f32 = 380.0;
const ROW: f32 = 52.0;
const HEAD: f32 = 52.0;
const PAD: f32 = 10.0;
/// Rows shown at once; more scroll.
const SHOWN: usize = 8;

/// A row of the history.
#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    /// An app's row: its name, how many it has, and whether they are folded.
    App { app: String, count: usize, folded: bool },
    /// A notification, by its place in the list.
    Note(usize),
}

/// The rows for `list` (oldest first), the apps in `folded` folded away.
pub fn rows(list: &[Notification], folded: &HashSet<String>) -> Vec<Row> {
    let mut apps: Vec<&str> = Vec::new();
    for note in list.iter().rev() {
        if !apps.contains(&note.app.as_str()) {
            apps.push(&note.app);
        }
    }
    let mut rows = Vec::new();
    for app in apps {
        let mine: Vec<usize> = (0..list.len()).rev().filter(|n| list[*n].app == app).collect();
        let fold = folded.contains(app);
        rows.push(Row::App {
            app: app.to_owned(),
            count: mine.len(),
            folded: fold,
        });
        if !fold {
            rows.extend(mine.into_iter().map(Row::Note));
        }
    }
    rows
}

/// The panel's size for `rows` rows.
pub fn size(rows: usize) -> (f32, f32) {
    let shown = rows.clamp(1, SHOWN) as f32;
    (WIDTH, HEAD + shown * ROW + 2.0 * PAD)
}

/// The bell: crossed out while quiet, faint while another server shows
/// notifications; the number of new ones beside it.
pub fn bell(unread: usize, total: usize, quiet: bool, serving: bool, open: bool, theme: &Theme, pad: Edges) -> El<Msg> {
    let faint = Ink::Own(theme.colors.on_surface.alpha(0.38));
    let icon = El::icon(if quiet { "bell-off" } else { "bell" });
    let mut parts = vec![if serving { icon } else { icon.ink(faint) }];
    if unread > 0 && serving {
        parts.push(
            El::text(unread.min(99).to_string(), Type::Label)
                .weight(700.0)
                .tabular()
                .ink(Ink::Primary),
        );
    }
    let about = if !serving {
        "Another server shows notifications".to_owned()
    } else {
        let count = match total {
            0 => "No notifications".to_owned(),
            1 => "1 notification".to_owned(),
            n => format!("{n} notifications"),
        };
        if quiet {
            format!("{count} \u{b7} Do not disturb")
        } else {
            count
        }
    };
    El::press(4.0, parts, Some(Msg::History))
        .on_middle(Msg::Quiet)
        .selected(open)
        .key("notifications")
        .padding(pad)
        .height(Size::Fill(1.0))
        .tooltip(about)
}

/// "14:05" today, "Mon 14:05" within a week, "3 Nov" before.
fn when(at: i64) -> String {
    use chrono::{Local, TimeZone};
    let Some(then) = Local.timestamp_opt(at, 0).single() else {
        return String::new();
    };
    let now = Local::now();
    let days = (now.date_naive() - then.date_naive()).num_days();
    match days {
        0 => then.format("%H:%M").to_string(),
        1..=6 => then.format("%a %H:%M").to_string(),
        _ => then.format("%-d %b").to_string(),
    }
}

/// The panel: its head with the quiet switch and a way to clear all, then
/// the rows, or a word that there are none.
pub fn view(list: &[Notification], rows: &[Row], quiet: bool, theme: &Theme) -> El<Msg> {
    let faint = Ink::Own(theme.colors.on_surface.alpha(0.6));
    let head = El::row(
        8.0,
        vec![
            El::text("Notifications", Type::Title)
                .weight(600.0)
                .width(Size::Fill(1.0)),
            El::text("Do not disturb", Type::Label).ink(faint),
            El::toggle(quiet, false, Some(Msg::Quiet)).key("quiet"),
            El::button(
                "Clear",
                ButtonStyle::Text,
                (!list.is_empty()).then_some(Msg::HistoryClear(None)),
            )
            .key("clear"),
        ],
    )
    .align(Align::Center)
    .width(Size::Fill(1.0))
    .height(Size::Fixed(HEAD));
    let body = if rows.is_empty() {
        El::row(
            0.0,
            vec![
                El::spacer(),
                El::text("No notifications", Type::Body).ink(faint),
                El::spacer(),
            ],
        )
        .align(Align::Center)
        .width(Size::Fill(1.0))
        .height(Size::Fixed(ROW))
    } else {
        let made: Vec<RowView> = rows
            .iter()
            .map(|row| match row {
                Row::App { app, count, folded } => RowView::App {
                    app: app.clone(),
                    count: *count,
                    folded: *folded,
                },
                Row::Note(n) => {
                    let note = &list[*n];
                    RowView::Note {
                        id: note.id,
                        summary: note.summary.clone(),
                        body: note.body.replace('\n', " "),
                        when: when(note.at),
                        opens: note.default_action,
                    }
                }
            })
            .collect();
        let faint = theme.colors.on_surface.alpha(0.6);
        let shown = rows.len().min(SHOWN) as f32 * ROW;
        // Rows known by what they show: an app by its name, a notification
        // by its id; so they glide, come in and fade as the history changes.
        let keys: Vec<u64> = made.iter().map(RowView::key).collect();
        El::list(made.len(), ROW, move |n| made[n].el(Ink::Own(faint)))
            .keyed(move |n| keys[n])
            .width(Size::Fill(1.0))
            .height(Size::Fixed(shown))
    };
    El::column(0.0, vec![head, body])
        .padding(Edges {
            left: PAD + 4.0,
            right: PAD,
            top: PAD,
            bottom: PAD,
        })
        .fill()
}

/// What a row shows, taken out of the notifications for the list to build
/// rows from as they come into view.
enum RowView {
    App {
        app: String,
        count: usize,
        folded: bool,
    },
    Note {
        id: u32,
        summary: String,
        body: String,
        when: String,
        opens: bool,
    },
}

impl RowView {
    /// What the row is known by in the list.
    fn key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        match self {
            Self::App { app, .. } => ("app", app).hash(&mut hasher),
            Self::Note { id, .. } => ("note", id).hash(&mut hasher),
        }
        hasher.finish()
    }

    fn el(&self, faint: Ink) -> El<Msg> {
        let close = |message: Msg, key: String| {
            El::press(0.0, vec![El::icon("close").ink(faint)], Some(message))
                .key(key)
                .padding(Edges::all(4.0))
        };
        match self {
            Self::App { app, count, folded } => {
                let name = if app.is_empty() { "Unnamed" } else { app.as_str() };
                El::press(
                    8.0,
                    vec![
                        El::icon(if *folded { "chevron-right" } else { "chevron-down" }).ink(faint),
                        El::text(name, Type::Body).weight(600.0).ellipsis(),
                        El::text(count.to_string(), Type::Label).ink(faint).tabular(),
                        El::spacer().width(Size::Fill(1.0)),
                        close(Msg::HistoryClear(Some(app.clone())), format!("clear {app}")),
                    ],
                    Some(Msg::HistoryFold(app.clone())),
                )
                .align(Align::Center)
                .key(("app", app.as_str()))
                .width(Size::Fill(1.0))
                .height(Size::Fixed(ROW))
            }
            Self::Note {
                id,
                summary,
                body,
                when,
                opens,
            } => {
                let mut words = vec![El::text(summary.clone(), Type::Body).weight(600.0).ellipsis()];
                if !body.is_empty() {
                    words.push(El::text(body.clone(), Type::Label).ink(faint).ellipsis());
                }
                El::press(
                    8.0,
                    vec![
                        El::column(2.0, words).width(Size::Fill(1.0)),
                        El::text(when.clone(), Type::Small).ink(faint),
                        close(Msg::HistoryRemove(*id), format!("remove {id}")),
                    ],
                    opens.then_some(Msg::HistoryOpen(*id)),
                )
                .align(Align::Center)
                .key(("note", *id))
                .padding(Edges {
                    left: 24.0,
                    ..Edges::default()
                })
                .width(Size::Fill(1.0))
                .height(Size::Fixed(ROW))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(id: u32, app: &str) -> Notification {
        Notification {
            id,
            app: app.into(),
            ..Notification::default()
        }
    }

    #[test]
    fn apps_in_the_order_of_their_newest_each_newest_first_and_folded_away() {
        let list = [note(1, "Mail"), note(2, "Chat"), note(3, "Mail"), note(4, "Chat")];
        let rows = rows(&list, &HashSet::new());
        assert_eq!(
            rows,
            [
                Row::App {
                    app: "Chat".into(),
                    count: 2,
                    folded: false
                },
                Row::Note(3),
                Row::Note(1),
                Row::App {
                    app: "Mail".into(),
                    count: 2,
                    folded: false
                },
                Row::Note(2),
                Row::Note(0),
            ]
        );
        let folded = HashSet::from(["Chat".to_owned()]);
        assert_eq!(super::rows(&list, &folded).len(), 4, "Chat's two folded away");
        assert_eq!(size(0).1, size(1).1, "room for a word when there are none");
        assert_eq!(size(30), size(SHOWN), "more scroll");
    }
}
