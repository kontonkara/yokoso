//! Text: laid out by Parley, with fonts and fallbacks as fontconfig has them,
//! rasterized by swash and kept in the atlas, a glyph at a size each.
//!
//! A paragraph is laid out once, in layout, at its surface's scale, so its
//! glyphs sit on that scale's pixels; render only draws it.

use std::collections::HashMap;

use parley::{
    FontContext, FontFamily, FontFeatures, FontWeight, Layout, LayoutContext, PositionedLayoutItem, StyleProperty,
};
use swash::{
    FontRef,
    scale::{Render, ScaleContext, Source, StrikeWith, image::Content},
    zeno::{Format, Vector},
};

use crate::{
    geometry::{Color, Rect},
    paint::Painter,
};

/// How a piece of text looks.
#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    /// Logical pixels.
    pub size: f32,
    /// 100 to 900; 400 is regular.
    pub weight: f32,
    /// A family by name; the system's sans-serif when None.
    pub family: Option<String>,
    /// Digits all one width, so numbers that change don't shift.
    pub tabular: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            size: 14.0,
            weight: 400.0,
            family: None,
            tabular: false,
        }
    }
}

/// Text laid out at one scale: drawn as often as wanted.
pub struct Paragraph {
    layout: Layout<()>,
    scale: f32,
}

impl Paragraph {
    /// Logical pixels.
    pub fn width(&self) -> f32 {
        self.layout.width() / self.scale
    }

    pub fn height(&self) -> f32 {
        self.layout.height() / self.scale
    }

    /// The caret before the byte `index`, logical pixels from the top left.
    pub fn caret(&self, index: usize) -> Rect {
        let cursor = parley::Cursor::from_byte_index(&self.layout, index, parley::Affinity::Downstream);
        let found = cursor.geometry(&self.layout, 1.0);
        let s = self.scale as f64;
        Rect::new(
            (found.x0 / s) as f32,
            (found.y0 / s) as f32,
            1.0,
            ((found.y1 - found.y0) / s) as f32,
        )
    }

    /// The byte index nearest `x`, `y` logical pixels from the top left.
    pub fn index_at(&self, x: f32, y: f32) -> usize {
        parley::Cursor::from_point(&self.layout, x * self.scale, y * self.scale).index()
    }

    /// Where `range` of bytes shows, as rects from the top left.
    pub fn selection(&self, range: std::ops::Range<usize>) -> Vec<Rect> {
        if range.is_empty() {
            return Vec::new();
        }
        let from = parley::Cursor::from_byte_index(&self.layout, range.start, parley::Affinity::Downstream);
        let to = parley::Cursor::from_byte_index(&self.layout, range.end, parley::Affinity::Upstream);
        let s = self.scale as f64;
        parley::Selection::new(from, to)
            .geometry(&self.layout)
            .into_iter()
            .map(|(found, _)| {
                Rect::new(
                    (found.x0 / s) as f32,
                    (found.y0 / s) as f32,
                    ((found.x1 - found.x0) / s) as f32,
                    ((found.y1 - found.y0) / s) as f32,
                )
            })
            .collect()
    }

    /// The scale it was laid out at: drawn at another, it is laid out again.
    pub fn scale(&self) -> f32 {
        self.scale
    }
}

/// A glyph as the atlas has it: where its picture sits around its origin,
/// and its place there.
#[derive(Clone, Copy)]
struct Glyph {
    left: i32,
    top: i32,
    width: i32,
    height: i32,
    uv: [f32; 4],
    colored: bool,
    generation: u64,
}

/// A glyph of a font at a size, a quarter pixel along.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphKey {
    font: u64,
    index: u32,
    glyph: u32,
    size: u32,
    quarter: u8,
    coords: u64,
}

const ELLIPSIS: &str = "\u{2026}";

pub struct Text {
    fonts: FontContext,
    layouts: LayoutContext<()>,
    scaler: ScaleContext,
    glyphs: HashMap<GlyphKey, Option<Glyph>>,
}

impl Default for Text {
    fn default() -> Self {
        Self::new()
    }
}

impl Text {
    pub fn new() -> Self {
        Self {
            fonts: FontContext::new(),
            layouts: LayoutContext::new(),
            scaler: ScaleContext::new(),
            glyphs: HashMap::new(),
        }
    }

    /// `text` laid out at `scale`, wrapped at `width` logical pixels where
    /// that's given; with `ellipsis`, kept to one line that ends in "…" where
    /// it doesn't fit.
    pub fn layout(&mut self, text: &str, style: &Style, scale: f32, width: Option<f32>, ellipsis: bool) -> Paragraph {
        let max = width.map(|width| width * scale);
        let mut layout = self.build(text, style, scale);
        if ellipsis {
            layout.break_all_lines(None);
            if let Some(max) = max
                && layout.width() > max
            {
                let shown = self.cut(&layout, text, style, scale, max);
                layout = self.build(&shown, style, scale);
                layout.break_all_lines(None);
            }
        } else {
            layout.break_all_lines(max);
        }
        Paragraph { layout, scale }
    }

    fn build(&mut self, text: &str, style: &Style, scale: f32) -> Layout<()> {
        let mut builder = self.layouts.ranged_builder(&mut self.fonts, text, scale, true);
        builder.push_default(StyleProperty::FontSize(style.size));
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(style.weight)));
        let family = style.family.as_deref().unwrap_or("sans-serif");
        builder.push_default(StyleProperty::FontFamily(FontFamily::from(family)));
        // Emoji from a font of their own, in color. Left to the family's
        // fallbacks they go to the first font that has them at all, often in
        // plain outlines; and an emoji font first would take digits too.
        for range in emoji_ranges(text) {
            builder.push(StyleProperty::FontFamily(FontFamily::from("emoji")), range);
        }
        if style.tabular {
            builder.push_default(StyleProperty::FontFeatures(FontFeatures::from("\"tnum\" 1")));
        }
        builder.build(text)
    }

    /// As much of `text` as fits `max` buffer pixels with an ellipsis after
    /// it, cut between clusters, never in one.
    fn cut(&mut self, layout: &Layout<()>, text: &str, style: &Style, scale: f32, max: f32) -> String {
        let mut room = self.build(ELLIPSIS, style, scale);
        room.break_all_lines(None);
        let room = max - room.width();
        let mut used = 0.0;
        let mut end = 0;
        if let Some(line) = layout.lines().next() {
            'runs: for run in line.runs() {
                for cluster in run.clusters() {
                    used += cluster.advance();
                    if used > room {
                        break 'runs;
                    }
                    end = end.max(cluster.text_range().end);
                }
            }
        }
        let mut end = end.min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}{ELLIPSIS}", text[..end].trim_end())
    }

    /// Draws `paragraph` with its top left at `x`, `y` buffer pixels.
    pub(crate) fn draw(&mut self, painter: &mut Painter, paragraph: &Paragraph, x: f32, y: f32, color: Color) {
        let (origin_x, origin_y) = (x.round(), y.round());
        for line in paragraph.layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let run = glyph_run.run();
                let font = run.font();
                let size = run.font_size();
                let coords: Vec<i16> = run.normalized_coords().iter().map(|c| c.to_bits()).collect();
                let coords_hash = coords
                    .iter()
                    .fold(0u64, |hash, c| hash.wrapping_mul(31).wrapping_add(*c as u16 as u64));
                let baseline = origin_y + glyph_run.baseline();
                let mut pen = origin_x + glyph_run.offset();
                for glyph in glyph_run.glyphs() {
                    let at = pen + glyph.x;
                    pen += glyph.advance;
                    let quarter = ((at.fract() * 4.0).floor() as u8).min(3);
                    let key = GlyphKey {
                        font: font.data.id(),
                        index: font.index,
                        glyph: glyph.id,
                        size: size.to_bits(),
                        quarter,
                        coords: coords_hash,
                    };
                    let Some(found) = self.glyph(painter, key, font, size, &coords) else {
                        continue;
                    };
                    let rect = Rect::new(
                        at.floor() + found.left as f32,
                        (baseline - glyph.y).round() - found.top as f32,
                        found.width as f32,
                        found.height as f32,
                    );
                    painter.glyph(rect, found.uv, color, found.colored);
                }
            }
        }
    }

    fn glyph(
        &mut self,
        painter: &mut Painter,
        key: GlyphKey,
        font: &parley::FontData,
        size: f32,
        coords: &[i16],
    ) -> Option<Glyph> {
        if let Some(known) = self.glyphs.get(&key) {
            match known {
                Some(glyph) if glyph.generation == painter.atlas.generation => return Some(*glyph),
                None => return None,
                // The atlas started over since: drawn into it again.
                Some(_) => {}
            }
        }
        let font_ref = FontRef::from_index(font.data.as_ref(), font.index as usize)?;
        let mut scaler = self
            .scaler
            .builder(font_ref)
            .size(size)
            .hint(true)
            .normalized_coords(coords)
            .build();
        let image = Render::new(&[
            Source::ColorOutline(0),
            Source::ColorBitmap(StrikeWith::BestFit),
            Source::Outline,
        ])
        .format(Format::Alpha)
        .offset(Vector::new(key.quarter as f32 / 4.0, 0.0))
        .render(&mut scaler, key.glyph as u16);
        let before = painter.atlas.generation;
        let made = image.and_then(|image| {
            let (width, height) = (image.placement.width as i32, image.placement.height as i32);
            if width == 0 || height == 0 {
                return None;
            }
            let (rgba, colored) = match image.content {
                Content::Mask => (image.data.iter().flat_map(|&a| [255, 255, 255, a]).collect(), false),
                Content::Color => (premultiply(&image.data), true),
                // Coverage for each third of a pixel: as a plain mask, by its middle.
                Content::SubpixelMask => (
                    image
                        .data
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .flat_map(|p| [255, 255, 255, p[1]])
                        .collect(),
                    false,
                ),
            };
            let uv = painter.atlas.add(width, height, rgba)?;
            Some(Glyph {
                left: image.placement.left,
                top: image.placement.top,
                width,
                height,
                uv,
                colored,
                generation: before,
            })
        });
        if painter.atlas.generation != before {
            // The atlas was full and started over: every glyph goes, and
            // this one is tried again on the next frame, which draws all.
            self.glyphs.clear();
            return None;
        }
        self.glyphs.insert(key, made);
        made
    }
}

/// The byte ranges of `text` that are emoji: pictographs, flags, and
/// symbols asked to show as emoji (U+FE0F), with what joins them to the
/// next and what modifies them.
pub fn emoji_ranges(text: &str) -> Vec<std::ops::Range<usize>> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();
    for (index, &(at, c)) in chars.iter().enumerate() {
        let code = c as u32;
        let asked = chars.get(index + 1).is_some_and(|(_, next)| *next == '\u{fe0f}');
        let emoji = matches!(code, 0x1f000..=0x1faff)
            || (matches!(code, 0x2600..=0x27bf | 0x2300..=0x23ff | 0x2b00..=0x2bff) && asked);
        let joining = matches!(code, 0x200d | 0xfe0f | 0x20e3 | 0x1f3fb..=0x1f3ff | 0xe0020..=0xe007f);
        let end = at + c.len_utf8();
        match ranges.last_mut() {
            Some(last) if (emoji || joining) && last.end == at => last.end = end,
            _ if emoji => ranges.push(at..end),
            _ => {}
        }
    }
    ranges
}

fn premultiply(data: &[u8]) -> Vec<u8> {
    data.as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| {
            let a = p[3] as u16;
            [
                (p[0] as u16 * a / 255) as u8,
                (p[1] as u16 * a / 255) as u8,
                (p[2] as u16 * a / 255) as u8,
                p[3],
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::emoji_ranges;

    #[test]
    fn emoji_are_found_with_what_joins_them_and_digits_are_not() {
        let text = "ok 🔥 12:30 👩‍💻 ✓ ☀️";
        let found: Vec<&str> = emoji_ranges(text).into_iter().map(|range| &text[range]).collect();
        assert_eq!(found, ["🔥", "👩‍💻", "☀️"]);
    }

    #[test]
    fn flags_and_skin_tones_stay_whole() {
        let text = "🇺🇦👍🏽";
        let found: Vec<&str> = emoji_ranges(text).into_iter().map(|range| &text[range]).collect();
        assert_eq!(found, ["🇺🇦👍🏽"]);
    }
}
