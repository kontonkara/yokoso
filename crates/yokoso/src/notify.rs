//! The notification server: `org.freedesktop.Notifications` on the session
//! bus, as its specification 1.2 has it. What apps send goes to the shell as
//! notifications; what is done with them (closed, an action, a reply) comes
//! back and goes out as the specification's signals. With the name held by
//! another server, it waits in the queue for it and takes it once let go.

use std::{
    collections::HashMap,
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use futures_util::StreamExt;
use yokoso_ui::picture::Picture;
use zbus::{
    Connection, OwnedVariant, Variant,
    fdo::{RequestNameFlags, RequestNameReply},
    object_server::SignalEmitter,
};

use crate::icon_theme::Lookup;

const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
/// The action that stands for a reply typed in the notification itself.
const REPLY: &str = "inline-reply";
/// How large pictures are read: twice a toast's, for a sharp one at 2x.
const PICTURE_PIXELS: u32 = 96;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Urgency {
    Low,
    #[default]
    Normal,
    Critical,
}

/// A notification as the shell has it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Notification {
    pub id: u32,
    /// The app's name, as it gives it; its desktop entry's, if it says.
    pub app: String,
    pub desktop: String,
    pub summary: String,
    /// Plain text: any markup taken out.
    pub body: String,
    /// The app's icon, and a picture of its own (a sender's face, a cover).
    pub icon: Option<Picture>,
    pub image: Option<Picture>,
    /// What it named them by, for history kept on disk.
    pub icon_name: String,
    /// Its buttons: key and label; "default" is a click on it, not a button.
    pub actions: Vec<(String, String)>,
    pub default_action: bool,
    /// Room for a reply typed in it, with the words to show in it empty.
    pub reply: Option<String>,
    pub urgency: Urgency,
    /// Not to be kept once gone from sight.
    pub transient: bool,
    /// Stays after an action.
    pub resident: bool,
    /// How long it shows: the server's own, none (until closed), or this.
    pub timeout: Timeout,
    /// It takes the place of one shown before, with the same id.
    pub replaces: bool,
    /// When it came, Unix seconds.
    pub at: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Timeout {
    #[default]
    Ours,
    Never,
    After(Duration),
}

/// Why it went, as the specification numbers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    Dismissed = 2,
    Asked = 3,
}

/// What apps say.
#[derive(Clone, Debug, PartialEq)]
pub enum Said {
    Notify(Box<Notification>),
    /// An app closed its own.
    Close(u32),
    /// Whether this server holds the name now.
    Serving(bool),
}

/// What the shell did with one, to be told to its app.
#[derive(Clone, Debug, PartialEq)]
pub enum Ask {
    Closed(u32, Reason),
    /// An action, with an activation token for the window it opens.
    Invoke(u32, String, Option<String>),
    Reply(u32, String),
}

/// Text without markup: tags out, the five entities back to their
/// characters, `<br>` a line break.
pub fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        match rest[start..].find('>') {
            Some(end) => {
                let tag = rest[start + 1..start + end].trim().to_ascii_lowercase();
                if tag.starts_with("br") {
                    out.push('\n');
                }
                rest = &rest[start + end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
        .trim()
        .to_owned()
}

/// `image-data` as the specification has it: width, height, bytes a row,
/// alpha or not, bits a sample, samples a pixel, the bytes.
type ImageData = (i32, i32, i32, bool, i32, i32, Vec<u8>);

/// A picture from `image-data`: RGB or RGBA, 8 bits a sample, rows padded.
pub fn image(data: &ImageData) -> Option<Picture> {
    let (width, height, stride, alpha, bits, channels, bytes) = data;
    if *bits != 8 || !matches!((alpha, channels), (true, 4) | (false, 3)) || *width <= 0 || *height <= 0 {
        return None;
    }
    let (width, height, stride, channels) = (*width as usize, *height as usize, *stride as usize, *channels as usize);
    if stride < width * channels || bytes.len() < stride * (height - 1) + width * channels {
        return None;
    }
    let mut pixels = Vec::with_capacity(width * height * 4);
    for row in 0..height {
        for column in 0..width {
            let at = row * stride + column * channels;
            let a = if channels == 4 { u32::from(bytes[at + 3]) } else { 255 };
            let premultiply = |c: u8| ((u32::from(c) * a + 127) / 255) as u8;
            pixels.extend([
                premultiply(bytes[at]),
                premultiply(bytes[at + 1]),
                premultiply(bytes[at + 2]),
                a as u8,
            ]);
        }
    }
    let picture = Picture::from_rgba(width as u32, height as u32, pixels)?;
    // Large ones made small as they come: the atlas is for small things.
    let largest = PICTURE_PIXELS;
    if picture.width > largest || picture.height > largest {
        let scale = largest as f32 / picture.width.max(picture.height) as f32;
        let w = ((picture.width as f32 * scale).round() as u32).max(1);
        let h = ((picture.height as f32 * scale).round() as u32).max(1);
        return picture.resized(w, h);
    }
    Some(picture)
}

fn hint<T: TryFrom<Variant<'static>>>(hints: &HashMap<String, OwnedVariant>, keys: &[&str]) -> Option<T> {
    keys.iter().find_map(|key| {
        let value: Variant<'static> = hints.get(*key)?.try_clone().ok()?.into();
        T::try_from(value).ok()
    })
}

/// A picture named by a path, a file URL or an icon name.
fn picture(name: &str, icons: &mut Lookup) -> Option<Picture> {
    let name = name.strip_prefix("file://").unwrap_or(name);
    let path = icons.find(name, PICTURE_PIXELS, None)?;
    Picture::load(&path, PICTURE_PIXELS)
}

/// A notification from what `Notify` was given; `id` is the one it gets.
#[allow(clippy::too_many_arguments)]
fn notification(
    id: u32,
    app: &str,
    replaces: bool,
    app_icon: &str,
    summary: &str,
    body: &str,
    actions: &[String],
    hints: &HashMap<String, OwnedVariant>,
    timeout: i32,
    icons: &mut Lookup,
) -> Notification {
    let urgency = match hint::<u8>(hints, &["urgency"]) {
        Some(0) => Urgency::Low,
        Some(2) => Urgency::Critical,
        _ => Urgency::Normal,
    };
    let desktop: String = hint(hints, &["desktop-entry"]).unwrap_or_default();
    let image = hint::<ImageData>(hints, &["image-data", "image_data", "icon_data"])
        .and_then(|data| image(&data))
        .or_else(|| hint::<String>(hints, &["image-path", "image_path"]).and_then(|name| picture(&name, icons)));
    let icon_name = if app_icon.is_empty() {
        desktop.clone()
    } else {
        app_icon.to_owned()
    };
    let icon = picture(&icon_name, icons);
    let mut pairs = Vec::new();
    let mut default_action = false;
    let mut reply = None;
    for pair in actions.chunks(2) {
        let [key, label] = pair else { continue };
        match key.as_str() {
            "default" => default_action = true,
            REPLY => {
                let placeholder: String = hint(hints, &["x-kde-reply-placeholder-text"]).unwrap_or_default();
                reply = Some(if placeholder.is_empty() {
                    label.clone()
                } else {
                    placeholder
                });
            }
            _ => pairs.push((key.clone(), label.clone())),
        }
    }
    let flag = |key: &str| hint::<bool>(hints, &[key]).unwrap_or(false);
    Notification {
        id,
        app: app.to_owned(),
        desktop,
        summary: plain(summary),
        body: plain(body),
        icon,
        image,
        icon_name,
        actions: pairs,
        default_action,
        reply,
        urgency,
        transient: flag("transient"),
        resident: flag("resident"),
        timeout: match timeout {
            ..0 => Timeout::Ours,
            0 => Timeout::Never,
            ms => Timeout::After(Duration::from_millis(ms as u64)),
        },
        replaces,
        at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_secs() as i64),
    }
}

struct Server {
    send: Box<dyn Fn(Said) -> bool + Send + Sync>,
    next: AtomicU32,
    icons: std::sync::Mutex<Lookup>,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Server {
    fn get_capabilities(&self) -> Vec<&str> {
        vec![
            "actions",
            "body",
            "icon-static",
            "persistence",
            "inline-reply",
            "x-kde-reply",
        ]
    }

    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Vec<String>,
        hints: HashMap<String, OwnedVariant>,
        expire_timeout: i32,
    ) -> u32 {
        let id = if replaces_id > 0 {
            replaces_id
        } else {
            self.next.fetch_add(1, Ordering::Relaxed)
        };
        let mut icons = self.icons.lock().expect("not poisoned");
        let made = notification(
            id,
            app_name,
            replaces_id > 0,
            app_icon,
            summary,
            body,
            &actions,
            &hints,
            expire_timeout,
            &mut icons,
        );
        (self.send)(Said::Notify(Box::new(made)));
        id
    }

    fn close_notification(&self, id: u32) {
        (self.send)(Said::Close(id));
    }

    fn get_server_information(&self) -> (&str, &str, &str, &str) {
        ("yokoso", "yokoso", env!("CARGO_PKG_VERSION"), "1.2")
    }

    #[zbus(signal)]
    async fn notification_closed(emitter: &SignalEmitter<'_>, id: u32, reason: u32) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn action_invoked(emitter: &SignalEmitter<'_>, id: u32, action_key: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn activation_token(emitter: &SignalEmitter<'_>, id: u32, token: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn notification_replied(emitter: &SignalEmitter<'_>, id: u32, text: &str) -> zbus::Result<()>;
}

async fn run(
    send: impl Fn(Said) -> bool + Send + Sync + Clone + 'static,
    asks: async_channel::Receiver<Ask>,
) -> zbus::Result<()> {
    let bus = Connection::session().await?;
    let server = Server {
        send: Box::new(send.clone()),
        next: AtomicU32::new(1),
        icons: std::sync::Mutex::new(Lookup::desktop()),
    };
    bus.object_server().at(PATH, server).await?;
    // Into the queue: another server keeps it until it lets go.
    let reply = bus
        .request_name_with_flags(NAME, RequestNameFlags::ALLOW_REPLACEMENT)
        .await?;
    let mut serving = reply == RequestNameReply::PrimaryOwner || reply == RequestNameReply::AlreadyOwner;
    tracing::info!(serving, "notifications");
    if !send(Said::Serving(serving)) {
        return Ok(());
    }
    let dbus = zbus::fdo::DBusProxy::new(&bus).await?;
    let mut wakes = futures_util::stream::select_all([
        dbus.receive_name_acquired()
            .await?
            .map(move |signal| {
                Wake::Serving(
                    signal
                        .args()
                        .is_ok_and(|args| args.name().as_str() == NAME)
                        .then_some(true),
                )
            })
            .boxed(),
        dbus.receive_name_lost()
            .await?
            .map(move |signal| {
                Wake::Serving(
                    signal
                        .args()
                        .is_ok_and(|args| args.name().as_str() == NAME)
                        .then_some(false),
                )
            })
            .boxed(),
        asks.map(Wake::Ask).boxed(),
    ]);
    let emitter = SignalEmitter::new(&bus, PATH)?;
    while let Some(wake) = wakes.next().await {
        match wake {
            Wake::Serving(Some(now)) if now != serving => {
                serving = now;
                tracing::info!(serving, "notifications");
                if !send(Said::Serving(serving)) {
                    return Ok(());
                }
            }
            Wake::Serving(_) => {}
            Wake::Ask(ask) => {
                let said = match ask {
                    Ask::Closed(id, reason) => Server::notification_closed(&emitter, id, reason as u32).await,
                    Ask::Invoke(id, key, token) => {
                        if let Some(token) = token {
                            let _ = Server::activation_token(&emitter, id, &token).await;
                        }
                        Server::action_invoked(&emitter, id, &key).await
                    }
                    Ask::Reply(id, text) => Server::notification_replied(&emitter, id, &text).await,
                };
                if let Err(error) = said {
                    tracing::warn!(%error, "notifications: a signal did not go out");
                }
            }
        }
    }
    Ok(())
}

/// What wakes the server's thread: the name gained or lost, or the shell
/// asking.
enum Wake {
    Serving(Option<bool>),
    Ask(Ask),
}

/// The server on a thread of its own: `send` hears what apps say; what is
/// sent on the returned channel goes back to them. Without a session bus it
/// tries again a while later.
pub fn listen(send: impl Fn(Said) -> bool + Send + Sync + Clone + 'static) -> async_channel::Sender<Ask> {
    let (asks, heard) = async_channel::unbounded();
    let spawned = std::thread::Builder::new().name("notifications".into()).spawn(move || {
        loop {
            match zbus::block_on(run(send.clone(), heard.clone())) {
                Ok(()) => return,
                Err(error) => tracing::warn!(%error, "no notification server"),
            }
            if !send(Said::Serving(false)) {
                return;
            }
            std::thread::sleep(Duration::from_secs(5));
        }
    });
    if let Err(error) = spawned {
        tracing::warn!(%error, "no thread for notifications");
    }
    asks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_comes_out_as_plain_text() {
        assert_eq!(plain("<b>Bold</b> and <i>it</i>"), "Bold and it");
        assert_eq!(plain("a<br/>b"), "a\nb");
        assert_eq!(plain("Tom &amp; Jerry &lt;3"), "Tom & Jerry <3");
        assert_eq!(plain("<a href=\"x\">link</a>"), "link");
        assert_eq!(plain("2 < 3"), "2 < 3", "a lone bracket stays");
    }

    #[test]
    fn image_data_of_rows_with_room_turns_into_rgba() {
        // 2 by 1, RGB, rows of 8 bytes: red, then green.
        let data = (2, 1, 8, false, 8, 3, vec![255, 0, 0, 0, 255, 0, 9, 9]);
        let picture = image(&data).unwrap();
        assert_eq!(*picture.pixels, vec![255, 0, 0, 255, 0, 255, 0, 255]);
        // Half-transparent white, RGBA: premultiplied.
        let data = (1, 1, 4, true, 8, 4, vec![255, 255, 255, 128]);
        assert_eq!(*image(&data).unwrap().pixels, vec![128, 128, 128, 128]);
        assert!(image(&(2, 2, 6, false, 8, 3, vec![0; 6])).is_none(), "short of bytes");
    }

    #[test]
    fn actions_and_hints_make_a_notification() {
        let mut icons = Lookup::new(Vec::new(), Some("none".into()));
        let actions: Vec<String> = ["default", "Open", "mark", "Mark as read", REPLY, "Reply"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let hints = HashMap::from([
            ("urgency".to_owned(), OwnedVariant::from(2u8)),
            ("transient".to_owned(), OwnedVariant::from(true)),
        ]);
        let made = notification(
            7,
            "Chat",
            false,
            "",
            "<b>Ann</b>",
            "hi &amp; bye",
            &actions,
            &hints,
            0,
            &mut icons,
        );
        assert_eq!((made.summary.as_str(), made.body.as_str()), ("Ann", "hi & bye"));
        assert!(made.default_action);
        assert_eq!(made.actions, [("mark".to_owned(), "Mark as read".to_owned())]);
        assert_eq!(made.reply.as_deref(), Some("Reply"));
        assert_eq!(made.urgency, Urgency::Critical);
        assert!(made.transient && !made.resident);
        assert_eq!(made.timeout, Timeout::Never);
        let quick = notification(8, "", false, "", "", "", &[], &HashMap::new(), 1500, &mut icons);
        assert_eq!(quick.timeout, Timeout::After(Duration::from_millis(1500)));
        assert_eq!(quick.urgency, Urgency::Normal);
    }
}
