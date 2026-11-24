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
