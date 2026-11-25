//! The text field's own part: its size, its text laid out, and how it
//! paints — the text or its dots, the placeholder, the selection, the caret
//! and a word about Caps Lock.

use unicode_segmentation::UnicodeSegmentation;

use super::{FieldEl, tree::State};
use crate::{
    app::Frame,
    geometry::Rect,
    text::{Paragraph, Style, Text},
    theme::{Theme, state},
};

/// Room inside the field around its text.
pub const PAD: f32 = 12.0;

/// What a password shows for each character.
const DOT: &str = "\u{2022}";

pub fn height(theme: &Theme) -> f32 {
    theme.sizes.control + 8.0
}

fn style(theme: &Theme) -> Style {
    Style {
        size: theme.sizes.body,
        weight: 400.0,
        family: None,
        tabular: false,
    }
}

/// What the field shows: its text, or a dot for each character of a
/// password.
pub fn shown<M>(field: &FieldEl<M>) -> String {
    if field.password {
        DOT.repeat(field.text.graphemes(true).count())
    } else {
        field.text.clone()
    }
}

/// A byte index in the text as one in what is shown.
pub fn to_shown<M>(field: &FieldEl<M>, index: usize) -> usize {
    if field.password {
        field.text[..index.min(field.text.len())].graphemes(true).count() * DOT.len()
    } else {
        index
    }
}

/// A byte index in what is shown as one in the text.
pub fn from_shown<M>(field: &FieldEl<M>, index: usize) -> usize {
    if field.password {
        let count = index / DOT.len();
        field
            .text
            .grapheme_indices(true)
            .nth(count)
            .map_or(field.text.len(), |(at, _)| at)
    } else {
        index.min(field.text.len())
    }
}

/// The text (or its dots) laid out on one line, and the placeholder.
pub fn paragraphs<M>(field: &FieldEl<M>, text: &mut Text, theme: &Theme, scale: f32) -> Vec<Paragraph> {
    vec![
        text.layout(&shown(field), &style(theme), scale, None, false),
        text.layout(&field.placeholder, &style(theme), scale, None, false),
        text.layout(
            "Caps Lock",
            &Style {
                size: theme.sizes.small,
                weight: 500.0,
                family: None,
                tabular: false,
            },
            scale,
            None,
            false,
        ),
    ]
}

/// Where its text starts across, scrolled by `scroll`.
pub fn text_x(rect: Rect, scroll: f32) -> f32 {
    rect.x + PAD - scroll
}

#[allow(clippy::too_many_arguments)]
pub fn paint<M>(
    field: &FieldEl<M>,
    rect: Rect,
    state: &State,
    paragraphs: &[Paragraph],
    focused: bool,
    caps: bool,
    frame: &mut Frame,
    theme: &Theme,
) {
    let colors = &theme.colors;
    let enabled = field.on_change.is_some();
    let radius = theme.sizes.radius_inner * 2.0;
    let editing = state.editing;
    frame.fill(rect, radius, colors.surface_container_highest);
    if enabled {
        frame.fill(rect, radius, colors.on_surface.alpha(state.hover.value * state::HOVER));
    }
    // The outline: thicker and in the primary color while editing.
    let (line, width) = if !enabled {
        (colors.on_surface.alpha(state::DISABLED_CONTAINER), 1.0)
    } else if editing {
        (colors.primary, 2.0)
    } else {
        (colors.outline, 1.0)
    };
    frame.ring(rect, radius, width, line);
    let [text, placeholder, caps_word] = paragraphs else {
        return;
    };
    // Caps Lock on, in a password: said at the end, which the text keeps clear of.
    let caps_room = if caps && field.password && editing {
        caps_word.width() + PAD
    } else {
        0.0
    };
    let inside = Rect::new(rect.x + PAD, rect.y, (rect.w - 2.0 * PAD - caps_room).max(0.0), rect.h);
    let outer = frame.clip_within(inside);
    let y = rect.y + (rect.h - text.height()) / 2.0;
    let x = text_x(rect, state.scroll_x);
    if field.text.is_empty() {
        frame.text(placeholder, rect.x + PAD, y, colors.on_surface_variant);
    } else {
        if editing && state.edit.has_selection() {
            let range = state.edit.selection();
            let range = to_shown(field, range.start)..to_shown(field, range.end);
            for piece in text.selection(range) {
                frame.fill(
                    Rect::new(x + piece.x, y + piece.y, piece.w, piece.h),
                    2.0,
                    colors.primary.alpha(0.35),
                );
            }
        }
        let ink = if enabled {
            colors.on_surface
        } else {
            colors.on_surface.alpha(state::DISABLED_CONTENT)
        };
        frame.text(text, x, y, ink);
    }
    if editing && state.caret_on {
        let caret = text.caret(to_shown(field, state.edit.cursor));
        let height = text.height().max(placeholder.height());
        frame.fill(Rect::new(x + caret.x - 1.0, y, 2.0, height), 1.0, colors.primary);
    }
    frame.clip(outer);
    if caps_room > 0.0 {
        let x = rect.right() - PAD - caps_word.width();
        let y = rect.y + (rect.h - caps_word.height()) / 2.0;
        frame.text(caps_word, x, y, colors.error);
    }
    if focused {
        super::kinds::ring(frame, rect, radius, state, theme);
    }
}
