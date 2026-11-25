//! What each kind of element is: its role, its size, its text, and how it
//! paints in its state. The tree calls these; they never change state.

use super::{ButtonStyle, CardStyle, El, Ink, Kind, SliderEl, TextEl, TileEl, Type, tree::Role, tree::State};
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
        Kind::Slider(_) => Role::Slider,
        Kind::Card { .. } => Role::Card,
        Kind::Tile(_) => Role::Tile,
        _ => Role::Group,
    }
}

/// Whether it takes the pointer and the keyboard.
pub fn interactive<M>(kind: &Kind<M>) -> bool {
    matches!(
        kind,
        Kind::Button { on_press: Some(_), .. } | Kind::Toggle { on_toggle: Some(_), .. }
    ) || matches!(kind, Kind::Slider(slider) if slider.on_change.is_some())
        || matches!(kind, Kind::Tile(tile) if tile.on_press.is_some())
}

/// Whether it waits on something, pulsing meanwhile.
pub fn pending<M>(kind: &Kind<M>) -> bool {
    matches!(kind, Kind::Toggle { pending: true, .. }) || matches!(kind, Kind::Tile(tile) if tile.pending)
}

/// Aims its springs at what the description says: a toggle's thumb at its
/// side. The first time there, at once.
pub fn settle<M>(kind: &Kind<M>, state: &mut State) {
    let target = match kind {
        Kind::Toggle { on, .. } => Some(if *on { 1.0 } else { 0.0 }),
        Kind::Slider(slider) => Some(slider.fraction(slider.value)),
        Kind::Tile(tile) => Some(if tile.on { 1.0 } else { 0.0 }),
        _ => None,
    };
    if let Some(target) = target {
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
        Kind::Icon { .. } => (theme.sizes.icon, theme.sizes.icon),
        Kind::Tile(tile) => {
            let title = text.layout(&tile.title, &tile_title(theme), scale, None, false);
            let width =
                TILE_PAD + theme.sizes.icon + TILE_PAD * 0.75 + title.width().ceil() + tile_more(tile) + TILE_PAD;
            (width.max(160.0).min(max.unwrap_or(f32::MAX)), TILE_HEIGHT)
        }
        Kind::Slider(slider) => {
            let label = if slider.show_value { VALUE_ROOM } else { 0.0 };
            ((240.0 + label).min(max.unwrap_or(f32::MAX)), HANDLE.1)
        }
        Kind::Button { label, .. } => {
            let laid = text.layout(label, &label_style(theme), scale, None, false);
            let width = laid.width().ceil() + 2.0 * button_pad(theme);
            (width.min(max.unwrap_or(f32::MAX)), theme.sizes.control)
        }
        _ => (0.0, 0.0),
    }
}

/// Its text, laid out for the rect it got: a line or a few.
pub fn paragraphs<M>(el: &El<M>, rect: Rect, text: &mut Text, theme: &Theme, scale: f32) -> Vec<Paragraph> {
    if let Kind::Tile(tile) = &el.kind {
        let room = tile_text_room(tile, rect, theme) + 0.5;
        let small = Style {
            size: theme.sizes.small,
            weight: 400.0,
            family: None,
            tabular: false,
        };
        return vec![
            text.layout(&tile.title, &tile_title(theme), scale, Some(room), true),
            text.layout(&tile.subtitle, &small, scale, Some(room), true),
        ];
    }
    let one = match &el.kind {
        Kind::Text(t) => Some(text.layout(&t.text, &style(t, theme), scale, Some(rect.w + 0.5), t.ellipsis)),
        Kind::Slider(slider) if slider.show_value => Some(text.layout(
            &value_text(slider),
            &Style {
                size: theme.sizes.label,
                weight: 500.0,
                family: None,
                tabular: true,
            },
            scale,
            None,
            false,
        )),
        Kind::Button { label, .. } => {
            let room = (rect.w - 2.0 * button_pad(theme)).max(0.0) + 0.5;
            Some(text.layout(label, &label_style(theme), scale, Some(room), true))
        }
        _ => None,
    };
    one.into_iter().collect()
}

/// Paints `el` in `rect`, as `state` and `focus_visible` have it.
pub fn paint<M>(
    el: &El<M>,
    rect: Rect,
    state: &State,
    paragraphs: &[Paragraph],
    focused: bool,
    frame: &mut Frame,
    theme: &Theme,
) {
    let colors = &theme.colors;
    let paragraph = paragraphs.first();
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
        Kind::Slider(slider) => paint_slider(slider, rect, state, paragraph, focused, frame, theme),
        Kind::Card { style, .. } => {
            let radius = theme.sizes.radius;
            match style {
                CardStyle::Filled => frame.fill(rect, radius, colors.surface_container_highest),
                CardStyle::Elevated => {
                    frame.shadow(rect.grow(-1.0), radius, 10.0, colors.shadow.alpha(0.35));
                    frame.fill(rect, radius, colors.surface_container_low);
                }
                CardStyle::Outlined => {
                    frame.fill(rect, radius, colors.surface);
                    frame.ring(rect, radius, 1.0, colors.outline_variant);
                }
            }
        }
        Kind::Icon { name, ink: color } => frame.icon(name, rect, ink(*color, theme)),
        Kind::Tile(tile) => paint_tile(tile, rect, state, paragraphs, focused, frame, theme),
        _ => {}
    }
}

const TILE_HEIGHT: f32 = 64.0;
const TILE_PAD: f32 = 16.0;
/// The arrow into a tile's details takes this much at its end.
const TILE_MORE: f32 = 44.0;

fn tile_title(theme: &Theme) -> Style {
    Style {
        size: theme.sizes.body,
        weight: 600.0,
        family: None,
        tabular: false,
    }
}

fn tile_more<M>(tile: &TileEl<M>) -> f32 {
    if tile.on_more.is_some() { TILE_MORE } else { 0.0 }
}

fn tile_text_room<M>(tile: &TileEl<M>, rect: Rect, theme: &Theme) -> f32 {
    (rect.w - TILE_PAD - theme.sizes.icon - TILE_PAD * 0.75 - tile_more(tile).max(TILE_PAD)).max(0.0)
}

/// Whether `x` is on a tile's arrow into its details.
pub fn tile_more_at<M>(tile: &TileEl<M>, rect: Rect, x: f32) -> bool {
    tile.on_more.is_some() && x >= rect.right() - TILE_MORE
}

fn paint_tile<M>(
    tile: &TileEl<M>,
    rect: Rect,
    state: &State,
    paragraphs: &[Paragraph],
    focused: bool,
    frame: &mut Frame,
    theme: &Theme,
) {
    let colors = &theme.colors;
    let enabled = tile.on_press.is_some();
    let t = state.value.value.clamp(0.0, 1.0);
    let container = colors.surface_container_highest.mix(colors.primary, t);
    let content = colors.on_surface.mix(colors.on_primary, t);
    let (container, content) = if enabled {
        (container, content)
    } else {
        (
            colors.on_surface.alpha(state::DISABLED_CONTAINER),
            colors.on_surface.alpha(state::DISABLED_CONTENT),
        )
    };
    // Pressed, a tile rounds a little more.
    let press = state.press.value.clamp(0.0, 1.2);
    let radius = theme.sizes.radius * 1.5 + 8.0 * press;
    frame.fill(rect, radius, container);
    if enabled {
        let layer = state.hover.value * state::HOVER + press * state::PRESS;
        frame.fill(rect, radius, content.alpha(layer.clamp(0.0, 1.0)));
    }
    // Waiting on the system, its icon pulses.
    let pulse = if tile.pending {
        0.45 + 0.55 * (state.phase * std::f32::consts::TAU * 1.2).cos().abs()
    } else {
        1.0
    };
    let size = theme.sizes.icon;
    let icon = Rect::new(rect.x + TILE_PAD, rect.y + (rect.h - size) / 2.0, size, size);
    frame.icon(tile.icon, icon, content.alpha(pulse));
    let x = icon.right() + TILE_PAD * 0.75;
    if let [title, subtitle] = paragraphs {
        let total = title.height() + subtitle.height();
        let y = rect.y + (rect.h - total) / 2.0;
        frame.text(title, x, y, content);
        frame.text(subtitle, x, y + title.height(), content.alpha(0.8));
    }
    if tile.on_more.is_some() {
        let more = Rect::new(rect.right() - TILE_MORE, rect.y, TILE_MORE, rect.h);
        frame.fill(
            Rect::new(more.x, more.y + 14.0, 1.0, more.h - 28.0),
            0.0,
            content.alpha(0.25),
        );
        let arrow = Rect::new(
            more.x + (more.w - size) / 2.0,
            more.y + (more.h - size) / 2.0,
            size,
            size,
        );
        frame.icon("chevron-right", arrow, content);
    }
    if focused {
        ring(frame, rect, radius, state, theme);
    }
}

/// A slider's handle: a tall pill, as Material 3 Expressive has it.
const HANDLE: (f32, f32) = (4.0, 44.0);
/// Room for the value written beside a slider.
const VALUE_ROOM: f32 = 56.0;
/// The space between the handle and the track on either side of it.
const HANDLE_GAP: f32 = 6.0;

fn value_text<M>(slider: &SliderEl<M>) -> String {
    if slider.step >= 1.0 {
        format!("{:.0}", slider.value)
    } else {
        format!("{:.2}", slider.value)
    }
}

/// Where a slider's handle runs, in `rect`: all of it, but the room for its
/// value.
pub fn slider_track<M>(slider: &SliderEl<M>, rect: Rect) -> Rect {
    let room = if slider.show_value { VALUE_ROOM } else { 0.0 };
    Rect::new(rect.x, rect.y, (rect.w - room).max(0.0), rect.h)
}

/// The value at `x` along a slider's `track`, on a step.
pub fn slider_value_at<M>(slider: &SliderEl<M>, track: Rect, x: f32) -> f32 {
    let inner = (track.w - HANDLE.0).max(1.0);
    let t = ((x - track.x - HANDLE.0 / 2.0) / inner).clamp(0.0, 1.0);
    slider.fit(slider.min + t * (slider.max - slider.min))
}

fn paint_slider<M>(
    slider: &SliderEl<M>,
    rect: Rect,
    state: &State,
    paragraph: Option<&Paragraph>,
    focused: bool,
    frame: &mut Frame,
    theme: &Theme,
) {
    let colors = &theme.colors;
    let enabled = slider.on_change.is_some();
    let track = slider_track(slider, rect);
    let t = state.value.value.clamp(0.0, 1.0);
    let middle = track.y + track.h / 2.0;
    let x = track.x + HANDLE.0 / 2.0 + t * (track.w - HANDLE.0).max(0.0);
    let (active, inactive, handle) = if enabled {
        (colors.primary, colors.secondary_container, colors.primary)
    } else {
        let dim = colors.on_surface.alpha(state::DISABLED_CONTAINER);
        (
            colors.on_surface.alpha(state::DISABLED_CONTENT),
            dim,
            colors.on_surface.alpha(state::DISABLED_CONTENT),
        )
    };
    let bar = 16.0;
    let left = Rect::new(track.x, middle - bar / 2.0, (x - HANDLE_GAP - track.x).max(0.0), bar);
    let right_x = x + HANDLE_GAP;
    let right = Rect::new(right_x, middle - bar / 2.0, (track.right() - right_x).max(0.0), bar);
    frame.fill(left, (left.w / 2.0).min(bar / 2.0), active);
    frame.fill(right, (right.w / 2.0).min(bar / 2.0), inactive);
    // Pressed, the handle narrows; hovered, it shows a state layer.
    let press = state.press.value.clamp(0.0, 1.0);
    let width = HANDLE.0 - 2.0 * press;
    let handle_rect = Rect::new(x - width / 2.0, middle - HANDLE.1 / 2.0, width, HANDLE.1);
    frame.fill(handle_rect, width / 2.0, handle);
    if enabled && state.hover.value > 0.0 {
        let halo = Rect::new(x - 10.0, middle - HANDLE.1 / 2.0 - 2.0, 20.0, HANDLE.1 + 4.0);
        frame.fill(halo, 10.0, handle.alpha(state.hover.value * state::HOVER));
    }
    if focused {
        ring(frame, handle_rect, width / 2.0, state, theme);
    }
    if let Some(paragraph) = paragraph {
        let x = rect.right() - paragraph.width();
        let y = middle - paragraph.height() / 2.0;
        frame.text(paragraph, x, y, colors.on_surface_variant);
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
