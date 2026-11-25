//! Menus in the tree: one opens at a time, over everything; while it is
//! open it takes the pointer and the keys, and Escape closes it before it
//! goes anywhere else.

use super::{Id, Tree};
use crate::{
    el::{Kind, MenuEl, menu},
    geometry::Rect,
    input::{Input, KeyState, Keysym},
    theme::Theme,
};

/// The evdev code of the left button.
const BTN_LEFT: u32 = 0x110;

impl<M: Clone> Tree<M> {
    /// The menu `id`, and its button's rect.
    pub(super) fn menu(&self, id: Id) -> Option<(&MenuEl<M>, Rect)> {
        let (el, rect) = self.element(id)?;
        match &el.kind {
            Kind::Menu(menu) if menu.on_pick.is_some() => Some((menu, rect)),
            _ => None,
        }
    }

    /// Where the tree is drawn: what a menu's list stays inside.
    pub(super) fn bounds(&self) -> Rect {
        self.view.map_or(Rect::new(0.0, 0.0, 1e6, 1e6), |view| {
            Rect::new(0.0, 0.0, view.width, view.height)
        })
    }

    /// Opens the menu `id`, its chosen item highlighted.
    pub(super) fn open_menu(&mut self, id: Id) {
        let Some(selected) = self.menu(id).map(|(menu, _)| menu.selected) else {
            return;
        };
        self.close_menu();
        if let Some(state) = self.states.get_mut(&id) {
            state.value.to(1.0);
            state.hover_row = Some(selected.unwrap_or(0));
        }
        self.menu_open = Some(id);
    }

    pub(super) fn close_menu(&mut self) {
        if let Some(id) = self.menu_open.take()
            && let Some(state) = self.states.get_mut(&id)
        {
            state.value.to(0.0);
        }
    }

    /// Input while a menu is open: it takes all of it. The messages, and
    /// whether anything shows a change; None when no menu is open.
    pub(super) fn menu_input(&mut self, input: &Input, theme: &Theme) -> Option<(Vec<M>, bool)> {
        let id = self.menu_open?;
        let Some((menu, rect)) = self.menu(id) else {
            self.menu_open = None;
            return None;
        };
        let popup = menu::popup(menu, rect, self.bounds(), theme);
        let count = menu.items.len();
        let pick = |at: usize| menu.on_pick.as_ref().map(|pick| pick(at));
        let mut messages = Vec::new();
        let highlighted = self.states.get(&id).and_then(|state| state.hover_row);
        let mut highlight = highlighted;
        let mut close = false;
        match input {
            Input::PointerMove { x, y } | Input::PointerEnter { x, y } => {
                if let Some(at) = menu::item_at(menu, popup, *x, *y, theme) {
                    highlight = Some(at);
                }
            }
            // A press on an item picks it; anywhere else, it only closes.
            Input::Press { button: BTN_LEFT, x, y } => {
                if let Some(at) = menu::item_at(menu, popup, *x, *y, theme) {
                    messages.extend(pick(at));
                }
                close = true;
                self.swallow_release = true;
            }
            Input::Key(key) if key.state != KeyState::Released => match key.keysym {
                Keysym::Down => highlight = Some(highlighted.map_or(0, |at| (at + 1).min(count.saturating_sub(1)))),
                Keysym::Up => highlight = Some(highlighted.map_or(0, |at| at.saturating_sub(1))),
                Keysym::Home => highlight = Some(0),
                Keysym::End => highlight = Some(count.saturating_sub(1)),
                Keysym::Return | Keysym::KP_Enter | Keysym::space => {
                    messages.extend(highlighted.and_then(pick));
                    close = true;
                }
                Keysym::Escape | Keysym::Tab | Keysym::ISO_Left_Tab => close = true,
                _ => {}
            },
            Input::KeyboardLeave => close = true,
            _ => {}
        }
        if close {
            self.close_menu();
        } else if let Some(state) = self.states.get_mut(&id) {
            state.hover_row = highlight;
        }
        Some((messages, true))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        el::{ButtonStyle, El, Size, tree::Tree},
        geometry::Rect,
        input::{Input, Key, KeyState, Keysym, Mods},
    };

    #[derive(Clone, Debug, PartialEq)]
    enum Msg {
        Pick(usize),
        Button,
    }

    fn tree() -> Tree<Msg> {
        let mut tree = Tree::default();
        tree.set(El::column(
            0.0,
            vec![
                El::menu(
                    vec!["one".into(), "two".into(), "three".into()],
                    Some(0),
                    "",
                    Some(Msg::Pick),
                )
                .width(Size::Fixed(200.0))
                .height(Size::Fixed(40.0)),
                El::button("under", ButtonStyle::Tonal, Some(Msg::Button))
                    .width(Size::Fixed(200.0))
                    .height(Size::Fixed(40.0)),
            ],
        ));
        tree.layout_with(Rect::new(0.0, 0.0, 400.0, 400.0), &mut |_: &El<Msg>, _| (0.0, 0.0));
        tree
    }

    fn key(keysym: Keysym) -> Input {
        Input::Key(Key {
            keysym,
            code: 0,
            text: None,
            state: KeyState::Pressed,
            mods: Mods::default(),
        })
    }

    fn click(tree: &mut Tree<Msg>, x: f32, y: f32) -> Vec<Msg> {
        tree.input(&Input::PointerMove { x, y });
        let mut all = tree.input(&Input::Press { button: 0x110, x, y }).messages;
        all.extend(tree.input(&Input::Release { button: 0x110, x, y }).messages);
        all
    }

    #[test]
    fn a_click_opens_it_and_the_keys_pick() {
        let mut tree = tree();
        click(&mut tree, 20.0, 20.0);
        assert!(tree.menu_open.is_some());
        tree.input(&key(Keysym::Down));
        tree.input(&key(Keysym::Down));
        assert_eq!(tree.input(&key(Keysym::Return)).messages, vec![Msg::Pick(2)]);
        assert!(tree.menu_open.is_none());
    }

    #[test]
    fn escape_closes_it_before_going_anywhere_else() {
        let mut tree = tree();
        click(&mut tree, 20.0, 20.0);
        let first = tree.input(&key(Keysym::Escape));
        assert!(!first.escaped && tree.menu_open.is_none());
        assert!(tree.input(&key(Keysym::Escape)).escaped);
    }

    #[test]
    fn a_click_away_only_closes_it() {
        let mut tree = tree();
        click(&mut tree, 20.0, 20.0);
        // The button under it is where the list is not: x past the list.
        assert_eq!(click(&mut tree, 350.0, 60.0), Vec::<Msg>::new());
        assert!(tree.menu_open.is_none());
        // Now the button takes clicks again.
        assert_eq!(click(&mut tree, 20.0, 60.0), vec![Msg::Button]);
    }

    #[test]
    fn a_click_on_an_item_picks_it() {
        let mut tree = tree();
        click(&mut tree, 20.0, 20.0);
        // The list opens under the button: its second item, about 4 + 40 + 20 down.
        assert_eq!(
            click(&mut tree, 20.0, 40.0 + 4.0 + 4.0 + 40.0 + 20.0),
            vec![Msg::Pick(1)]
        );
    }
}
