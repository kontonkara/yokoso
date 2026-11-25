//! Privacy: who records now. A microphone while an app records sound, a
//! camera while one runs, a screen while the screen is cast; in the
//! warning color, and the apps named on resting. Nothing shows when
//! nothing records.

use yokoso_ui::{
    el::{Edges, El, Ink, Size},
    theme::Theme,
};

use super::Msg;
use crate::pw::Use;

/// What it says on resting.
pub fn about(using: &[Use], casts: usize) -> String {
    let mut said = Vec::new();
    let apps: Vec<&str> = using
        .iter()
        .filter_map(|used| match used {
            Use::Microphone(app) => Some(app.as_str()),
            Use::Camera(_) => None,
        })
        .collect();
    if !apps.is_empty() {
        said.push(format!("Microphone: {}", apps.join(", ")));
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
    said.join(" \u{b7} ")
}

/// The part; None while nothing records.
pub fn view(using: &[Use], casts: usize, theme: &Theme, pad: Edges) -> Option<El<Msg>> {
    let warning = Ink::Own(theme.colors.tertiary);
    let mut icons = Vec::new();
    if using.iter().any(|used| matches!(used, Use::Microphone(_))) {
        icons.push(El::icon("mic").ink(warning));
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
    Some(
        El::press(4.0, icons, None)
            .key("privacy")
            .padding(pad)
            .height(Size::Fill(1.0))
            .tooltip(about(using, casts)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_names_who_records() {
        let using = [Use::Microphone("Telegram".into()), Use::Microphone("Firefox".into())];
        assert_eq!(about(&using, 0), "Microphone: Telegram, Firefox");
        assert_eq!(
            about(&[Use::Camera("Webcam".into())], 1),
            "Camera: Webcam \u{b7} The screen is shared"
        );
    }
}
