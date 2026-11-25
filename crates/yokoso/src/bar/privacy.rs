//! Privacy: who records now. A microphone while an app records sound, a
//! camera while one runs, a screen while the screen is cast; in the
//! warning color, and the apps named on resting. Nothing shows when
//! nothing records. While an app records, a click mutes the microphone or
//! has it back, and the wheel turns it; the middle button opens what the
//! config says.

use yokoso_ui::{
    el::{Edges, El, Ink, Size},
    theme::Theme,
};

use super::Msg;
use crate::pw::Use;

/// What it says on resting; `muted`, the microphone.
pub fn about(using: &[Use], casts: usize, muted: bool) -> String {
    let mut said = Vec::new();
    let apps: Vec<&str> = using
        .iter()
        .filter_map(|used| match used {
            Use::Microphone(app) => Some(app.as_str()),
            Use::Camera(_) => None,
        })
        .collect();
    if !apps.is_empty() {
        let state = if muted { ", muted" } else { "" };
        said.push(format!("Microphone{state}: {}", apps.join(", ")));
    }
    let cameras: Vec<&str> = using
        .iter()
        .filter_map(|used| match used {
            Use::Camera(name) => Some(name.as_str()),
            Use::Microphone(_) => None,
        })
        .collect();
    if !cameras.is_empty() {
        said.push(format!("Camera: {}", cameras.join(", ")));
    }
    if casts > 0 {
        said.push("The screen is shared".into());
    }
    if !apps.is_empty() {
        said.push(
            if muted {
                "A click has it back"
            } else {
                "A click mutes it"
            }
            .into(),
        );
    }
    said.join(" \u{b7} ")
}

/// The part; None while nothing records. `muted`, the microphone;
/// `on_middle`, what the config says it opens.
pub fn view(
    using: &[Use],
    casts: usize,
    muted: bool,
    theme: &Theme,
    pad: Edges,
    on_middle: Option<Msg>,
) -> Option<El<Msg>> {
    let warning = Ink::Own(theme.colors.tertiary);
    let listened = using.iter().any(|used| matches!(used, Use::Microphone(_)));
    let mut icons = Vec::new();
    if listened {
        icons.push(El::icon(if muted { "mic-off" } else { "mic" }).ink(warning));
    }
    if using.iter().any(|used| matches!(used, Use::Camera(_))) {
        icons.push(El::icon("camera").ink(warning));
    }
    if casts > 0 {
        icons.push(El::icon("screen").ink(warning));
    }
    if icons.is_empty() {
        return None;
    }
    let mut part = El::press(4.0, icons, listened.then_some(Msg::MicMute));
    if listened {
        part = part.on_scroll(Msg::MicVolume);
    }
    if let Some(open) = on_middle {
        part = part.on_middle(open);
    }
    Some(
        part.key("privacy")
            .padding(pad)
            .height(Size::Fill(1.0))
            .tooltip(about(using, casts, muted)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_names_who_records_and_what_a_click_does() {
        let using = [Use::Microphone("Telegram".into()), Use::Microphone("Firefox".into())];
        assert_eq!(
            about(&using, 0, false),
            "Microphone: Telegram, Firefox \u{b7} A click mutes it"
        );
        assert_eq!(
            about(&using[..1], 0, true),
            "Microphone, muted: Telegram \u{b7} A click has it back"
        );
        assert_eq!(
            about(&[Use::Camera("Webcam".into())], 1, false),
            "Camera: Webcam \u{b7} The screen is shared"
        );
    }
}
