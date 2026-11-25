//! What opens from the bar: a panel that grows out of its part, on a layer
//! over the whole screen. Over the bar the layer passes the pointer on to
//! the bar beneath, so its parts still answer and another part's panel
//! opens in place of this one; a click anywhere else, or Escape, closes it.

use yokoso_ui::{
    Rect,
    app::OutputId,
    el::{Align, Edges, El, Ink, Size, Type},
    input::Input,
    theme::Theme,
};

use super::Msg;
use crate::{panel::Panel, sni::Entry};

/// What a panel shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Shows {
    /// The clock's calendar, at a year and a month.
    Calendar { shown: (i32, u32) },
    /// A tray item's menu: the rows under `parent`, and the rows that led
    /// there, for the way back.
    Menu {
        address: String,
        parent: i32,
        entries: Vec<Entry>,
        trail: Vec<i32>,
    },
}

impl Shows {
    /// The key of the part of the bar it grows from.
    pub fn part(&self) -> String {
        match self {
            Self::Calendar { .. } => "clock".into(),
            Self::Menu { address, .. } => format!("tray:{address}"),
        }
    }

    /// Whether `other` is the same panel, whatever it shows inside.
    pub fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Calendar { .. }, Self::Calendar { .. }) => true,
            (Self::Menu { address: a, .. }, Self::Menu { address: b, .. }) => a == b,
            _ => false,
        }
    }
}

pub struct Popup {
    pub output: OutputId,
    pub shows: Shows,
    pub panel: Panel<Msg>,
    /// The part it grows from, and where it is, on the layer.
    pub button: Rect,
    /// Its content is to be built anew before the next layout.
    pub stale: bool,
    /// Where the pointer was last on its layer, and whether over the bar.
    pub pointer: Option<(f32, f32)>,
    pub on_bar: bool,
}

/// The size a menu of `entries` takes, with a way back or not.
pub fn menu_size(entries: &[Entry], back: bool) -> (f32, f32) {
    let rows: f32 = entries
        .iter()
        .map(|entry| if entry.separator { SEPARATOR } else { ROW })
        .sum::<f32>()
        + if back { ROW } else { 0.0 };
    (MENU_WIDTH, rows + 2.0 * MENU_PAD)
}

const MENU_WIDTH: f32 = 260.0;
const MENU_PAD: f32 = 8.0;
const ROW: f32 = 34.0;
const SEPARATOR: f32 = 9.0;

/// A tray item's menu: its rows, a check or a dot for those that toggle, an
/// arrow for those that open more; the way back first, below the top.
pub fn menu_view(entries: &[Entry], back: bool, theme: &Theme) -> El<Msg> {
    let faint = Ink::Own(theme.colors.on_surface.alpha(0.38));
    let marks = entries.iter().any(|entry| entry.toggle.is_some());
    let row = |parts: Vec<El<Msg>>, on_press: Option<Msg>, key: i32| {
        El::press(10.0, parts, on_press)
            .key(("row", key))
            .padding(Edges {
                left: 10.0,
                right: 10.0,
                ..Edges::default()
            })
            .width(Size::Fill(1.0))
            .height(Size::Fixed(ROW))
    };
    let mut rows = Vec::new();
    if back {
        rows.push(row(
            vec![
                El::icon("chevron-left"),
                El::text("Back", Type::Body).width(Size::Fill(1.0)),
            ],
            Some(Msg::MenuBack),
            -1,
        ));
    }
    for entry in entries {
        if entry.separator {
            rows.push(
                El::row(
                    0.0,
                    vec![
                        El::block(Ink::Own(theme.colors.outline_variant), 0.5)
                            .width(Size::Fill(1.0))
                            .height(Size::Fixed(1.0)),
                    ],
                )
                .align(Align::Center)
                .padding(Edges {
                    left: 10.0,
                    right: 10.0,
                    ..Edges::default()
                })
                .width(Size::Fill(1.0))
                .height(Size::Fixed(SEPARATOR)),
            );
            continue;
        }
        let ink = if entry.enabled { Ink::OnSurface } else { faint };
        let mut parts = Vec::new();
        if marks {
            parts.push(match entry.toggle {
                Some((_, true)) => El::icon("check").ink(Ink::Primary),
                _ => El::spacer()
                    .width(Size::Fixed(theme.sizes.icon))
                    .height(Size::Fixed(theme.sizes.icon)),
            });
        }
        parts.push(
            El::text(&entry.label, Type::Body)
                .ink(ink)
                .ellipsis()
                .width(Size::Fill(1.0)),
        );
        if entry.submenu {
            parts.push(El::icon("chevron-right").ink(ink));
        }
        let used = if entry.submenu {
            Msg::MenuOpen(entry.id)
        } else {
            Msg::MenuPick(entry.id)
        };
        let on_press = entry.enabled.then_some(used);
        rows.push(row(parts, on_press, entry.id));
    }
    El::column(0.0, rows).stagger().padding(Edges::all(MENU_PAD)).fill()
}

/// `input` as the bar beneath sees it, `dx`, `dy` taken off its place.
pub fn shifted(input: &Input, dx: f32, dy: f32) -> Input {
    match input {
        Input::PointerEnter { x, y } => Input::PointerEnter { x: x - dx, y: y - dy },
        Input::PointerMove { x, y } => Input::PointerMove { x: x - dx, y: y - dy },
        Input::Press { button, x, y } => Input::Press {
            button: *button,
            x: x - dx,
            y: y - dy,
        },
        Input::Release { button, x, y } => Input::Release {
            button: *button,
            x: x - dx,
            y: y - dy,
        },
        other => other.clone(),
    }
}

/// Where `input` is, for those that have a place.
pub fn at(input: &Input) -> Option<(f32, f32)> {
    match input {
        Input::PointerEnter { x, y }
        | Input::PointerMove { x, y }
        | Input::Press { x, y, .. }
        | Input::Release { x, y, .. } => Some((*x, *y)),
        _ => None,
    }
}

/// What a panel at `to` grows out of, and goes back into: its part at
/// `button`, or a line along the bar's edge as wide as itself, at the top
/// of it for a bar at the top.
pub fn origin(panels: crate::config::Panels, button: Rect, to: Rect, top: bool) -> Rect {
    match panels {
        crate::config::Panels::Grow => button,
        crate::config::Panels::Drop if top => Rect::new(to.x, to.y, to.w, 1.0),
        crate::config::Panels::Drop => Rect::new(to.x, to.bottom() - 1.0, to.w, 1.0),
    }
}

/// Where a panel `size` big goes for a part at `button`: under the bar at
/// the top (over it at the bottom), its middle under the part's, kept
/// `margin` in from the screen's sides.
pub fn place(button: Rect, size: (f32, f32), screen: (f32, f32), bar: Rect, top: bool, margin: f32) -> Rect {
    let (w, h) = size;
    let x = (button.x + button.w / 2.0 - w / 2.0).clamp(margin, (screen.0 - margin - w).max(margin));
    let y = if top { bar.bottom() + 6.0 } else { bar.y - 6.0 - h };
    Rect::new(x, y, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: i32, separator: bool) -> Entry {
        Entry {
            id,
            label: format!("row {id}"),
            enabled: true,
            separator,
            toggle: None,
            submenu: false,
        }
    }

    #[test]
    fn a_dropping_panel_comes_from_the_bar_edge_as_wide_as_itself() {
        use crate::config::Panels;
        let button = Rect::new(680.0, 12.0, 40.0, 32.0);
        let to = Rect::new(540.0, 54.0, 320.0, 300.0);
        assert_eq!(origin(Panels::Grow, button, to, true), button);
        assert_eq!(
            origin(Panels::Drop, button, to, true),
            Rect::new(540.0, 54.0, 320.0, 1.0)
        );
        assert_eq!(
            origin(Panels::Drop, button, to, false),
            Rect::new(540.0, 353.0, 320.0, 1.0)
        );
    }

    #[test]
    fn a_menu_is_as_tall_as_its_rows() {
        let entries = [entry(1, false), entry(2, true), entry(3, false)];
        assert_eq!(
            menu_size(&entries, false),
            (MENU_WIDTH, 2.0 * ROW + SEPARATOR + 2.0 * MENU_PAD)
        );
        assert_eq!(menu_size(&entries, true).1, 3.0 * ROW + SEPARATOR + 2.0 * MENU_PAD);
    }

    #[test]
    fn a_panel_goes_under_its_part_and_stays_on_the_screen() {
        let bar = Rect::new(8.0, 8.0, 1384.0, 40.0);
        let middle = place(
            Rect::new(680.0, 12.0, 40.0, 32.0),
            (320.0, 300.0),
            (1400.0, 900.0),
            bar,
            true,
            8.0,
        );
        assert_eq!(middle, Rect::new(540.0, 54.0, 320.0, 300.0));
        let edge = place(
            Rect::new(1350.0, 12.0, 30.0, 32.0),
            (260.0, 200.0),
            (1400.0, 900.0),
            bar,
            true,
            8.0,
        );
        assert_eq!(edge.x, 1400.0 - 8.0 - 260.0);
        let bottom = Rect::new(8.0, 852.0, 1384.0, 40.0);
        let above = place(
            Rect::new(680.0, 856.0, 40.0, 32.0),
            (320.0, 300.0),
            (1400.0, 900.0),
            bottom,
            false,
            8.0,
        );
        assert_eq!(above.y, 852.0 - 6.0 - 300.0);
    }
}
