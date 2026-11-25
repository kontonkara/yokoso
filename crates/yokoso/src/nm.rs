//! NetworkManager: what the network is now, from the system bus. Read once,
//! then again whenever NetworkManager signals a change; never on a timer.

use std::{collections::HashMap, time::Duration};

use futures_util::StreamExt;
use zbus::{
    Connection, MatchRule, MessageStream, OwnedObjectPath, OwnedVariant, Proxy,
    proxy::{Builder, CacheProperties},
};

const NM: &str = "org.freedesktop.NetworkManager";
const ROOT: &str = "/org/freedesktop/NetworkManager";
const ACTIVE: &str = "org.freedesktop.NetworkManager.Connection.Active";

/// What the network goes through.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Link {
    #[default]
    None,
    Wifi {
        ssid: String,
        /// Percent.
        strength: u8,
    },
    Wired,
    /// Anything else that carries it: a modem, a bridge.
    Other(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Network {
    pub link: Link,
    /// Its name, as the connection has it.
    pub name: String,
    pub address: Option<String>,
    /// The name of a connection on its way up.
    pub connecting: Option<String>,
    /// Every connection that is up but the loopback: its name, its type,
    /// and whether NetworkManager calls it a VPN.
    pub up: Vec<(String, String, bool)>,
    pub wireless: bool,
    /// Radios off by a switch or a key: nothing NetworkManager can turn on.
    pub airplane: bool,
}

/// What NetworkManager says: the network, or that it isn't there.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Net {
    #[default]
    Unknown,
    Up(Network),
}

async fn proxy<'a>(bus: &Connection, path: &'a str, interface: &'a str) -> zbus::Result<Proxy<'a>> {
    Builder::<Proxy>::new(bus)
        .destination(NM)
        .path(path)
        .interface(interface)
        .cache_properties(CacheProperties::No)
        .build()
        .await
}

/// Whether a connection of `kind` named `name` counts as a VPN: what
/// NetworkManager calls one, WireGuard, and the ones the config names.
pub fn is_vpn(kind: &str, vpn: bool, name: &str, named: &[String]) -> bool {
    vpn || kind == "wireguard" || kind == "vpn" || named.iter().any(|n| n == name)
}

async fn read(bus: &Connection) -> zbus::Result<Network> {
    let root = proxy(bus, ROOT, NM).await?;
    let wireless: bool = root.get_property("WirelessEnabled").await?;
    let hardware: bool = root.get_property("WirelessHardwareEnabled").await?;
    let primary: OwnedObjectPath = root.get_property("PrimaryConnection").await?;
    let actives: Vec<OwnedObjectPath> = root.get_property("ActiveConnections").await?;
    let mut network = Network {
        wireless,
        airplane: !hardware,
        ..Network::default()
    };
    for path in &actives {
        let Ok(active) = proxy(bus, path.as_str(), ACTIVE).await else {
            continue;
        };
        let kind: String = active.get_property("Type").await.unwrap_or_default();
        let name: String = active.get_property("Id").await.unwrap_or_default();
        let vpn: bool = active.get_property("Vpn").await.unwrap_or(false);
        // 1 is on its way up, 2 up.
        let state: u32 = active.get_property("State").await.unwrap_or(0);
        if kind == "loopback" {
            continue;
        }
        if state == 2 {
            network.up.push((name.clone(), kind.clone(), vpn));
        } else if state == 1 && network.connecting.is_none() {
            network.connecting = Some(name.clone());
        }
    }
    if primary.as_str() == "/" {
        return Ok(network);
    }
    let active = proxy(bus, primary.as_str(), ACTIVE).await?;
    let kind: String = active.get_property("Type").await?;
    network.name = active.get_property("Id").await?;
    let devices: Vec<OwnedObjectPath> = active.get_property("Devices").await.unwrap_or_default();
    network.link = match kind.as_str() {
        "802-11-wireless" => {
            let mut strength = 0;
            let mut ssid = network.name.clone();
            if let Some(device) = devices.first() {
                let wifi = proxy(bus, device.as_str(), "org.freedesktop.NetworkManager.Device.Wireless").await?;
                let point: OwnedObjectPath = wifi.get_property("ActiveAccessPoint").await?;
                if point.as_str() != "/" {
                    let point = proxy(bus, point.as_str(), "org.freedesktop.NetworkManager.AccessPoint").await?;
                    strength = point.get_property("Strength").await.unwrap_or(0);
                    let bytes: Vec<u8> = point.get_property("Ssid").await.unwrap_or_default();
                    if !bytes.is_empty() {
                        ssid = String::from_utf8_lossy(&bytes).into_owned();
                    }
                }
            }
            Link::Wifi { ssid, strength }
        }
        "802-3-ethernet" => Link::Wired,
        other => Link::Other(other.to_owned()),
    };
    let config: OwnedObjectPath = active.get_property("Ip4Config").await.unwrap_or_default();
    if config.as_str() != "/" && !config.as_str().is_empty() {
        let ip4 = proxy(bus, config.as_str(), "org.freedesktop.NetworkManager.IP4Config").await?;
        let addresses: Vec<HashMap<String, OwnedVariant>> = ip4.get_property("AddressData").await.unwrap_or_default();
        network.address = addresses
            .first()
            .and_then(|address| address.get("address"))
            .and_then(|value| String::try_from(value.clone()).ok());
    }
    Ok(network)
}

async fn watch(send: &impl Fn(Net) -> bool) -> zbus::Result<()> {
    let bus = Connection::system().await?;
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender(NM)
        .build()?;
    let mut signals = MessageStream::for_match_rule(rule, &bus, None).await?;
    if !send(Net::Up(read(&bus).await?)) {
        return Ok(());
    }
    while signals.next().await.is_some() {
        if !send(Net::Up(read(&bus).await?)) {
            return Ok(());
        }
    }
    Err(zbus::Error::Failure("NetworkManager went away".into()))
}

/// Watches the network on a thread of its own. Without NetworkManager it says it doesn't know, and
/// tries again a while later.
pub fn listen(send: impl Fn(Net) -> bool + Send + 'static) {
    let spawned = std::thread::Builder::new()
        .name("networkmanager".into())
        .spawn(move || {
            let mut said = false;
            loop {
                match zbus::block_on(watch(&send)) {
                    Ok(()) => return,
                    Err(error) => {
                        if !said {
                            tracing::warn!(%error, "no network from NetworkManager");
                            said = true;
                        }
                        if !send(Net::Unknown) {
                            return;
                        }
                    }
                }
                std::thread::sleep(Duration::from_secs(5));
            }
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "no thread for NetworkManager");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vpns_are_what_networkmanager_calls_one_wireguard_and_those_named() {
        let named = vec!["myvpn".to_owned()];
        assert!(is_vpn("vpn", true, "office", &named));
        assert!(is_vpn("wireguard", false, "wg0", &named));
        assert!(is_vpn("tun", false, "myvpn", &named));
        assert!(!is_vpn("tun", false, "relay", &named));
        assert!(!is_vpn("802-11-wireless", false, "Home", &named));
    }
}
