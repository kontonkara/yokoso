//! The config: `config.toml`, as Nix or a hand wrote it, and over it
//! `overrides.toml`, what the settings window changed. Both are watched; a
//! broken one is reported with its line and column, and the last good config
//! stays in force.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use serde::Deserialize;
use yokoso_ui::Color;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub bar: Bar,
    pub notifications: Notifications,
    pub osd: Osd,
    pub probe: Probe,
    pub debug: Debug,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Bar {
    pub position: Edge,
    /// Logical pixels: its height, and how far it floats from the edges.
    pub height: f32,
    pub margin: f32,
    /// The screens it is on, by their description or connector; none for
    /// the main one: the built-in screen, else the first.
    pub screens: Vec<String>,
    /// Its parts, at its start, in its middle and at its end.
    pub left: Vec<Module>,
    pub center: Vec<Module>,
    pub right: Vec<Module>,
    /// With a panel open, the pointer on another part opens that one's.
    pub hover_switch: bool,
    /// How a panel opens: out of its part, or down from the bar's edge.
    pub panels: Panels,
    pub clock: Clock,
    pub battery: Part,
    pub network: Network,
    pub sound: Sound,
    pub privacy: Part,
}

impl Default for Bar {
    fn default() -> Self {
        Self {
            position: Edge::Top,
            height: 40.0,
            margin: 8.0,
            screens: Vec::new(),
            left: vec![Module::Workspaces, Module::Media],
            center: vec![Module::Clock],
            right: vec![
                Module::Privacy,
                Module::Tray,
                Module::Layout,
                Module::Network,
                Module::Sound,
                Module::Battery,
                Module::Notifications,
            ],
            hover_switch: true,
            panels: Panels::default(),
            clock: Clock::default(),
            battery: Part::default(),
            network: Network::default(),
            sound: Sound::default(),
            privacy: Part::default(),
        }
    }
}

/// How a panel opens from the bar.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Panels {
    /// Out of its part's lower edge, as wide as the part at first.
    #[default]
    Grow,
    /// Down from the bar's edge under its part, as a list drops; the part
    /// stays as it is.
    Drop,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    #[default]
    Top,
    Bottom,
}

/// A part of the bar.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Module {
    Workspaces,
    Media,
    Clock,
    Privacy,
    Tray,
    Layout,
    Network,
    Sound,
    Battery,
    /// The bell: how many are new, quiet or not; the history opens from it.
    Notifications,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Clock {
    /// As strftime has it: "%H:%M", "%-I:%M %p".
    #[serde(deserialize_with = "strftime")]
    pub format: String,
    /// The date beside the time.
    pub date: bool,
    #[serde(deserialize_with = "strftime")]
    pub date_format: String,
}

/// A part that opens something elsewhere on a click, until it has a panel
/// of its own: a command, as words.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Part {
    pub on_click: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Network {
    pub on_click: Vec<String>,
    /// Connections that count as a VPN though NetworkManager doesn't call
    /// them one, by name: a tunnel brought up by hand.
    pub vpn: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Sound {
    pub on_click: Vec<String>,
    /// Percent the wheel moves the volume by, a notch.
    pub step: f32,
    /// How loud the wheel goes, percent: past 100 is louder than the
    /// output is made to be.
    pub max: f32,
}

impl Default for Sound {
    fn default() -> Self {
        Self {
            on_click: Vec::new(),
            step: 5.0,
            max: 100.0,
        }
    }
}

fn strftime<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let format = String::deserialize(deserializer)?;
    crate::bar::clock::check(&format).map_err(serde::de::Error::custom)?;
    Ok(format)
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            format: "%H:%M".into(),
            date: false,
            date_format: "%a %-d %b".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Probe {
    /// "#rrggbb".
    #[serde(deserialize_with = "color")]
    pub accent: Color,
    /// Logical pixels.
    pub radius: f32,
}

impl Default for Probe {
    fn default() -> Self {
        Self {
            accent: Color::hex("#f3a17a").expect("a color"),
            radius: 8.0,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Debug {
    /// Frame times and counts over every surface.
    pub overlay: bool,
}

fn color<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Color, D::Error> {
    let text = String::deserialize(deserializer)?;
    Color::hex(&text).ok_or_else(|| serde::de::Error::custom(format!("not a color: {text:?}, want \"#rrggbb\"")))
}

/// Where the config lives.
pub fn dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("yokoso")
}

pub const FILES: [&str; 2] = ["config.toml", "overrides.toml"];

/// Reads the config from `dir`: its files laid one over the next. A missing
/// file counts as empty.
pub fn load(dir: &Path) -> Result<Config, String> {
    let mut merged = toml::Table::new();
    for name in FILES {
        let path = dir.join(name);
        let source = match std::fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        let table: toml::Table = toml::from_str(&source).map_err(|error| located(&path, &source, &error))?;
        // Checked alone too, so its mistakes are found where they are.
        Config::deserialize(toml::Value::Table(table.clone()))
            .map_err(|error| format!("{}: {}", path.display(), error.message()))?;
        lay_over(&mut merged, table);
    }
    Config::deserialize(toml::Value::Table(merged)).map_err(|error| error.message().to_owned())
}

/// Checks one file alone: for `yokoso validate`.
pub fn validate(path: &Path) -> Result<(), String> {
    let source = std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let table: toml::Table = toml::from_str(&source).map_err(|error| located(path, &source, &error))?;
    Config::deserialize(toml::Value::Table(table))
        .map_err(|error| format!("{}: {}", path.display(), error.message()))?;
    Ok(())
}

/// `upper` laid over `lower`: tables merge key by key, anything else is
/// replaced.
fn lay_over(lower: &mut toml::Table, upper: toml::Table) {
    for (key, value) in upper {
        match (lower.get_mut(&key), value) {
            (Some(toml::Value::Table(below)), toml::Value::Table(above)) => lay_over(below, above),
            (_, value) => {
                lower.insert(key, value);
            }
        }
    }
}

/// "path:line:column: message", where the parser says where.
fn located(path: &Path, source: &str, error: &toml::de::Error) -> String {
    let Some(span) = error.span() else {
        return format!("{}: {}", path.display(), error.message());
    };
    let before = &source[..span.start.min(source.len())];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, |last| last.chars().count()) + 1;
    format!("{}:{line}:{column}: {}", path.display(), error.message())
}

/// Watches `dir` and calls `changed` with a fresh load each time one of its
/// files changes, a short while after the last write.
pub fn watch(dir: PathBuf, changed: impl Fn(Result<Config, String>) + Send + 'static) -> Result<(), String> {
    use inotify::{Inotify, WatchMask};
    std::fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let mut inotify = Inotify::init().map_err(|error| error.to_string())?;
    inotify
        .watches()
        .add(
            &dir,
            WatchMask::CLOSE_WRITE | WatchMask::MOVED_TO | WatchMask::CREATE | WatchMask::DELETE,
        )
        .map_err(|error| format!("{}: {error}", dir.display()))?;
    std::thread::Builder::new()
        .name("config".into())
        .spawn(move || {
            let mut buffer = [0u8; 4096];
            loop {
                let Ok(events) = inotify.read_events_blocking(&mut buffer) else {
                    return;
                };
                let ours = events
                    .filter_map(|event| event.name.map(|name| name.to_string_lossy().into_owned()))
                    .any(|name| FILES.contains(&name.as_str()));
                if !ours {
                    continue;
                }
                // Editors write in several steps: wait for the last one.
                std::thread::sleep(Duration::from_millis(100));
                changed(load(&dir));
            }
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_with(test: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("yokoso-config-{}-{test}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, source) in files {
            std::fs::write(dir.join(name), source).unwrap();
        }
        dir
    }

    #[test]
    fn no_files_is_the_defaults() {
        let dir = dir_with("none", &[]);
        assert_eq!(load(&dir).unwrap(), Config::default());
    }

    #[test]
    fn overrides_lie_over_the_config_key_by_key() {
        let dir = dir_with(
            "over",
            &[
                ("config.toml", "[probe]\naccent = \"#ff0000\"\nradius = 4\n"),
                ("overrides.toml", "[probe]\nradius = 12\n"),
            ],
        );
        let config = load(&dir).unwrap();
        assert_eq!(config.probe.accent, Color::hex("#ff0000").unwrap());
        assert_eq!(config.probe.radius, 12.0);
    }

    #[test]
    fn the_bar_takes_its_parts_by_name_and_names_one_it_does_not_know() {
        let dir = dir_with(
            "bar",
            &[(
                "config.toml",
                "[bar]\nposition = \"bottom\"\nleft = [\"clock\"]\n[bar.clock]\nformat = \"%H:%M:%S\"\n",
            )],
        );
        let bar = load(&dir).unwrap().bar;
        assert_eq!(bar.position, Edge::Bottom);
        assert_eq!(bar.left, vec![Module::Clock]);
        assert_eq!(bar.clock.format, "%H:%M:%S");
        assert_eq!(bar.right, Bar::default().right, "the rest stays as it was");
        let dir = dir_with("bar-typo", &[("config.toml", "[bar]\nright = [\"clok\"]\n")]);
        assert!(load(&dir).unwrap_err().contains("clok"));
        let dir = dir_with("bar-format", &[("config.toml", "[bar.clock]\nformat = \"%H:%Q\"\n")]);
        assert!(load(&dir).unwrap_err().contains("strftime"));
    }

    #[test]
    fn a_broken_file_says_where() {
        let dir = dir_with("broken", &[("config.toml", "[probe]\nradius = = 4\n")]);
        let error = load(&dir).unwrap_err();
        assert!(error.contains("config.toml:2:"), "{error}");
    }

    #[test]
    fn a_typo_and_a_bad_color_are_named() {
        let dir = dir_with("typo", &[("config.toml", "[probe]\nradios = 4\n")]);
        assert!(load(&dir).unwrap_err().contains("radios"));
        let dir = dir_with("color", &[("config.toml", "[probe]\naccent = \"red\"\n")]);
        assert!(load(&dir).unwrap_err().contains("not a color"));
    }
}

/// Notifications: their toasts, their history, quiet times and rules.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Notifications {
    /// How many toasts show at once; more wait their turn.
    pub visible: usize,
    /// Seconds a toast shows; a low one, a critical one (0: until closed).
    pub timeout: f32,
    pub timeout_low: f32,
    pub timeout_critical: f32,
    /// Critical ones over full-screen windows too.
    pub over_fullscreen: bool,
    /// How many the history keeps.
    pub history: usize,
    /// Quiet from and until, "23:00" and "08:00"; none for no such time.
    #[serde(deserialize_with = "time_of_day")]
    pub quiet_from: String,
    #[serde(deserialize_with = "time_of_day")]
    pub quiet_until: String,
    /// Quiet while the screen is shared.
    pub quiet_sharing: bool,
    pub rules: Vec<Rule>,
}

impl Default for Notifications {
    fn default() -> Self {
        Self {
            visible: 3,
            timeout: 5.0,
            timeout_low: 3.0,
            timeout_critical: 10.0,
            over_fullscreen: true,
            history: 100,
            quiet_from: String::new(),
            quiet_until: String::new(),
            quiet_sharing: false,
            rules: Vec::new(),
        }
    }
}

/// What a rule does with the notifications it takes.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Act {
    /// Gone, not kept.
    Hide,
    /// Kept, never a toast.
    #[default]
    Quiet,
    /// A toast even while quiet.
    Through,
    /// Its command runs, and it goes on as any other.
    Run,
}

/// A rule: notifications from an app, with some words in them, or both.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Rule {
    /// The app's name or its desktop entry, any case.
    pub app: String,
    /// Words in its summary or body, any case.
    pub text: String,
    pub action: Act,
    /// For `run`: a command, as words; the notification's summary and body
    /// in YOKOSO_SUMMARY and YOKOSO_BODY.
    pub run: Vec<String>,
}

fn time_of_day<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let time = String::deserialize(deserializer)?;
    if !time.is_empty() && chrono::NaiveTime::parse_from_str(&time, "%H:%M").is_err() {
        return Err(serde::de::Error::custom(format!(
            "{time:?} is not a time of day, as \"23:00\""
        )));
    }
    Ok(time)
}

/// The OSD: what changed, a moment at the bottom of the main screen.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Osd {
    /// Seconds it stays after the last change.
    pub duration: f32,
    /// What it doesn't show: volume, mic, output, brightness, layout.
    pub hidden: Vec<String>,
}

impl Default for Osd {
    fn default() -> Self {
        Self {
            duration: 1.5,
            hidden: Vec::new(),
        }
    }
}
