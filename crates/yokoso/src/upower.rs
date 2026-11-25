//! UPower: the battery as one, its DisplayDevice, from the system bus. Read
//! once, then again on each change it signals; nothing is asked on a timer.

use std::time::Duration;

use futures_util::StreamExt;
use zbus::{Connection, Proxy};

const UPOWER: &str = "org.freedesktop.UPower";
const DISPLAY: &str = "/org/freedesktop/UPower/devices/DisplayDevice";
const DEVICE: &str = "org.freedesktop.UPower.Device";

/// How the battery is charging, as UPower numbers it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Charge {
    #[default]
    Unknown,
    Charging,
    Discharging,
    Empty,
    Full,
    /// Plugged in, held where it is.
    Holding,
}

impl Charge {
    fn from_upower(state: u32) -> Self {
        match state {
            1 => Self::Charging,
            2 | 6 => Self::Discharging,
            3 => Self::Empty,
            4 => Self::Full,
            5 => Self::Holding,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Battery {
    pub percentage: f64,
    pub charge: Charge,
    /// Seconds; 0 while UPower doesn't know.
    pub to_empty: i64,
    pub to_full: i64,
    /// Watts, in or out.
    pub rate: f64,
}

/// What UPower says: a battery, none, or nothing at all.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Power {
    #[default]
    Unknown,
    Battery(Battery),
    NoBattery,
}

async fn read(device: &Proxy<'_>) -> zbus::Result<Power> {
    let present: bool = device.get_property("IsPresent").await?;
    let kind: u32 = device.get_property("Type").await?;
    if !present || kind != 2 {
        return Ok(Power::NoBattery);
    }
    Ok(Power::Battery(Battery {
        percentage: device.get_property("Percentage").await?,
        charge: Charge::from_upower(device.get_property("State").await?),
        to_empty: device.get_property("TimeToEmpty").await?,
        to_full: device.get_property("TimeToFull").await?,
        rate: device.get_property("EnergyRate").await?,
    }))
}

async fn watch(send: &impl Fn(Power) -> bool) -> zbus::Result<()> {
    let bus = Connection::system().await?;
    let device = Proxy::new(&bus, UPOWER, DISPLAY, DEVICE).await?;
    let properties = Proxy::new(&bus, UPOWER, DISPLAY, "org.freedesktop.DBus.Properties").await?;
    let mut changes = properties.receive_signal("PropertiesChanged").await?;
    if !send(read(&device).await?) {
        return Ok(());
    }
    while changes.next().await.is_some() {
        if !send(read(&device).await?) {
            return Ok(());
        }
    }
    Err(zbus::Error::Failure("UPower went away".into()))
}

/// Watches the battery on a thread of its own. Without UPower it says it
/// doesn't know, and tries again a while later.
pub fn listen(send: impl Fn(Power) -> bool + Send + 'static) {
    let spawned = std::thread::Builder::new().name("upower".into()).spawn(move || {
        let mut said = false;
        loop {
            if let Err(error) = zbus::block_on(watch(&send)) {
                if !said {
                    tracing::warn!(%error, "no battery from UPower");
                    said = true;
                }
                if !send(Power::Unknown) {
                    return;
                }
            } else {
                return;
            }
            std::thread::sleep(Duration::from_secs(5));
        }
    });
    if let Err(error) = spawned {
        tracing::warn!(%error, "no thread for UPower");
    }
}
