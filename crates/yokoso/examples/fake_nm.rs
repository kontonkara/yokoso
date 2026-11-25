//! A network manager for the stand, on the bus it is given as the system's:
//! a Wi-Fi connection that goes down and up, a VPN that comes and goes, and
//! bursts of the signals a scan sends, at a word from the test:
//!
//!   busctl --user call org.freedesktop.NetworkManager /stand stand.Network Wifi b false
//!   busctl --user call org.freedesktop.NetworkManager /stand stand.Network Vpn b true
//!   busctl --user call org.freedesktop.NetworkManager /stand stand.Network Connecting b true
//!   busctl --user call org.freedesktop.NetworkManager /stand stand.Network Strength y 40
//!   busctl --user call org.freedesktop.NetworkManager /stand stand.Network Burst u 500

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use zbus::{Connection, OwnedObjectPath, OwnedVariant, Variant, interface};

const ROOT: &str = "/org/freedesktop/NetworkManager";
const WIFI: &str = "/org/freedesktop/NetworkManager/ActiveConnection/1";
const VPN: &str = "/org/freedesktop/NetworkManager/ActiveConnection/2";
const DEVICE: &str = "/org/freedesktop/NetworkManager/Devices/1";
const POINT: &str = "/org/freedesktop/NetworkManager/AccessPoint/1";
const IP4: &str = "/org/freedesktop/NetworkManager/IP4Config/1";

struct State {
    wifi: bool,
    /// The Wi-Fi on its way up, not up yet.
    connecting: bool,
    vpn: bool,
    strength: u8,
}

type Shared = Arc<Mutex<State>>;

fn path(name: &str) -> OwnedObjectPath {
    OwnedObjectPath::try_from(name).expect("a path")
}

struct Root(Shared);

#[interface(name = "org.freedesktop.NetworkManager")]
impl Root {
    #[zbus(property)]
    fn wireless_enabled(&self) -> bool {
        self.0.lock().unwrap().wifi
    }

    #[zbus(property)]
    fn wireless_hardware_enabled(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn primary_connection(&self) -> OwnedObjectPath {
        let state = self.0.lock().unwrap();
        path(if state.wifi && !state.connecting { WIFI } else { "/" })
    }

    #[zbus(property)]
    fn active_connections(&self) -> Vec<OwnedObjectPath> {
        let state = self.0.lock().unwrap();
        let mut up = Vec::new();
        if state.wifi {
            up.push(path(WIFI));
        }
        if state.vpn {
            up.push(path(VPN));
        }
        up
    }
}

/// An active connection: the Wi-Fi, or the VPN as a tunnel the manager sees
/// but didn't make.
struct Active {
    id: &'static str,
    kind: &'static str,
    wifi: bool,
    shared: Shared,
}

#[interface(name = "org.freedesktop.NetworkManager.Connection.Active")]
impl Active {
    #[zbus(property)]
    fn id(&self) -> &str {
        self.id
    }

    #[zbus(property, name = "Type")]
    fn kind(&self) -> &str {
        self.kind
    }

    #[zbus(property)]
    fn vpn(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn state(&self) -> u32 {
        // 1 on its way up, 2 up.
        if self.wifi && self.shared.lock().unwrap().connecting {
            1
        } else {
            2
        }
    }

    #[zbus(property)]
    fn devices(&self) -> Vec<OwnedObjectPath> {
        if self.wifi { vec![path(DEVICE)] } else { vec![] }
    }

    #[zbus(property)]
    fn ip4_config(&self) -> OwnedObjectPath {
        path(if self.wifi { IP4 } else { "/" })
    }
}

struct Device;

#[interface(name = "org.freedesktop.NetworkManager.Device.Wireless")]
impl Device {
    #[zbus(property)]
    fn active_access_point(&self) -> OwnedObjectPath {
        path(POINT)
    }
}

struct Point(Shared);

#[interface(name = "org.freedesktop.NetworkManager.AccessPoint")]
impl Point {
    #[zbus(property)]
    fn strength(&self) -> u8 {
        self.0.lock().unwrap().strength
    }

    #[zbus(property)]
    fn ssid(&self) -> Vec<u8> {
        b"Stand".to_vec()
    }
}

struct Ip4;

#[interface(name = "org.freedesktop.NetworkManager.IP4Config")]
impl Ip4 {
    #[zbus(property)]
    fn address_data(&self) -> Vec<HashMap<String, OwnedVariant>> {
        let address = Variant::from("192.168.1.5").try_to_owned().expect("a plain value");
        vec![HashMap::from([("address".to_owned(), address)])]
    }
}

/// Says some properties of `path` changed, as the manager does: what the
/// shell reads again whatever they are.
async fn changed(bus: &Connection, path: &str, interface: &str) {
    let none: HashMap<&str, Variant<'_>> = HashMap::new();
    let _ = bus
        .emit_signal(
            None::<&str>,
            path,
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
            &(interface, none, Vec::<&str>::new()),
        )
        .await;
}

struct Stand(Shared);

#[interface(name = "stand.Network")]
impl Stand {
    async fn wifi(&self, on: bool, #[zbus(connection)] bus: &Connection) {
        self.0.lock().unwrap().wifi = on;
        changed(bus, ROOT, "org.freedesktop.NetworkManager").await;
    }

    async fn vpn(&self, up: bool, #[zbus(connection)] bus: &Connection) {
        self.0.lock().unwrap().vpn = up;
        changed(bus, ROOT, "org.freedesktop.NetworkManager").await;
    }

    /// The Wi-Fi on its way up, or up.
    async fn connecting(&self, on: bool, #[zbus(connection)] bus: &Connection) {
        self.0.lock().unwrap().connecting = on;
        changed(bus, ROOT, "org.freedesktop.NetworkManager").await;
    }

    /// The Wi-Fi's strength, in percent.
    async fn strength(&self, value: u8, #[zbus(connection)] bus: &Connection) {
        self.0.lock().unwrap().strength = value;
        changed(bus, POINT, "org.freedesktop.NetworkManager.AccessPoint").await;
    }

    /// `count` signals at once, as a scan sends about the points it sees.
    async fn burst(&self, count: u32, #[zbus(connection)] bus: &Connection) {
        for n in 0..count {
            self.0.lock().unwrap().strength = 60 + (n % 20) as u8;
            changed(bus, POINT, "org.freedesktop.NetworkManager.AccessPoint").await;
        }
    }
}

fn main() -> zbus::Result<()> {
    zbus::block_on(async {
        let state = Arc::new(Mutex::new(State {
            wifi: true,
            connecting: false,
            vpn: false,
            strength: 72,
        }));
        let bus = Connection::system().await?;
        let server = bus.object_server();
        server.at(ROOT, Root(state.clone())).await?;
        let wifi = Active {
            id: "Stand",
            kind: "802-11-wireless",
            wifi: true,
            shared: state.clone(),
        };
        server.at(WIFI, wifi).await?;
        let vpn = Active {
            id: "myvpn",
            kind: "tun",
            wifi: false,
            shared: state.clone(),
        };
        server.at(VPN, vpn).await?;
        server.at(DEVICE, Device).await?;
        server.at(POINT, Point(state.clone())).await?;
        server.at(IP4, Ip4).await?;
        server.at("/stand", Stand(state)).await?;
        bus.request_name("org.freedesktop.NetworkManager").await?;
        std::future::pending::<()>().await;
        Ok(())
    })
}
