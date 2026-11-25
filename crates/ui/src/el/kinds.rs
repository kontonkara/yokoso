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
        Kind::Toggle { .. } => Role::Toggle,
        _ => Role::Group,
    }
}

/// Whether it takes the pointer and the keyboard.
pub fn interactive<M>(kind: &Kind<M>) -> bool {
    matches!(
        kind,
        Kind::Button { on_press: Some(_), .. } | Kind::Toggle { on_toggle: Some(_), .. }
    )
}

/// Whether it waits on something, pulsing meanwhile.
pub fn pending<M>(kind: &Kind<M>) -> bool {
    matches!(kind, Kind::Toggle { pending: true, .. })
}

/// Aims its springs at what the description says: a toggle's thumb at its
/// side. The first time there, at once.
pub fn settle<M>(kind: &Kind<M>, state: &mut State) {
    if let Kind::Toggle { on, .. } = kind {
        let target = if *on { 1.0 } else { 0.0 };
        if state.known {
            state.value.to(target);
        } else {
            state.value.snap(target);
        }
    }
    state.known = true;
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
        Kind::Toggle { .. } => (TRACK.0.min(max.unwrap_or(f32::MAX)), TRACK.1),
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
        Kind::Toggle { on, pending, on_toggle } => {
            paint_toggle(*on, *pending, on_toggle.is_some(), rect, state, focused, frame, theme);
        }
        _ => {}
    }
}

/// A switch's track, as Material 3 has it.
const TRACK: (f32, f32) = (52.0, 32.0);

#[allow(clippy::too_many_arguments)]
fn paint_toggle(
    on: bool,
    pending: bool,
    enabled: bool,
    rect: Rect,
    state: &State,
    focused: bool,
    frame: &mut Frame,
    theme: &Theme,
) {
    let colors = &theme.colors;
    let track = Rect::new(
        rect.x + (rect.w - TRACK.0) / 2.0,
        rect.y + (rect.h - TRACK.1) / 2.0,
        TRACK.0,
        TRACK.1,
    );
    let radius = TRACK.1 / 2.0;
    // How far over the thumb is, 0 off to 1 on: it may overshoot.
    let t = state.value.value;
    let shown = t.clamp(0.0, 1.0);
    let fade = |color: Color| {
        if enabled {
            color
        } else {
            color.alpha(state::DISABLED_CONTENT)
        }
    };
    let off_track = colors.surface_container_highest;
    frame.fill(track, radius, fade(off_track.mix(colors.primary, shown)));
    if shown < 1.0 {
        frame.ring(track, radius, 2.0, fade(colors.outline).alpha(1.0 - shown));
    }
    // The thumb: small off, larger on, larger still while pressed.
    let press = state.press.value.clamp(0.0, 1.0);
    let size = 16.0 + 8.0 * shown;
    let size = size + (28.0 - size) * press;
    let x = track.x + radius + t * (TRACK.0 - 2.0 * radius);
    let y = track.y + radius;
    if enabled {
        let halo = 40.0;
        let layer = state.hover.value * state::HOVER + press * state::PRESS;
        let ink = if on { colors.primary } else { colors.on_surface };
        frame.fill(
            Rect::new(x - halo / 2.0, y - halo / 2.0, halo, halo),
            halo / 2.0,
            ink.alpha(layer),
        );
    }
    let thumb_color = colors.outline.mix(colors.on_primary, shown);
    // Waiting on the system, the thumb pulses.
    let pulse = if pending {
        0.6 + 0.4 * (state.phase * std::f32::consts::TAU * 1.2).cos()
    } else {
        1.0
    };
    frame.fill(
        Rect::new(x - size / 2.0, y - size / 2.0, size, size),
        size / 2.0,
        fade(thumb_color).alpha(pulse),
    );
    if focused {
        ring(frame, track, radius, state, theme);
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
