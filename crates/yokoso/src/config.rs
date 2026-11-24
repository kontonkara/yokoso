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
    pub probe: Probe,
    pub debug: Debug,
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
