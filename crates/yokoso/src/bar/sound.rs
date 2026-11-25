//! The sound: the default output's volume as waves, crossed out muted, the
//! percentage beside it only past 100. The wheel turns it, the middle
//! button mutes it.

use yokoso_ui::{
    el::{Edges, El, Ink, Size, Type},
    theme::Theme,
};

use super::Msg;
use crate::pw::{Output, Sound};

/// The icon for `output`: fewer waves, quieter.
pub fn icon(output: &Output) -> &'static str {
    match output.volume {
        _ if output.muted => "volume-x",
        v if v <= 0.0 => "volume-x",
        v if v < 0.34 => "volume-0",
        v if v < 0.67 => "volume-1",
        _ => "volume",
    }
}

/// What it says on resting.
pub fn about(sound: &Sound) -> String {
    match sound {
        Sound::Unknown => "No word from PipeWire".into(),
        Sound::Silent => "No output to play to".into(),
        Sound::Out(output) => {
            let name = if output.description.is_empty() {
                &output.name
            } else {
                &output.description
            };
            let percent = (output.volume * 100.0).round();
            if output.muted {
                format!("{name}: muted ({percent}%)")
            } else {
                format!("{name}: {percent}%")
            }
        }
    }
}

/// The part: always there, as no sound is worth seeing too.
pub fn view(sound: &Sound, theme: &Theme, pad: Edges, on_click: Option<Msg>) -> El<Msg> {
    let faint = Ink::Own(theme.colors.on_surface.alpha(0.3));
    let parts = match sound {
        Sound::Unknown | Sound::Silent => vec![El::icon("volume-x").ink(faint)],
        Sound::Out(output) => {
            let mut parts = vec![El::icon(icon(output))];
            if output.volume > 1.0 && !output.muted {
                parts.push(
                    El::text(format!("{:.0}%", output.volume * 100.0), Type::Body)
                        .weight(600.0)
                        .tabular()
                        .ink(Ink::Own(theme.colors.tertiary)),
                );
            }
            parts
        }
    };
    let mut part = El::press(4.0, parts, on_click);
    if matches!(sound, Sound::Out(_)) {
        part = part.on_middle(Msg::Mute).on_scroll(Msg::Volume);
    }
    part.key("sound")
        .padding(pad)
        .height(Size::Fill(1.0))
        .tooltip(about(sound))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(volume: f32, muted: bool) -> Output {
        Output {
            name: "alsa".into(),
            description: "Speakers".into(),
            volume,
            muted,
        }
    }

    #[test]
    fn waves_go_with_the_volume_and_muted_is_crossed_out() {
        assert_eq!(icon(&out(0.9, false)), "volume");
        assert_eq!(icon(&out(0.5, false)), "volume-1");
        assert_eq!(icon(&out(0.2, false)), "volume-0");
        assert_eq!(icon(&out(0.0, false)), "volume-x");
        assert_eq!(icon(&out(0.9, true)), "volume-x");
    }

    #[test]
    fn it_names_the_output_and_its_volume() {
        assert_eq!(about(&Sound::Out(out(0.5, false))), "Speakers: 50%");
        assert_eq!(about(&Sound::Out(out(0.5, true))), "Speakers: muted (50%)");
        assert_eq!(about(&Sound::Unknown), "No word from PipeWire");
    }
}
