//! What each kind of element is: its role, its size, its text, and how it
//! paints in its state. The tree calls these; they never change state.

use super::{ButtonStyle, El, Ink, Kind, TextEl, Type, tree::Role, tree::State};
use crate::{
    app::Frame,
    geometry::{Color, Rect},
    text::{Paragraph, Style, Text},
    theme::{Theme, state},
};

pub fn role<M>(kind: &Kind<M>) -> Role {
    match kind {
        Kind::Text(_) => Role::Text,
        Kind::Button { .. } => Role::Button,
        _ => Role::Group,
    }
}

/// Whether it takes the pointer and the keyboard.
pub fn interactive<M>(kind: &Kind<M>) -> bool {
    matches!(kind, Kind::Button { on_press: Some(_), .. })
}

/// Text's style from the theme's tokens.
pub fn style(text: &TextEl, theme: &Theme) -> Style {
    let sizes = &theme.sizes;
    let size = match text.ty {
        Type::Body => sizes.body,
        Type::Label => sizes.label,
        Type::Small => sizes.small,
        Type::Title => sizes.title,
        Type::Headline => sizes.headline,
    };
    Style {
        size,
        weight: text.weight,
        family: None,
        tabular: text.tabular,
    }
}

pub fn ink(ink: Ink, theme: &Theme) -> Color {
    let colors = &theme.colors;
    match ink {
        Ink::OnSurface => colors.on_surface,
        Ink::OnSurfaceVariant => colors.on_surface_variant,
        Ink::Primary => colors.primary,
        Ink::Error => colors.error,
        Ink::Own(color) => color,
    }
}

fn label_style(theme: &Theme) -> Style {
    Style {
        size: theme.sizes.body,
        weight: 500.0,
        family: None,
        tabular: false,
    }
}

/// A button's room on either side of its label.
fn button_pad(theme: &Theme) -> f32 {
    theme.sizes.pad * 1.5
}

/// How large a leaf wants to be, given no more than `max` across.
pub fn measure<M>(el: &El<M>, max: Option<f32>, text: &mut Text, theme: &Theme, scale: f32) -> (f32, f32) {
    match &el.kind {
        Kind::Text(t) => {
            let wrap = if t.ellipsis { None } else { max };
            let laid = text.layout(&t.text, &style(t, theme), scale, wrap, false);
            (laid.width().ceil().min(max.unwrap_or(f32::MAX)), laid.height().ceil())
        }
        Kind::Button { label, .. } => {
            let laid = text.layout(label, &label_style(theme), scale, None, false);
            let width = laid.width().ceil() + 2.0 * button_pad(theme);
            (width.min(max.unwrap_or(f32::MAX)), theme.sizes.control)
        }
        _ => (0.0, 0.0),
    }
}

/// Its text, laid out for the rect it got.
pub fn paragraph<M>(el: &El<M>, rect: Rect, text: &mut Text, theme: &Theme, scale: f32) -> Option<Paragraph> {
    match &el.kind {
        Kind::Text(t) => Some(text.layout(&t.text, &style(t, theme), scale, Some(rect.w + 0.5), t.ellipsis)),
        Kind::Button { label, .. } => {
            let room = (rect.w - 2.0 * button_pad(theme)).max(0.0) + 0.5;
            Some(text.layout(label, &label_style(theme), scale, Some(room), true))
        }
        _ => None,
    }
}

/// Paints `el` in `rect`, as `state` and `focus_visible` have it.
pub fn paint<M>(
    el: &El<M>,
    rect: Rect,
    state: &State,
    paragraph: Option<&Paragraph>,
    focused: bool,
    frame: &mut Frame,
    theme: &Theme,
) {
    let colors = &theme.colors;
    match &el.kind {
        Kind::Text(t) => {
            if let Some(paragraph) = paragraph {
                // Centered up and down: a line in a taller box.
                let y = rect.y + ((rect.h - paragraph.height()) / 2.0).max(0.0);
                frame.text(paragraph, rect.x, y, ink(t.ink, theme));
            }
        }
        Kind::Button { style, on_press, .. } => {
            let enabled = on_press.is_some();
            let (container, content) = match style {
                ButtonStyle::Filled => (Some(colors.primary), colors.on_primary),
                ButtonStyle::Tonal => (Some(colors.secondary_container), colors.on_secondary_container),
                ButtonStyle::Outlined | ButtonStyle::Text => (None, colors.primary),
            };
            let (container, content) = if enabled {
                (container, content)
            } else {
                (
                    container.map(|_| colors.on_surface.alpha(state::DISABLED_CONTAINER)),
                    colors.on_surface.alpha(state::DISABLED_CONTENT),
                )
            };
            // Pressed, a pill straightens its corners a little.
            let press = state.press.value.clamp(0.0, 1.2);
            let radius = rect.h / 2.0 * (1.0 - 0.35 * press);
            if let Some(container) = container {
                frame.fill(rect, radius, container);
            }
            if *style == ButtonStyle::Outlined {
                let line = if enabled {
                    colors.outline_variant
                } else {
                    colors.on_surface.alpha(state::DISABLED_CONTAINER)
                };
                frame.ring(rect, radius, 1.0, line);
            }
            if enabled {
                let layer = state.hover.value * state::HOVER + press * state::PRESS;
                frame.fill(rect, radius, content.alpha(layer.clamp(0.0, 1.0)));
            }
            if focused {
                ring(frame, rect, radius, state, theme);
            }
            if let Some(paragraph) = paragraph {
                let x = rect.x + (rect.w - paragraph.width()) / 2.0;
                let y = rect.y + (rect.h - paragraph.height()) / 2.0;
                frame.text(paragraph, x, y, content);
            }
        }
        _ => {}
    }
}

/// The focus ring: outside the element, as far as its spring has come.
pub fn ring(frame: &mut Frame, rect: Rect, radius: f32, state: &State, theme: &Theme) {
    let shown = state.focus.value.clamp(0.0, 1.0);
    if shown <= 0.0 {
        return;
    }
    let width = theme.sizes.focus_ring;
    let gap = 2.0;
    let outer = rect.grow(gap + width);
    frame.ring(outer, radius + gap + width, width, theme.colors.secondary.alpha(shown));
}
