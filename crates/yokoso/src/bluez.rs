//! BlueZ: whether Bluetooth is on, from the system bus. With Wi-Fi off too,
//! that is airplane mode, as it is turned on elsewhere: both radios off.
//! Read once, then again on each change BlueZ signals; never on a timer.

use std::time::Duration;

use futures_util::{FutureExt, StreamExt};
use zbus::{Connection, MatchRule, MessageStream, fdo::ObjectManagerProxy};

const BLUEZ: &str = "org.bluez";
const ADAPTER: &str = "org.bluez.Adapter1";

/// What BlueZ says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Bluetooth {
    /// No word from it: not there, or not yet.
    #[default]
    Unknown,
    /// Its adapters, and whether any of them is on.
    Known { adapters: usize, on: bool },
}

impl Bluetooth {
    /// Off with adapters to be on: what airplane mode asks of it.
    pub fn off(self) -> bool {
        matches!(self, Self::Known { adapters, on: false } if adapters > 0)
    }
}

async fn read(bus: &Connection) -> zbus::Result<Bluetooth> {
    let manager = ObjectManagerProxy::builder(bus)
        .destination(BLUEZ)
        .path("/")
        .build()
        .await?;
    let objects = manager.get_managed_objects().await?;
    let mut adapters = 0;
    let mut on = false;
    for interfaces in objects.values() {
        if let Some(adapter) = interfaces.get(ADAPTER) {
            adapters += 1;
            on |= adapter
                .get("Powered")
                .and_then(|value| bool::try_from(value).ok())
                .unwrap_or(false);
        }
    }
    Ok(Bluetooth::Known { adapters, on })
}

async fn watch(send: &impl Fn(Bluetooth) -> bool) -> zbus::Result<()> {
    // Signals on a connection of their own, as the network's: a burst of them
    // can't hold back the replies.
    let listening = Connection::system().await?;
    let bus = Connection::system().await?;
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender(BLUEZ)
        .build()?;
    let mut signals = MessageStream::for_match_rule(rule, &listening, None).await?;
    if !send(read(&bus).await?) {
        return Ok(());
    }
    while signals.next().await.is_some() {
        // A burst is read once.
        while let Some(Some(_)) = signals.next().now_or_never() {}
        if !send(read(&bus).await?) {
            return Ok(());
        }
    }
    Err(zbus::Error::Failure("BlueZ went away".into()))
}

/// Watches Bluetooth on a thread of its own. Without BlueZ it says it
/// doesn't know, and tries again a while later.
pub fn listen(send: impl Fn(Bluetooth) -> bool + Send + 'static) {
    let spawned = std::thread::Builder::new().name("bluez".into()).spawn(move || {
        let mut said = false;
        loop {
            match zbus::block_on(watch(&send)) {
                Ok(()) => return,
                Err(error) => {
                    if !said {
                        tracing::info!(%error, "no Bluetooth from BlueZ");
                        said = true;
                    }
                    if !send(Bluetooth::Unknown) {
                        return;
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(5));
        }
    });
    if let Err(error) = spawned {
        tracing::warn!(%error, "no thread for BlueZ");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bluetooth_is_off_only_with_an_adapter_to_be_on() {
        assert!(Bluetooth::Known { adapters: 1, on: false }.off());
        assert!(!Bluetooth::Known { adapters: 1, on: true }.off());
        assert!(!Bluetooth::Known { adapters: 0, on: false }.off(), "none at all");
        assert!(!Bluetooth::Unknown.off());
    }
}
