//! Holds a name on the session bus, and nothing more, until it is stopped:
//! another server keeping the one the shell wants, for the stand.
//!
//!   hold_name org.freedesktop.Notifications

fn main() -> zbus::Result<()> {
    let name = std::env::args().nth(1).expect("usage: hold_name NAME");
    zbus::block_on(async {
        let bus = zbus::Connection::session().await?;
        bus.request_name(name.as_str()).await?;
        std::future::pending::<()>().await;
        Ok(())
    })
}
