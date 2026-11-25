//! The tray: StatusNotifierItems on the session bus. yokoso watches for them
//! where nobody does, and takes the watching over when the one who did goes;
//! else it hosts, reading what the other watcher lists. Each item is read
//! again on its own signals; clicks and menus go back to it. Menus are
//! dbusmenu, a level at a time.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_util::{StreamExt, stream::select_all};
use yokoso_ui::picture::Picture;
use zbus::{
    Connection, Proxy,
    fdo::{DBusProxy, RequestNameFlags, RequestNameReply},
    message::Header,
    object_server::SignalEmitter,
};

use crate::icon_theme::Lookup;

const WATCHER: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";
const ITEM: &str = "org.kde.StatusNotifierItem";
const MENU: &str = "com.canonical.dbusmenu";
/// Pixels an icon is drawn at: twice its size on the bar, for scaled screens.
const ICON_PIXELS: u32 = 40;

/// An item as the bar shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// "bus/path": how it is called back.
    pub address: String,
    pub id: String,
    pub title: String,
    /// Passive, Active or NeedsAttention.
    pub status: String,
    pub icon: Option<Picture>,
    pub tooltip: String,
    /// Its menu's path, if it has one.
    pub menu: Option<String>,
    /// A click opens its menu rather than activating it.
    pub only_menu: bool,
}

/// A row of a menu.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub separator: bool,
    /// A check (false) or a radio (true), and whether it is on.
    pub toggle: Option<(bool, bool)>,
    pub submenu: bool,
}

/// What the tray says.
#[derive(Clone, Debug, PartialEq)]
pub enum Said {
    Items(Vec<Item>),
    /// The rows under `parent` of an item's menu.
    Menu {
        address: String,
        parent: i32,
        entries: Vec<Entry>,
    },
}

/// What the bar asks of an item.
#[derive(Clone, Debug, PartialEq)]
pub enum Ask {
    Activate(String),
    Secondary(String),
    /// Wheel steps, up more.
    Scroll(String, i32),
    /// The rows of its menu under `parent`, 0 for the top.
    Menu(String, i32),
    /// A row of its menu chosen.
    Pick(String, i32),
}

/// "bus/path" into its two halves.
pub fn split(address: &str) -> (&str, &str) {
    match address.find('/') {
        Some(at) => (&address[..at], &address[at..]),
        None => (address, "/StatusNotifierItem"),
    }
}

/// The address of an item registered by `sender` as `service`: a bus name,
/// or a path on the sender's own name.
pub fn address(sender: &str, service: &str) -> String {
    if service.starts_with('/') {
        format!("{sender}{service}")
    } else if service.contains('/') {
        service.to_owned()
    } else {
        format!("{service}/StatusNotifierItem")
    }
}

type Pixmaps = Vec<(i32, i32, Vec<u8>)>;

/// Of an item's pixmaps, the smallest at least `size` wide, else the largest.
pub fn pixmap(pixmaps: &[(i32, i32, Vec<u8>)], size: u32) -> Option<Picture> {
    let fine: Vec<&(i32, i32, Vec<u8>)> = pixmaps
        .iter()
        .filter(|(w, h, data)| *w > 0 && *h > 0 && data.len() == (*w * *h * 4) as usize)
        .collect();
    let chosen = fine
        .iter()
        .filter(|(w, _, _)| *w as u32 >= size)
        .min_by_key(|(w, _, _)| *w)
        .or_else(|| fine.iter().max_by_key(|(w, _, _)| *w))?;
    Picture::from_argb32_be(chosen.0 as u32, chosen.1 as u32, &chosen.2)
}

/// A label without its access key's underscore; two of them are one.
pub fn plain(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut chars = label.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '_' {
            if chars.peek() == Some(&'_') {
                out.push('_');
                chars.next();
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The watcher yokoso is where there is none.
struct Watcher {
    items: Arc<Mutex<Vec<String>>>,
    changed: async_channel::Sender<()>,
}

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    async fn register_status_notifier_item(
        &self,
        service: &str,
        #[zbus(header)] header: Header<'_>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) {
        let sender = header.sender().map(|name| name.to_string()).unwrap_or_default();
        let address = address(&sender, service);
        let fresh = {
            let mut items = self.items.lock().expect("not poisoned");
            let fresh = !items.contains(&address);
            if fresh {
                items.push(address.clone());
            }
            fresh
        };
        if fresh {
            let _ = Self::status_notifier_item_registered(&emitter, &address).await;
            let _ = self.changed.send(()).await;
        }
    }

    async fn register_status_notifier_host(&self, _service: &str, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        let _ = Self::status_notifier_host_registered(&emitter).await;
    }

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.items.lock().expect("not poisoned").clone()
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }

    #[zbus(signal)]
    async fn status_notifier_item_registered(emitter: &SignalEmitter<'_>, service: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn status_notifier_item_unregistered(emitter: &SignalEmitter<'_>, service: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn status_notifier_host_registered(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

async fn read(bus: &Connection, address: &str, icons: &mut Lookup) -> zbus::Result<Item> {
    let (name, path) = split(address);
    let proxy = Proxy::new(bus, name.to_owned(), path.to_owned(), ITEM).await?;
    let text = async |key: &str| proxy.get_property::<String>(key).await.unwrap_or_default();
    let status = text("Status").await;
    let theme_path = text("IconThemePath").await;
    let extra = (!theme_path.is_empty()).then(|| std::path::PathBuf::from(&theme_path));
    // Asking for attention, its other icon, where it has one.
    let keys = if status == "NeedsAttention" {
        vec![("AttentionIconName", "AttentionIconPixmap"), ("IconName", "IconPixmap")]
    } else {
        vec![("IconName", "IconPixmap")]
    };
    let mut icon = None;
    for (named, drawn) in keys {
        let name = text(named).await;
        icon = icons
            .find(&name, ICON_PIXELS, extra.as_deref())
            .and_then(|path| Picture::load(&path, ICON_PIXELS));
        if icon.is_none() {
            let pixmaps: Pixmaps = proxy.get_property(drawn).await.unwrap_or_default();
            icon = pixmap(&pixmaps, ICON_PIXELS);
        }
        if icon.is_some() {
            break;
        }
    }
    let tooltip = proxy
        .get_property::<(String, Pixmaps, String, String)>("ToolTip")
        .await
        .map(|(_, _, title, body)| {
            if body.is_empty() {
                title
            } else {
                format!("{title}: {body}")
            }
        })
        .unwrap_or_default();
    let menu = proxy
        .get_property::<zbus::OwnedObjectPath>("Menu")
        .await
        .ok()
        .map(|path| path.to_string())
        .filter(|path| path != "/");
    Ok(Item {
        address: address.to_owned(),
        id: text("Id").await,
        title: text("Title").await,
        status,
        icon,
        tooltip,
        menu,
        only_menu: proxy.get_property::<bool>("ItemIsMenu").await.unwrap_or(false),
    })
}

type Node = (i32, HashMap<String, zbus::OwnedVariant>, Vec<zbus::OwnedVariant>);

/// A row by its properties; none for one that isn't shown.
fn entry(id: i32, props: &HashMap<String, zbus::OwnedVariant>, has_children: bool) -> Option<Entry> {
    let text = |key: &str| {
        props
            .get(key)
            .and_then(|v| String::try_from(v.clone()).ok())
            .unwrap_or_default()
    };
    let flag = |key: &str, otherwise: bool| {
        props
            .get(key)
            .and_then(|v| bool::try_from(v.clone()).ok())
            .unwrap_or(otherwise)
    };
    if !flag("visible", true) {
        return None;
    }
    let state = props
        .get("toggle-state")
        .and_then(|v| i32::try_from(v.clone()).ok())
        .unwrap_or(0);
    Some(Entry {
        id,
        label: plain(&text("label")),
        enabled: flag("enabled", true),
        separator: text("type") == "separator",
        toggle: match text("toggle-type").as_str() {
            "checkmark" => Some((false, state == 1)),
            "radio" => Some((true, state == 1)),
            _ => None,
        },
        submenu: has_children || text("children-display") == "submenu",
    })
}

async fn layout(bus: &Connection, item: &Item, parent: i32) -> Vec<Entry> {
    let Some(menu) = &item.menu else { return Vec::new() };
    let (name, _) = split(&item.address);
    let Ok(proxy) = Proxy::new(bus, name.to_owned(), menu.clone(), MENU).await else {
        return Vec::new();
    };
    let _ = proxy.call_method("AboutToShow", &(parent,)).await;
    let reply = proxy
        .call_method("GetLayout", &(parent, 1i32, Vec::<String>::new()))
        .await
        .and_then(|reply| reply.body().deserialize::<(u32, Node)>());
    match reply {
        Ok((_, (_, _, children))) => children
            .into_iter()
            .filter_map(|child| Node::try_from(child).ok())
            .filter_map(|(id, props, children)| entry(id, &props, !children.is_empty()))
            .collect(),
        Err(error) => {
            tracing::warn!(%error, address = item.address, "a tray menu");
            Vec::new()
        }
    }
}

async fn ask(bus: &Connection, items: &HashMap<String, Item>, ask: Ask, send: &impl Fn(Said) -> bool) {
    let address = match &ask {
        Ask::Activate(a) | Ask::Secondary(a) | Ask::Scroll(a, _) | Ask::Menu(a, _) | Ask::Pick(a, _) => a.clone(),
    };
    let Some(item) = items.get(&address) else { return };
    let (name, path) = split(&address);
    let done = match ask {
        Ask::Menu(_, parent) => {
            let entries = layout(bus, item, parent).await;
            send(Said::Menu {
                address,
                parent,
                entries,
            });
            return;
        }
        Ask::Pick(_, id) => {
            let Some(menu) = &item.menu else { return };
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs() as u32);
            match Proxy::new(bus, name.to_owned(), menu.clone(), MENU).await {
                Ok(proxy) => proxy
                    .call_method("Event", &(id, "clicked", zbus::Variant::from(0i32), now))
                    .await
                    .map(|_| ()),
                Err(error) => Err(error),
            }
        }
        Ask::Activate(_) | Ask::Secondary(_) | Ask::Scroll(..) => {
            match Proxy::new(bus, name.to_owned(), path.to_owned(), ITEM).await {
                Ok(proxy) => match ask {
                    Ask::Scroll(_, delta) => proxy.call_method("Scroll", &(delta, "vertical")).await.map(|_| ()),
                    Ask::Secondary(_) => proxy.call_method("SecondaryActivate", &(0i32, 0i32)).await.map(|_| ()),
                    _ => proxy.call_method("Activate", &(0i32, 0i32)).await.map(|_| ()),
                },
                Err(error) => Err(error),
            }
        }
    };
    if let Err(error) = done {
        tracing::warn!(%error, %address, "a tray item did not answer");
    }
}

enum Wake {
    /// The list of items may have changed.
    List,
    /// The watcher's name changed hands.
    Watcher,
    Item(String),
    Ask(Ask),
}

async fn run(send: &impl Fn(Said) -> bool, asks: async_channel::Receiver<Ask>) -> zbus::Result<()> {
    let bus = Connection::session().await?;
    // Signals on a connection of their own: on the one that asks, a burst of
    // them filling its queue would hold back the replies, for ever.
    let ears = Connection::session().await?;
    let (changed, heard) = async_channel::unbounded::<()>();
    let ours = Arc::new(Mutex::new(Vec::<String>::new()));
    bus.object_server()
        .at(
            WATCHER_PATH,
            Watcher {
                items: ours.clone(),
                changed,
            },
        )
        .await?;
    let take = async || {
        bus.request_name_with_flags(WATCHER, RequestNameFlags::DO_NOT_QUEUE)
            .await
            .is_ok_and(|reply| reply == RequestNameReply::PrimaryOwner)
    };
    let mut watching = take().await;
    tracing::info!(watching, "tray");
    let host = format!("org.kde.StatusNotifierHost-{}", std::process::id());
    let _ = bus.request_name(host.as_str()).await;
    let watcher = Proxy::new(&bus, WATCHER, WATCHER_PATH, WATCHER).await?;
    let _ = watcher
        .call_method("RegisterStatusNotifierHost", &(host.as_str(),))
        .await;
    let dbus = DBusProxy::new(&bus).await?;
    let heard_watcher = Proxy::new(&ears, WATCHER, WATCHER_PATH, WATCHER).await?;
    let mut wakes = select_all([
        heard_watcher
            .receive_signal("StatusNotifierItemRegistered")
            .await?
            .map(|_| Wake::List)
            .boxed(),
        heard_watcher
            .receive_signal("StatusNotifierItemUnregistered")
            .await?
            .map(|_| Wake::List)
            .boxed(),
        DBusProxy::new(&ears)
            .await?
            .receive_name_owner_changed()
            .await?
            .map(|signal| match signal.args() {
                Ok(args) if args.name().as_str() == WATCHER => Wake::Watcher,
                _ => Wake::List,
            })
            .boxed(),
        heard.map(|()| Wake::List).boxed(),
        asks.map(Wake::Ask).boxed(),
    ]);
    let mut icons = Lookup::desktop();
    let mut items: HashMap<String, Item> = HashMap::new();
    let mut listening: Vec<String> = Vec::new();
    let mut last: Option<Vec<Item>> = None;
    loop {
        let addresses: Vec<String> = if watching {
            ours.lock().expect("not poisoned").clone()
        } else {
            watcher
                .get_property::<Vec<String>>("RegisteredStatusNotifierItems")
                .await
                .unwrap_or_default()
        };
        // Those whose app is gone are let go of, and taken off ours.
        let mut alive = Vec::new();
        for address in addresses {
            let (name, _) = split(&address);
            let has = match zbus::names::BusName::try_from(name) {
                Ok(name) => dbus.name_has_owner(name).await.unwrap_or(false),
                Err(_) => false,
            };
            if has {
                alive.push(address);
            } else if watching {
                ours.lock().expect("not poisoned").retain(|other| *other != address);
                let emitter = SignalEmitter::new(&bus, WATCHER_PATH)?;
                let _ = Watcher::status_notifier_item_unregistered(&emitter, &address).await;
            }
        }
        items.retain(|address, _| alive.contains(address));
        for address in &alive {
            if !items.contains_key(address)
                && let Ok(item) = read(&bus, address, &mut icons).await
            {
                items.insert(address.clone(), item);
            }
            if !listening.contains(address) {
                listening.push(address.clone());
                let (name, path) = split(address);
                if let Ok(proxy) = Proxy::new(&ears, name.to_owned(), path.to_owned(), ITEM).await
                    && let Ok(stream) = proxy.receive_all_signals().await
                {
                    let address = address.clone();
                    wakes.push(stream.map(move |_| Wake::Item(address.clone())).boxed());
                }
            }
        }
        listening.retain(|address| alive.contains(address));
        let now: Vec<Item> = alive.iter().filter_map(|address| items.get(address).cloned()).collect();
        if last.as_ref() != Some(&now) {
            if !send(Said::Items(now.clone())) {
                return Ok(());
            }
            last = Some(now);
        }
        match wakes.next().await {
            Some(Wake::Item(address)) => {
                if let Ok(mut item) = read(&bus, &address, &mut icons).await {
                    // The same pixels keep the picture they had: drawn once, not
                    // again at each new tooltip.
                    if let Some(old) = items.get(&address)
                        && old.icon == item.icon
                    {
                        item.icon.clone_from(&old.icon);
                    }
                    items.insert(address, item);
                }
            }
            Some(Wake::Watcher) if !watching => {
                // The one who watched went: ours now, and the apps register
                // with us anew as they see a watcher come.
                if take().await {
                    watching = true;
                    tracing::info!("tray: watching for items now");
                }
            }
            Some(Wake::Ask(asked)) => ask(&bus, &items, asked, send).await,
            Some(_) => {}
            None => return Ok(()),
        }
    }
}

/// The tray on a thread of its own: `send` hears its items and menus; what
/// is sent on the returned channel goes to the items. Without a session bus
/// it tries again a while later.
pub fn listen(send: impl Fn(Said) -> bool + Send + 'static) -> async_channel::Sender<Ask> {
    let (asks, heard) = async_channel::unbounded();
    let spawned = std::thread::Builder::new().name("tray".into()).spawn(move || {
        loop {
            match zbus::block_on(run(&send, heard.clone())) {
                Ok(()) => return,
                Err(error) => tracing::warn!(%error, "no tray"),
            }
            if !send(Said::Items(Vec::new())) {
                return;
            }
            std::thread::sleep(Duration::from_secs(5));
        }
    });
    if let Err(error) = spawned {
        tracing::warn!(%error, "no thread for the tray");
    }
    asks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_a_bus_and_a_path() {
        assert_eq!(
            split(":1.42/org/ayatana/NotificationItem/x"),
            (":1.42", "/org/ayatana/NotificationItem/x")
        );
        assert_eq!(
            split("org.kde.StatusNotifierItem-1-1"),
            ("org.kde.StatusNotifierItem-1-1", "/StatusNotifierItem")
        );
        assert_eq!(address(":1.5", "/StatusNotifierItem"), ":1.5/StatusNotifierItem");
        assert_eq!(
            address(":1.5", "org.kde.StatusNotifierItem-9-1"),
            "org.kde.StatusNotifierItem-9-1/StatusNotifierItem"
        );
    }

    #[test]
    fn the_pixmap_nearest_above_the_size_is_taken() {
        let square = |side: i32| (side, side, vec![255; (side * side * 4) as usize]);
        let all = [square(16), square(32), square(48), square(64)];
        assert_eq!(pixmap(&all, 40).map(|p| p.width), Some(48));
        assert_eq!(pixmap(&all[..2], 40).map(|p| p.width), Some(32), "else the largest");
        assert!(pixmap(&[(4, 4, vec![0; 3])], 40).is_none(), "short of bytes");
    }

    #[test]
    fn labels_lose_their_access_keys() {
        assert_eq!(plain("_Quit"), "Quit");
        assert_eq!(plain("Save__as"), "Save_as");
    }
}
