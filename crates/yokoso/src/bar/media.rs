//! The media part: there while a player has a track. A note in the accent
//! while it plays, a pause mark while it doesn't; the track's title beside.
//! A click plays or pauses, the middle button goes to the next track.

use yokoso_ui::{
    el::{Edges, El, Ink, Size, Type},
    theme::Theme,
};

use super::Msg;
use crate::mpris::Playing;

/// Titles longer than this many characters are cut, so the bar keeps room.
const LONGEST: usize = 32;

/// `text` cut to `LONGEST` characters, an ellipsis where it was.
pub fn cut(text: &str) -> String {
    if text.chars().count() <= LONGEST {
        return text.to_owned();
    }
    let kept: String = text.chars().take(LONGEST - 1).collect();
    format!("{}\u{2026}", kept.trim_end())
}

/// What it says on resting.
pub fn about(player: &Playing) -> String {
    let track = match (player.artist.is_empty(), player.title.is_empty()) {
        (false, false) => format!("{} \u{2014} {}", player.artist, player.title),
        (true, false) => player.title.clone(),
        (false, true) => player.artist.clone(),
        (true, true) => "Something".into(),
    };
    let state = if player.playing { "" } else { ", paused" };
    if player.identity.is_empty() {
        format!("{track}{state}")
    } else {
        format!("{track} \u{b7} {}{state}", player.identity)
    }
}

/// The part; None with nothing playing or paused.
pub fn view(player: Option<&Playing>, theme: &Theme, pad: Edges) -> Option<El<Msg>> {
    let player = player?;
    let faint = Ink::Own(theme.colors.on_surface.alpha(0.6));
    let (icon, ink) = if player.playing {
        ("music", Ink::Primary)
    } else {
        ("pause", faint)
    };
    let title = if player.title.is_empty() {
        &player.identity
    } else {
        &player.title
    };
    let text_ink = if player.playing { Ink::OnSurface } else { faint };
    let mut part = El::press(
        6.0,
        vec![El::icon(icon).ink(ink), El::text(cut(title), Type::Body).ink(text_ink)],
        Some(Msg::PlayPause),
    );
    if player.can_next {
        part = part.on_middle(Msg::NextTrack);
    }
    Some(
        part.key("media")
            .padding(pad)
            .height(Size::Fill(1.0))
            .tooltip(about(player)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_titles_are_cut_and_the_track_is_named_in_full_on_resting() {
        assert_eq!(cut("Short"), "Short");
        let long = "A very long title of a song that goes on and on";
        assert_eq!(cut(long).chars().count(), LONGEST);
        assert!(cut(long).ends_with('\u{2026}'));
        let player = Playing {
            identity: "mpv".into(),
            title: "Song".into(),
            artist: "Band".into(),
            playing: false,
            ..Playing::default()
        };
        assert_eq!(about(&player), "Band \u{2014} Song \u{b7} mpv, paused");
    }
}
