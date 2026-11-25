//! A tray item for the stand: an icon of pixels and a menu, which writes
//! down every call it gets, a line each, to the file it is given.
//!
//!   fake_tray LOG

use std::{collections::HashMap, io::Write};

use zbus::{Connection, OwnedVariant, Variant, interface};

struct Item {
    log: String,
}

fn note(log: &str, line: &str) {
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(log) {
        let _ = writeln!(file, "{line}");
    }
}

/// A square of `side` pixels in one color, ARGB in network order.
fn square(side: i32, argb: [u8; 4]) -> (i32, i32, Vec<u8>) {
    (side, side, argb.repeat((side * side) as usize))
}

#[interface(name = "org.kde.StatusNotifierItem")]
impl Item {
    fn activate(&self, _x: i32, _y: i32) {
        note(&self.log, "activate");
    }

    fn secondary_activate(&self, _x: i32, _y: i32) {
        note(&self.log, "secondary");
    }

    fn scroll(&self, delta: i32, orientation: &str) {
        note(&self.log, &format!("scroll {delta} {orientation}"));
    }

    #[zbus(property)]
    fn id(&self) -> &str {
        "fake"
    }

    #[zbus(property)]
    fn title(&self) -> &str {
        "Fake tray item"
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "Active"
    }

    #[zbus(property)]
    fn icon_name(&self) -> &str {
        ""
    }

    #[zbus(property)]
    fn icon_pixmap(&self) -> Vec<(i32, i32, Vec<u8>)> {
        // Teal, at two sizes.
        vec![square(16, [255, 0, 160, 160]), square(48, [255, 0, 160, 160])]
    }

    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn menu(&self) -> zbus::ObjectPath<'_> {
        zbus::ObjectPath::from_static_str_unchecked("/Menu")
    }
}

struct Menu {
    log: String,
}

type Node = (i32, HashMap<String, OwnedVariant>, Vec<OwnedVariant>);

fn row(id: i32, props: &[(&str, Variant<'static>)], children: Vec<OwnedVariant>) -> OwnedVariant {
    let props: HashMap<String, OwnedVariant> = props
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.try_to_owned().expect("plain values")))
        .collect();
    Variant::from(zbus::Structure::from((id, props, children)))
        .try_to_owned()
        .expect("plain values")
}

#[interface(name = "com.canonical.dbusmenu")]
impl Menu {
    fn about_to_show(&self, id: i32) -> bool {
        note(&self.log, &format!("about-to-show {id}"));
        false
    }

    fn event(&self, id: i32, event: &str, _data: Variant<'_>, _time: u32) {
        note(&self.log, &format!("event {id} {event}"));
    }

    fn get_layout(&self, parent: i32, _depth: i32, _props: Vec<String>) -> (u32, Node) {
        let label = |text: &str| ("label", Variant::from(text.to_owned()));
        let rows = if parent == 4 {
            vec![row(5, &[label("Deeper")], Vec::new())]
        } else {
            vec![
                row(1, &[label("_Open")], Vec::new()),
                row(2, &[("type", Variant::from("separator".to_owned()))], Vec::new()),
                row(
                    3,
                    &[
                        label("Muted"),
                        ("toggle-type", Variant::from("checkmark".to_owned())),
                        ("toggle-state", Variant::from(1i32)),
                    ],
                    Vec::new(),
                ),
                row(
                    4,
                    &[label("More"), ("children-display", Variant::from("submenu".to_owned()))],
                    vec![row(5, &[label("Deeper")], Vec::new())],
                ),
                row(6, &[label("Hidden"), ("visible", Variant::from(false))], Vec::new()),
                row(7, &[label("Quit")], Vec::new()),
            ]
        };
        (1, (parent, HashMap::new(), rows))
    }
}

fn main() -> zbus::Result<()> {
    let log = std::env::args().nth(1).expect("usage: fake_tray LOG");
    zbus::block_on(async {
        let bus = Connection::session().await?;
        bus.object_server()
            .at("/StatusNotifierItem", Item { log: log.clone() })
            .await?;
        bus.object_server().at("/Menu", Menu { log: log.clone() }).await?;
        let name = format!("org.kde.StatusNotifierItem-{}-1", std::process::id());
        bus.request_name(name.as_str()).await?;
        // A watcher may come later than this: it is asked again until one
        // takes it.
        loop {
            let watcher = zbus::Proxy::new(
                &bus,
                "org.kde.StatusNotifierWatcher",
                "/StatusNotifierWatcher",
                "org.kde.StatusNotifierWatcher",
            )
            .await?;
            if watcher
                .call_method("RegisterStatusNotifierItem", &(name.as_str(),))
                .await
                .is_ok()
            {
                note(&log, "registered");
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        std::future::pending::<()>().await;
        Ok(())
    })
}
