//! BlueZ for the stand, on the bus it is given as the system's: one
//! adapter, on, turned off and on at a word from the test:
//!
//!   busctl --user call org.bluez /stand stand.Bluetooth Power b false

use zbus::{Connection, fdo::ObjectManager, interface, object_server::SignalEmitter};

struct Adapter {
    powered: bool,
}

#[interface(name = "org.bluez.Adapter1")]
impl Adapter {
    #[zbus(property)]
    fn powered(&self) -> bool {
        self.powered
    }

    #[zbus(property)]
    fn address(&self) -> &str {
        "00:11:22:33:44:55"
    }
}

struct Stand;

#[interface(name = "stand.Bluetooth")]
impl Stand {
    async fn power(&self, on: bool, #[zbus(connection)] bus: &Connection) -> zbus::fdo::Result<()> {
        let adapter = bus.object_server().interface::<_, Adapter>("/org/bluez/hci0").await?;
        adapter.get_mut().await.powered = on;
        let emitter = SignalEmitter::new(bus, "/org/bluez/hci0")?;
        adapter.get().await.powered_changed(&emitter).await?;
        Ok(())
    }
}

fn main() -> zbus::Result<()> {
    zbus::block_on(async {
        let bus = Connection::system().await?;
        let server = bus.object_server();
        server.at("/", ObjectManager).await?;
        server.at("/org/bluez/hci0", Adapter { powered: true }).await?;
        server.at("/stand", Stand).await?;
        bus.request_name("org.bluez").await?;
        std::future::pending::<()>().await;
        Ok(())
    })
}
