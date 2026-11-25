//! Media players, by MPRIS on the session bus: the one playing, else the
//! one paused last, with its track; play, pause and the next track go back
//! to it. Read again on each player's own signals; never on a timer.

use std::{collections::HashMap, time::Duration};

use futures_util::{StreamExt, stream::select_all};
use zbus::{Connection, MatchRule, MessageStream, OwnedVariant, Proxy, fdo::DBusProxy};

const PREFIX: &str = "org.mpris.MediaPlayer2.";
const PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

/// A player with a track.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Playing {
    /// Its bus name.
    pub name: String,
    /// What it calls itself: "Firefox", "mpv".
    pub identity: String,
    pub title: String,
    pub artist: String,
    pub playing: bool,
    pub can_next: bool,
}

/// What the bar asks of the player shown.
#[derive(Clone, Debug, PartialEq)]
pub enum Ask {
    PlayPause(String),
    Next(String),
}

/// Of the players, the one to show: one playing, the last to start; else
/// the last one paused; none stopped.
pub fn chosen(players: &[Playing], order: &[String]) -> Option<Playing> {
    let by_order = |player: &&Playing| order.iter().position(|name| *name == player.name).unwrap_or(0);
    players
        .iter()
        .filter(|player| player.playing)
        .max_by_key(by_order)
        .or_else(|| players.iter().max_by_key(by_order))
        .cloned()
}

async fn read(bus: &Connection, name: &str) -> zbus::Result<Option<Playing>> {
    let player = Proxy::new(bus, name.to_owned(), PATH, PLAYER).await?;
    let status: String = player.get_property("PlaybackStatus").await.unwrap_or_default();
    if status != "Playing" && status != "Paused" {
        return Ok(None);
    }
    let metadata: HashMap<String, OwnedVariant> = player.get_property("Metadata").await.unwrap_or_default();
    let title = metadata
        .get("xesam:title")
        .and_then(|v| String::try_from(v.clone()).ok())
        .unwrap_or_default();
    let artist = metadata
        .get("xesam:artist")
        .and_then(|v| Vec::<String>::try_from(v.clone()).ok())
        .map(|all| all.join(", "))
        .unwrap_or_default();
    let root = Proxy::new(bus, name.to_owned(), PATH, "org.mpris.MediaPlayer2").await?;
    Ok(Some(Playing {
        name: name.to_owned(),
        identity: root.get_property("Identity").await.unwrap_or_default(),
        title,
        artist,
        playing: status == "Playing",
        can_next: player.get_property("CanGoNext").await.unwrap_or(false),
    }))
}

enum Wake {
    /// A player's properties changed, by its unique name.
    Changed(String),
    /// A name came or went.
    Names,
    Ask(Ask),
}

async fn run(send: &impl Fn(Option<Playing>) -> bool, asks: async_channel::Receiver<Ask>) -> zbus::Result<()> {
    let bus = Connection::session().await?;
    let dbus = DBusProxy::new(&bus).await?;
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.DBus.Properties")
        .member("PropertiesChanged")
        .path(PATH)
        .build()?;
    let changes = MessageStream::for_match_rule(rule, &bus, None).await?;
    let mut wakes = select_all([
        changes
            .map(|message| match message {
                Ok(message) => Wake::Changed(
                    message
                        .header()
                        .sender()
                        .map(|sender| sender.to_string())
                        .unwrap_or_default(),
                ),
                Err(_) => Wake::Names,
            })
            .boxed(),
        dbus.receive_name_owner_changed().await?.map(|_| Wake::Names).boxed(),
        asks.map(Wake::Ask).boxed(),
    ]);
    // Each player by name, its unique name, and in the order they last
    // started playing.
    let mut players: HashMap<String, Playing> = HashMap::new();
    let mut owners: HashMap<String, String> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut fresh = true;
    let mut last = None;
    loop {
        if fresh {
            fresh = false;
            let names: Vec<String> = dbus
                .list_names()
                .await?
                .into_iter()
                .map(|name| name.to_string())
                .filter(|name| name.starts_with(PREFIX))
                .collect();
            players.retain(|name, _| names.contains(name));
            owners.retain(|_, name| names.contains(name));
            for name in &names {
                if !owners.values().any(|known| known == name)
                    && let Ok(bus_name) = zbus::names::BusName::try_from(name.as_str())
                    && let Ok(owner) = dbus.get_name_owner(bus_name).await
                {
                    owners.insert(owner.to_string(), name.clone());
                }
                if !players.contains_key(name)
                    && let Ok(Some(player)) = read(&bus, name).await
                {
                    if player.playing {
                        order.retain(|other| other != name);
                        order.push(name.clone());
                    }
                    players.insert(name.clone(), player);
                }
            }
        }
        let all: Vec<Playing> = players.values().cloned().collect();
        let now = chosen(&all, &order);
        if last.as_ref() != Some(&now) {
            if !send(now.clone()) {
                return Ok(());
            }
            last = Some(now);
        }
        match wakes.next().await {
            Some(Wake::Changed(owner)) => {
                if let Some(name) = owners.get(&owner).cloned() {
                    match read(&bus, &name).await {
                        Ok(Some(player)) => {
                            let started = player.playing && !players.get(&name).is_some_and(|was| was.playing);
                            if started {
                                order.retain(|other| *other != name);
                                order.push(name.clone());
                            }
                            players.insert(name, player);
                        }
                        _ => {
                            players.remove(&name);
                        }
                    }
                } else {
                    fresh = true;
                }
            }
            Some(Wake::Names) => fresh = true,
            Some(Wake::Ask(ask)) => {
                let (name, method) = match &ask {
                    Ask::PlayPause(name) => (name, "PlayPause"),
                    Ask::Next(name) => (name, "Next"),
                };
                if let Ok(player) = Proxy::new(&bus, name.clone(), PATH, PLAYER).await
                    && let Err(error) = player.call_method(method, &()).await
                {
                    tracing::warn!(%error, name, method, "a player did not answer");
                }
            }
            None => return Ok(()),
        }
    }
}

/// The players on a thread of their own: `send` hears the one to show, none
/// for none; what is sent on the returned channel goes to it.
pub fn listen(send: impl Fn(Option<Playing>) -> bool + Send + 'static) -> async_channel::Sender<Ask> {
    let (asks, heard) = async_channel::unbounded();
    let spawned = std::thread::Builder::new().name("mpris".into()).spawn(move || {
        loop {
            match zbus::block_on(run(&send, heard.clone())) {
                Ok(()) => return,
                Err(error) => tracing::warn!(%error, "no media players"),
            }
            if !send(None) {
                return;
            }
            std::thread::sleep(Duration::from_secs(5));
        }
    });
    if let Err(error) = spawned {
        tracing::warn!(%error, "no thread for media players");
    }
    asks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(name: &str, playing: bool) -> Playing {
        Playing {
            name: name.into(),
            playing,
            ..Playing::default()
        }
    }

    #[test]
    fn the_one_playing_last_is_shown_else_the_last_paused() {
        let order = vec!["a".to_owned(), "b".to_owned()];
        let both = [player("a", true), player("b", true)];
        assert_eq!(chosen(&both, &order).unwrap().name, "b");
        let one = [player("a", true), player("b", false)];
        assert_eq!(chosen(&one, &order).unwrap().name, "a");
        let none = [player("a", false), player("b", false)];
        assert_eq!(chosen(&none, &order).unwrap().name, "b");
        assert_eq!(chosen(&[], &order), None);
    }
}
