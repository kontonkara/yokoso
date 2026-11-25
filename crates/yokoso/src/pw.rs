//! PipeWire: the default output's volume, and what records. Read from
//! `pw-dump --monitor`, part of PipeWire itself, which says each change as it
//! happens; set through `wpctl`, so WirePlumber's routes are kept. Nothing is
//! asked on a timer.
//!
//! `YOKOSO_PW_DUMP` and `YOKOSO_WPCTL` name other programs in their place:
//! the stand's, which never touch a real PipeWire.

use std::{
    collections::HashMap,
    io::BufReader,
    process::{Command, Stdio},
    time::Duration,
};

use serde_json::Value;

/// The default output, as PipeWire has it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Output {
    pub name: String,
    /// What it is called for people.
    pub description: String,
    /// 1 is 100%; louder past it.
    pub volume: f32,
    pub muted: bool,
}

/// What PipeWire says: its output, or that it isn't there.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Sound {
    #[default]
    Unknown,
    /// No output at all: nothing to play to.
    Silent,
    Out(Output),
}

/// Everything `pw-dump` has said so far, by object.
#[derive(Default)]
pub struct Dump {
    objects: HashMap<u64, Value>,
    /// The default metadata: its keys and their values.
    defaults: HashMap<String, Value>,
}

impl Dump {
    /// Takes in one of its batches: objects new or changed, and gone ones
    /// as their id with no info.
    pub fn take(&mut self, batch: Vec<Value>) {
        for object in batch {
            let Some(id) = object.get("id").and_then(Value::as_u64) else {
                continue;
            };
            if object.get("info").is_some_and(Value::is_null) && object.get("metadata").is_none() {
                self.objects.remove(&id);
                continue;
            }
            let default_metadata = object.get("type").and_then(Value::as_str) == Some("PipeWire:Interface:Metadata")
                && object.pointer("/props/metadata.name").and_then(Value::as_str) == Some("default");
            if default_metadata && let Some(entries) = object.get("metadata").and_then(Value::as_array) {
                for entry in entries {
                    let Some(key) = entry.get("key").and_then(Value::as_str) else {
                        continue;
                    };
                    match entry.get("value") {
                        Some(Value::Null) | None => {
                            self.defaults.remove(key);
                        }
                        Some(value) => {
                            self.defaults.insert(key.to_owned(), value.clone());
                        }
                    }
                }
            }
            self.objects.insert(id, object);
        }
    }

    fn nodes(&self) -> impl Iterator<Item = &Value> {
        self.objects
            .values()
            .filter(|object| object.get("type").and_then(Value::as_str) == Some("PipeWire:Interface:Node"))
    }

    /// The default output now.
    pub fn sound(&self) -> Sound {
        let sinks: Vec<&Value> = self
            .nodes()
            .filter(|node| node.pointer("/info/props/media.class").and_then(Value::as_str) == Some("Audio/Sink"))
            .collect();
        let wanted = self
            .defaults
            .get("default.audio.sink")
            .and_then(|value| value.get("name"))
            .and_then(Value::as_str);
        fn name_of<'a>(node: &&'a Value) -> Option<&'a str> {
            node.pointer("/info/props/node.name").and_then(Value::as_str)
        }
        let Some(sink) = sinks.iter().find(|node| name_of(node) == wanted).or(sinks.first()) else {
            return Sound::Silent;
        };
        let props = sink.pointer("/info/params/Props/0");
        let volumes: Vec<f32> = props
            .and_then(|props| props.get("channelVolumes"))
            .and_then(Value::as_array)
            .map(|all| all.iter().filter_map(Value::as_f64).map(|v| v as f32).collect())
            .unwrap_or_default();
        // As people hear it: the cube root of the linear factor, as wpctl
        // shows it.
        let volume = volumes.iter().copied().fold(0.0f32, f32::max).cbrt();
        let muted = props
            .and_then(|props| props.get("mute"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        Sound::Out(Output {
            name: name_of(sink).unwrap_or_default().to_owned(),
            description: sink
                .pointer("/info/props/node.description")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            volume: (volume * 100.0).round() / 100.0,
            muted,
        })
    }
}

fn program(variable: &str, default: &str) -> String {
    std::env::var(variable)
        .ok()
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| default.to_owned())
}

/// Watches PipeWire on a thread of its own. When `pw-dump` ends or isn't
/// there, it says it doesn't know, and starts it again a while later.
pub fn listen(send: impl Fn(Sound) -> bool + Send + 'static) {
    let spawned = std::thread::Builder::new().name("pipewire".into()).spawn(move || {
        let mut said = false;
        loop {
            let child = Command::new(program("YOKOSO_PW_DUMP", "pw-dump"))
                .args(["--monitor", "--no-colors"])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();
            match child {
                Ok(mut child) => {
                    let mut dump = Dump::default();
                    let mut last = None;
                    if let Some(out) = child.stdout.take() {
                        let batches =
                            serde_json::Deserializer::from_reader(BufReader::new(out)).into_iter::<Vec<Value>>();
                        for batch in batches {
                            let Ok(batch) = batch else { break };
                            dump.take(batch);
                            let sound = dump.sound();
                            if last.as_ref() != Some(&sound) {
                                if !send(sound.clone()) {
                                    let _ = child.kill();
                                    let _ = child.wait();
                                    return;
                                }
                                last = Some(sound);
                            }
                        }
                    }
                    let _ = child.kill();
                    let _ = child.wait();
                }
                Err(error) if !said => {
                    tracing::warn!(%error, "no pw-dump: no sound");
                    said = true;
                }
                Err(_) => {}
            }
            if !send(Sound::Unknown) {
                return;
            }
            std::thread::sleep(Duration::from_secs(5));
        }
    });
    if let Err(error) = spawned {
        tracing::warn!(%error, "no thread for PipeWire");
    }
}

/// Runs `wpctl` with `args`, not waiting on it.
fn wpctl(args: Vec<String>) {
    let _ = std::thread::Builder::new().name("wpctl".into()).spawn(move || {
        match Command::new(program("YOKOSO_WPCTL", "wpctl"))
            .args(&args)
            .stdin(Stdio::null())
            .status()
        {
            Ok(status) if !status.success() => tracing::warn!(?args, %status, "wpctl refused"),
            Ok(_) => {}
            Err(error) => tracing::warn!(%error, "no wpctl"),
        }
    });
}

/// The default output `percent` louder, or quieter for less than none, up
/// to `max` percent.
pub fn change_volume(percent: f32, max: f32) {
    let sign = if percent >= 0.0 { "+" } else { "-" };
    wpctl(vec![
        "set-volume".into(),
        "-l".into(),
        format!("{:.2}", max / 100.0),
        "@DEFAULT_AUDIO_SINK@".into(),
        format!("{}%{sign}", percent.abs().round()),
    ]);
}

/// The default output muted, or heard again.
pub fn toggle_mute() {
    wpctl(vec!["set-mute".into(), "@DEFAULT_AUDIO_SINK@".into(), "toggle".into()]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sink(id: u64, name: &str, linear: f64, mute: bool) -> Value {
        json!({
            "id": id,
            "type": "PipeWire:Interface:Node",
            "info": {
                "props": { "media.class": "Audio/Sink", "node.name": name, "node.description": format!("{name} out") },
                "params": { "Props": [ { "channelVolumes": [linear, linear], "mute": mute }, {} ] }
            }
        })
    }

    fn default_sink(name: &str) -> Value {
        json!({
            "id": 40,
            "type": "PipeWire:Interface:Metadata",
            "props": { "metadata.name": "default" },
            "metadata": [ { "subject": 0, "key": "default.audio.sink", "type": "Spa:String:JSON", "value": { "name": name } } ]
        })
    }

    #[test]
    fn the_default_output_is_heard_as_wpctl_says_it() {
        let mut dump = Dump::default();
        assert_eq!(dump.sound(), Sound::Silent);
        dump.take(vec![
            sink(65, "speakers", 0.125, false),
            sink(70, "phones", 1.0, false),
            default_sink("speakers"),
        ]);
        let Sound::Out(out) = dump.sound() else { panic!() };
        assert_eq!((out.name.as_str(), out.volume, out.muted), ("speakers", 0.5, false));
        assert_eq!(out.description, "speakers out");
        // Muted and louder, then the default moves to the headphones.
        dump.take(vec![sink(65, "speakers", 1.728, true)]);
        assert!(matches!(
            dump.sound(),
            Sound::Out(Output {
                volume: 1.2,
                muted: true,
                ..
            })
        ));
        dump.take(vec![default_sink("phones")]);
        assert!(matches!(dump.sound(), Sound::Out(Output { volume: 1.0, .. })));
        // The headphones gone: whatever is left.
        dump.take(vec![json!({ "id": 70, "info": null })]);
        let Sound::Out(out) = dump.sound() else { panic!() };
        assert_eq!(out.name, "speakers");
    }
}
