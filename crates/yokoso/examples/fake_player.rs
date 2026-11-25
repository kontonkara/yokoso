//! A media player for the stand, by MPRIS: a track that plays, pauses and
//! goes on to the next, saying so as players do, and writing down each call
//! to the file it is given.
//!
//!   fake_player LOG

use std::{collections::HashMap, io::Write};

use zbus::{Connection, OwnedVariant, Variant, interface, object_server::SignalEmitter};

fn note(log: &str, line: &str) {
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(log) {
        let _ = writeln!(file, "{line}");
    }
}

struct Root;

#[interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    #[zbus(property)]
    fn identity(&self) -> &str {
        "Fake player"
    }
}

struct Player {
    log: String,
    playing: bool,
    track: usize,
}

const TRACKS: [(&str, &str); 2] = [("First song", "The band"), ("Second song", "The band")];

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    async fn play_pause(&mut self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        note(&self.log, "play-pause");
        self.playing = !self.playing;
        let _ = self.playback_status_changed(&emitter).await;
    }

    async fn next(&mut self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        note(&self.log, "next");
        self.track = (self.track + 1) % TRACKS.len();
        let _ = self.metadata_changed(&emitter).await;
    }

    #[zbus(property)]
    fn playback_status(&self) -> &str {
        if self.playing { "Playing" } else { "Paused" }
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedVariant> {
        let (title, artist) = TRACKS[self.track];
        let owned = |value: Variant<'_>| value.try_to_owned().expect("plain values");
        HashMap::from([
            ("xesam:title".to_owned(), owned(Variant::from(title.to_owned()))),
            ("xesam:artist".to_owned(), owned(Variant::from(vec![artist.to_owned()]))),
        ])
    }

    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        true
    }
}

fn main() -> zbus::Result<()> {
    let log = std::env::args().nth(1).expect("usage: fake_player LOG");
    zbus::block_on(async {
        let bus = Connection::session().await?;
        bus.object_server().at("/org/mpris/MediaPlayer2", Root).await?;
        bus.object_server()
            .at(
                "/org/mpris/MediaPlayer2",
                Player {
                    log,
                    playing: true,
                    track: 0,
                },
            )
            .await?;
        bus.request_name("org.mpris.MediaPlayer2.fake").await?;
        std::future::pending::<()>().await;
        Ok(())
    })
}
