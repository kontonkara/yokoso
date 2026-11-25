//! The menu's own part: its button, the list that opens over everything,
//! and where each of its items is.

use super::{MenuEl, tree::State};
use crate::{
    app::Frame,
    geometry::Rect,
    text::{Paragraph, Style, Text},
    theme::{Theme, state},
};

const PAD: f32 = 12.0;
/// Room in the list above its first item and below its last.
const EDGE: f32 = 4.0;

pub fn item_height(theme: &Theme) -> f32 {
    theme.sizes.control
}

fn style(theme: &Theme) -> Style {
    Style {
        size: theme.sizes.body,
        weight: 400.0,
        family: None,
        tabular: false,
    }
}

/// What its button says: the chosen item, or the placeholder.
pub fn label<M>(menu: &MenuEl<M>) -> &str {
    menu.selected
        .and_then(|at| menu.items.get(at))
        .map_or(menu.placeholder.as_str(), String::as_str)
}

/// Its button's label, then each item.
pub fn paragraphs<M>(menu: &MenuEl<M>, text: &mut Text, theme: &Theme, scale: f32) -> Vec<Paragraph> {
    std::iter::once(label(menu))
        .chain(menu.items.iter().map(String::as_str))
        .map(|words| text.layout(words, &style(theme), scale, None, false))
        .collect()
}

/// Its size: as wide as its longest item, with room for the arrow.
pub fn measure<M>(menu: &MenuEl<M>, text: &mut Text, theme: &Theme, scale: f32, max: Option<f32>) -> (f32, f32) {
    let widest = paragraphs(menu, text, theme, scale)
        .iter()
        .map(Paragraph::width)
        .fold(0.0f32, f32::max);
    let width = widest.ceil() + 2.0 * PAD + theme.sizes.icon + 8.0;
    (width.min(max.unwrap_or(f32::MAX)), theme.sizes.control)
}

/// Where its list opens: under its button, or over it where there is no
/// room below, inside `bounds`.
pub fn popup<M>(menu: &MenuEl<M>, rect: Rect, bounds: Rect, theme: &Theme) -> Rect {
    let height = menu.items.len() as f32 * item_height(theme) + 2.0 * EDGE;
    let width = rect.w.max(200.0);
    let x = rect.x.min((bounds.right() - width - 4.0).max(bounds.x));
    let below = rect.bottom() + 4.0 + height <= bounds.bottom();
    let y = if below {
        rect.bottom() + 4.0
    } else {
        (rect.y - 4.0 - height).max(bounds.y)
    };
    Rect::new(x, y, width, height)
}

/// The item at `y` in an open `popup`.
pub fn item_at<M>(menu: &MenuEl<M>, popup: Rect, x: f32, y: f32, theme: &Theme) -> Option<usize> {
    if !popup.contains(x, y) {
        return None;
    }
    let at = ((y - popup.y - EDGE) / item_height(theme)).floor();
    (at >= 0.0 && (at as usize) < menu.items.len()).then_some(at as usize)
}

/// Its button.
pub fn paint<M>(
    menu: &MenuEl<M>,
    rect: Rect,
    state: &State,
    paragraphs: &[Paragraph],
    focused: bool,
    frame: &mut Frame,
    theme: &Theme,
) {
    let colors = &theme.colors;
    let enabled = menu.on_pick.is_some();
    let radius = theme.sizes.radius_inner * 2.0;
    frame.fill(rect, radius, colors.surface_container_highest);
    let open = state.value.value.clamp(0.0, 1.0);
    let line = if !enabled {
        colors.on_surface.alpha(state::DISABLED_CONTAINER)
    } else {
        colors.outline.mix(colors.primary, open)
    };
    frame.ring(rect, radius, 1.0 + open, line);
    if enabled {
        let layer = state.hover.value * state::HOVER + state.press.value.clamp(0.0, 1.0) * state::PRESS;
        frame.fill(rect, radius, colors.on_surface.alpha(layer));
    }
    let content = if enabled {
        colors.on_surface
    } else {
        colors.on_surface.alpha(state::DISABLED_CONTENT)
    };
    if let Some(label) = paragraphs.first() {
        let ink = if menu.selected.is_some() {
            content
        } else {
            colors.on_surface_variant
        };
        frame.text(label, rect.x + PAD, rect.y + (rect.h - label.height()) / 2.0, ink);
    }
    let size = theme.sizes.icon;
    let arrow = Rect::new(rect.right() - PAD - size, rect.y + (rect.h - size) / 2.0, size, size);
    frame.icon(if open > 0.5 { "chevron-up" } else { "chevron-down" }, arrow, content);
    if focused {
        super::kinds::ring(frame, rect, radius, state, theme);
    }
}

/// Its list, over everything, as far open as its spring has come; the
/// highlighted item and the chosen one marked.
pub fn paint_popup<M>(
    menu: &MenuEl<M>,
    rect: Rect,
    bounds: Rect,
    state: &State,
    paragraphs: &[Paragraph],
    frame: &mut Frame,
    theme: &Theme,
) {
    let open = state.value.value.clamp(0.0, 1.0);
    if open <= 0.0 {
        return;
    }
    let colors = &theme.colors;
    let whole = popup(menu, rect, bounds, theme);
    // It unrolls from its top.
    let shown = Rect::new(whole.x, whole.y, whole.w, whole.h * (0.6 + 0.4 * open));
    let radius = theme.sizes.radius;
    frame.shadow(shown, radius, 12.0, colors.shadow.alpha(0.4 * open));
    frame.fill(shown, radius, colors.surface_container_high.alpha(open));
    frame.clip(Some(shown));
    let height = item_height(theme);
    for (at, paragraph) in paragraphs.iter().skip(1).enumerate() {
        let item = Rect::new(
            whole.x + EDGE,
            whole.y + EDGE + at as f32 * height,
            whole.w - 2.0 * EDGE,
            height,
        );
        if menu.selected == Some(at) {
            frame.fill(
                item,
                theme.sizes.radius_inner * 2.0,
                colors.secondary_container.alpha(open),
            );
        }
        if state.hover_row == Some(at) {
            frame.fill(
                item,
                theme.sizes.radius_inner * 2.0,
                colors.on_surface.alpha(state::HOVER * 1.5 * open),
            );
        }
        let y = item.y + (item.h - paragraph.height()) / 2.0;
        frame.text(paragraph, item.x + PAD, y, colors.on_surface.alpha(open));
        if menu.selected == Some(at) {
            let size = theme.sizes.icon;
            let check = Rect::new(item.right() - PAD - size, item.y + (item.h - size) / 2.0, size, size);
            frame.icon("check", check, colors.on_secondary_container.alpha(open));
        }
    }
    frame.clip(None);
}
