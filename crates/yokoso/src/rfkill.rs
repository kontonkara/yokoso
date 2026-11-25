//! The radios' switches, from `/dev/rfkill`: airplane mode, as the system
//! has it, is every radio blocked, by a key or a switch or in software. The
//! kernel tells each radio as it is opened and each change after; nothing is
//! asked on a timer.

use std::{collections::HashMap, io::Read, time::Duration};

/// One event, as the kernel's `struct rfkill_event` has it: the radio's
/// index, its kind, what happened, and whether software or hardware blocks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub idx: u32,
    pub op: u8,
    pub soft: bool,
    pub hard: bool,
}

/// The size of an event as first made: what a read asks for, whatever the
/// kernel adds after it.
const SIZE: usize = 8;
const ADD: u8 = 0;
const DEL: u8 = 1;
const CHANGE: u8 = 2;

impl Event {
    pub fn parse(bytes: &[u8; SIZE]) -> Self {
        Self {
            idx: u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            op: bytes[5],
            soft: bytes[6] != 0,
            hard: bytes[7] != 0,
        }
    }
}

/// The radios as the events so far say: each blocked or not.
#[derive(Default)]
pub struct Radios {
    blocked: HashMap<u32, bool>,
}

impl Radios {
    pub fn take(&mut self, event: Event) {
        match event.op {
            ADD | CHANGE => {
                self.blocked.insert(event.idx, event.soft || event.hard);
            }
            DEL => {
                self.blocked.remove(&event.idx);
            }
            _ => {}
        }
    }

    /// Airplane mode: radios there are, and every one blocked.
    pub fn airplane(&self) -> bool {
        !self.blocked.is_empty() && self.blocked.values().all(|blocked| *blocked)
    }
}

/// Watches the radios on a thread of its own: `send` hears whether airplane
/// mode is on, at first and on each change. Where `/dev/rfkill` can't be
/// read, it is off, and tried again a while later. YOKOSO_RFKILL names
/// another file to read, for the stand.
pub fn listen(send: impl Fn(bool) -> bool + Send + 'static) {
    let path = std::env::var("YOKOSO_RFKILL").unwrap_or_else(|_| "/dev/rfkill".into());
    let spawned = std::thread::Builder::new().name("rfkill".into()).spawn(move || {
        let mut said = false;
        loop {
            match std::fs::File::open(&path) {
                Ok(mut file) => {
                    let mut radios = Radios::default();
                    let mut last = None;
                    let mut bytes = [0u8; SIZE];
                    while file.read_exact(&mut bytes).is_ok() {
                        radios.take(Event::parse(&bytes));
                        let now = radios.airplane();
                        if last != Some(now) {
                            if !send(now) {
                                return;
                            }
                            last = Some(now);
                        }
                    }
                }
                Err(error) if !said => {
                    tracing::info!(%error, path, "no radio switches: no airplane mode");
                    said = true;
                }
                Err(_) => {}
            }
            if !send(false) {
                return;
            }
            std::thread::sleep(Duration::from_secs(5));
        }
    });
    if let Err(error) = spawned {
        tracing::warn!(%error, "no thread for the radio switches");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(idx: u32, op: u8, soft: bool, hard: bool) -> Event {
        let mut bytes = [0u8; SIZE];
        bytes[..4].copy_from_slice(&idx.to_ne_bytes());
        bytes[4] = 1;
        bytes[5] = op;
        bytes[6] = u8::from(soft);
        bytes[7] = u8::from(hard);
        Event::parse(&bytes)
    }

    #[test]
    fn airplane_mode_is_every_radio_blocked() {
        let mut radios = Radios::default();
        assert!(!radios.airplane(), "no radios at all");
        radios.take(event(0, ADD, false, false));
        radios.take(event(1, ADD, true, false));
        assert!(!radios.airplane(), "Wi-Fi on, Bluetooth off");
        radios.take(event(0, CHANGE, true, false));
        assert!(radios.airplane(), "both blocked in software");
        radios.take(event(0, CHANGE, false, true));
        assert!(radios.airplane(), "one by a switch");
        radios.take(event(1, DEL, false, false));
        radios.take(event(0, CHANGE, false, false));
        assert!(!radios.airplane());
    }
}
