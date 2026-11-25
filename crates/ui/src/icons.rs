//! The shell's own icons: simple strokes on a 24 pixel grid, drawn at the
//! size and scale asked, kept in the atlas as masks and painted in any color
//! of the theme.

use std::collections::HashMap;

use crate::{
    atlas::Uv,
    geometry::{Color, Rect},
    paint::Painter,
};

/// Each icon: its name and the inside of its SVG, strokes 2 wide, round.
const SET: &[(&str, &str)] = &[
    ("chevron-right", r#"<path d="M9 6l6 6-6 6"/>"#),
    ("chevron-down", r#"<path d="M6 9l6 6 6-6"/>"#),
    ("chevron-left", r#"<path d="M15 6l-6 6 6 6"/>"#),
    ("chevron-up", r#"<path d="M6 15l6-6 6 6"/>"#),
    ("check", r#"<path d="M5 12.5l4.5 4.5L19 7.5"/>"#),
    ("close", r#"<path d="M6 6l12 12M18 6L6 18"/>"#),
    ("plus", r#"<path d="M12 5v14M5 12h14"/>"#),
    ("minus", r#"<path d="M5 12h14"/>"#),
    (
        "wifi",
        r#"<path d="M2.5 9a14 14 0 0 1 19 0M5.5 12.5a9.5 9.5 0 0 1 13 0M8.5 16a5 5 0 0 1 7 0"/><circle cx="12" cy="19.5" r="1.2" fill="black" stroke="none"/>"#,
    ),
    ("bluetooth", r#"<path d="M7 7l10 10-5 5V2l5 5L7 17"/>"#),
    (
        "moon",
        r#"<path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z"/>"#,
    ),
    (
        "sun",
        r#"<circle cx="12" cy="12" r="4"/><path d="M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.3 5.3l1.4 1.4M17.3 17.3l1.4 1.4M5.3 18.7l1.4-1.4M17.3 6.7l1.4-1.4"/>"#,
    ),
    (
        "bell",
        r#"<path d="M6 9.5a6 6 0 0 1 12 0c0 5.5 2.5 7 2.5 7h-17S6 15 6 9.5zM10 20a2 2 0 0 0 4 0"/>"#,
    ),
    (
        "volume",
        r#"<path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4z"/><path d="M15.5 9a4 4 0 0 1 0 6M18.5 6a8 8 0 0 1 0 12"/>"#,
    ),
    (
        "search",
        r#"<circle cx="11" cy="11" r="6.5"/><path d="M16 16l4.5 4.5"/>"#,
    ),
    ("power", r#"<path d="M12 3v8M6.6 6.6a8 8 0 1 0 10.8 0"/>"#),
    (
        "battery",
        r#"<rect x="2.5" y="7" width="16.5" height="10" rx="2.5"/><path d="M21.5 10.5v3"/>"#,
    ),
    ("bolt", r#"<path d="M13.5 3L6 13h5.5l-1 8L18 11h-5.5z"/>"#),
];

/// The names of every icon there is.
pub fn names() -> impl Iterator<Item = &'static str> {
    SET.iter().map(|(name, _)| *name)
}

#[derive(Clone, Copy)]
struct Drawn {
    uv: Uv,
    size: i32,
    generation: u64,
}

#[derive(Default)]
pub struct Icons {
    drawn: HashMap<(&'static str, i32), Option<Drawn>>,
}

impl Icons {
    /// Paints `name` filling `rect`, buffer pixels, in `color`, `scale` of
    /// its size about its middle: drawn once at its full size, the picture
    /// scaled. An unknown name paints nothing.
    pub fn paint(&mut self, painter: &mut Painter, name: &str, rect: Rect, color: Color, scale: f32) {
        let size = rect.w.min(rect.h).round() as i32;
        if size <= 0 {
            return;
        }
        let Some((key, body)) = SET.iter().find(|(known, _)| *known == name).map(|(k, b)| (*k, *b)) else {
            return;
        };
        let known = self.drawn.get(&(key, size)).copied();
        let drawn = match known {
            Some(Some(drawn)) if drawn.generation == painter.atlas.generation => Some(drawn),
            Some(None) => None,
            _ => {
                let before = painter.atlas.generation;
                let made = draw(body, size).and_then(|mask| {
                    let uv = painter.atlas.add(size, size, mask)?;
                    Some(Drawn {
                        uv,
                        size,
                        generation: before,
                    })
                });
                if painter.atlas.generation != before {
                    // The atlas started over: tried again on the next frame.
                    self.drawn.clear();
                    return;
                }
                self.drawn.insert((key, size), made);
                made
            }
        };
        if let Some(drawn) = drawn {
            let full = drawn.size as f32;
            let x = (rect.x + (rect.w - full) / 2.0).round();
            let y = (rect.y + (rect.h - full) / 2.0).round();
            let side = full * scale.max(0.0);
            let at = if scale == 1.0 {
                Rect::new(x, y, full, full)
            } else {
                Rect::new(x + (full - side) / 2.0, y + (full - side) / 2.0, side, side)
            };
            painter.glyph(at, drawn.uv, color, false);
        }
    }
}

/// The icon `body` drawn `size` pixels square: white with its coverage in
/// alpha, as the atlas keeps masks.
fn draw(body: &str, size: i32) -> Option<Vec<u8>> {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{body}</svg>"#
    );
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size as u32, size as u32)?;
    let scale = size as f32 / 24.0;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Some(
        pixmap
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [255, 255, 255, p[3]])
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_draws_and_covers_some_of_its_square() {
        for (name, body) in SET {
            let mask = draw(body, 24).unwrap_or_else(|| panic!("{name} did not draw"));
            let covered = mask.as_chunks::<4>().0.iter().filter(|p| p[3] > 128).count();
            assert!(covered > 20 && covered < 24 * 24 / 2, "{name}: {covered}");
        }
    }
}
