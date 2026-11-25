//! The keyboard layout: its short name, as niri has it, and the next one on
//! a click or the wheel. Shown only where there is more than one.

use serde_json::json;
use yokoso_ui::el::{Edges, El, Size, Type};

use super::Msg;
use crate::niri::{self, State};

/// A layout's short name from its long one: "English (US)" is EN,
/// "Russian" RU. Languages whose code isn't their first letters are named
/// here; any other takes its first two letters.
pub fn short(name: &str) -> String {
    let language = name.split(['(', ',']).next().unwrap_or(name).trim();
    const CODES: [(&str, &str); 24] = [
        ("German", "DE"),
        ("Ukrainian", "UA"),
        ("Spanish", "ES"),
        ("Belarusian", "BY"),
        ("Kazakh", "KZ"),
        ("Japanese", "JA"),
        ("Chinese", "ZH"),
        ("Portuguese", "PT"),
        ("Czech", "CZ"),
        ("Turkish", "TR"),
        ("Greek", "GR"),
        ("Hebrew", "HE"),
        ("Korean", "KO"),
        ("Georgian", "KA"),
        ("Armenian", "HY"),
        ("Swedish", "SE"),
        ("Danish", "DK"),
        ("Dutch", "NL"),
        ("Serbian", "SR"),
        ("Croatian", "HR"),
        ("Lithuanian", "LT"),
        ("Latvian", "LV"),
        ("Estonian", "ET"),
        ("Slovenian", "SL"),
    ];
    if let Some((_, code)) = CODES.iter().find(|(long, _)| *long == language) {
        return (*code).to_owned();
    }
    language
        .chars()
        .filter(|c| c.is_alphabetic())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

/// The part; None with one layout or none.
pub fn view(niri: &State, pad: Edges) -> Option<El<Msg>> {
    let layouts = &niri.layouts;
    if layouts.names.len() < 2 {
        return None;
    }
    let name = layouts.names.get(layouts.current_idx)?;
    Some(
        El::press(
            0.0,
            vec![El::text(short(name), Type::Body).weight(600.0)],
            Some(Msg::LayoutStep(-1.0)),
        )
        .on_scroll(Msg::LayoutStep)
        .key("layout")
        .padding(pad)
        .height(Size::Fill(1.0))
        .tooltip(name.clone()),
    )
}

/// The layout before, up the wheel; the next one, down it or on a click.
pub fn act(steps: f32) {
    let layout = if steps > 0.0 { "Prev" } else { "Next" };
    niri::act(json!({ "SwitchLayout": { "layout": layout } }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_go_by_their_language_code() {
        assert_eq!(short("English (US)"), "EN");
        assert_eq!(short("English (US, intl., with dead keys)"), "EN");
        assert_eq!(short("Russian"), "RU");
        assert_eq!(short("German (Switzerland)"), "DE");
        assert_eq!(short("Ukrainian"), "UA");
        assert_eq!(short("French"), "FR");
    }
}
