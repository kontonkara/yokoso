//! Fields in the tree: keys, clicks and drags edit them, the clipboard
//! copies and pastes, the caret blinks a while and then holds still, and the
//! text scrolls to keep it in view.

use std::time::Duration;

use super::{Id, Tree};
use crate::{
    el::{
        FieldEl, Kind,
        edit::{Edit, Step},
        field,
    },
    geometry::Rect,
    input::{Key, Keysym},
};

/// The caret blinks this long after the last thing done in a field, then
/// holds still: no frames for a caret no one is using.
const BLINK_FOR: f64 = 10.0;
/// Half a blink: on this long, off this long.
const BLINK: f64 = 0.5;
/// Two clicks closer than this, on one spot, are a double click.
const DOUBLE_CLICK: f64 = 0.4;

/// The evdev codes of the letters the shortcuts go by, wherever a layout
/// puts its letters.
const KEY_A: u32 = 30;
const KEY_C: u32 = 46;
const KEY_V: u32 = 47;
const KEY_X: u32 = 45;

/// Whether `key` is the shortcut on `letter`: by its Latin letter where the
/// layout gives one (so Dvorak's C is C), else by where the key is (so
/// Ctrl+C works in a Cyrillic layout too).
fn shortcut(key: &Key, code: u32, letter: char) -> bool {
    match key.keysym.key_char().filter(char::is_ascii_alphabetic) {
        Some(typed) => typed.to_ascii_lowercase() == letter,
        None => key.code == code,
    }
}

/// What a key did in a field.
pub(super) struct Outcome<M> {
    pub message: Option<M>,
    pub copy: Option<String>,
    pub paste: bool,
    /// The key is not the field's: focus or the shell takes it.
    pub passed: bool,
}

impl<M> Default for Outcome<M> {
    fn default() -> Self {
        Self {
            message: None,
            copy: None,
            paste: false,
            passed: false,
        }
    }
}

impl<M: Clone> Tree<M> {
    /// The field `id`, and its rect.
    pub(super) fn field(&self, id: Id) -> Option<(&FieldEl<M>, Rect)> {
        let (el, rect) = self.element(id)?;
        match &el.kind {
            Kind::Field(field) if field.on_change.is_some() => Some((field, rect)),
            _ => None,
        }
    }

    /// Something was done in the field `id`: the caret shows, and blinks
    /// from now.
    pub(super) fn touch_field(&mut self, id: Id) {
        self.touch(id);
    }

    fn touch(&mut self, id: Id) {
        if let Some(state) = self.states.get_mut(&id) {
            state.activity = crate::app::monotonic();
            state.caret_on = true;
        }
        // The text may scroll: laid out again.
        self.stale = true;
    }

    /// `edit` applied to a copy of the field's text: the new text sent, if
    /// it changed, and the cursor kept.
    fn change(&mut self, id: Id, apply: impl FnOnce(&mut Edit, &mut String)) -> Option<M> {
        let (field, _) = self.field(id)?;
        let mut text = field.text.clone();
        let before = text.clone();
        let change = field.on_change.clone()?;
        let state = self.states.get_mut(&id)?;
        apply(&mut state.edit, &mut text);
        self.touch(id);
        (text != before).then(|| change(text))
    }

    /// A key in the field `id`.
    pub(super) fn field_key(&mut self, id: Id, key: &Key) -> Outcome<M> {
        let mut out = Outcome::default();
        let Some((field, _)) = self.field(id) else {
            out.passed = true;
            return out;
        };
        let (text, password, submit) = (field.text.clone(), field.password, field.on_submit.clone());
        let (ctrl, shift) = (key.mods.ctrl, key.mods.shift);
        if let Some(state) = self.states.get_mut(&id) {
            state.caps = key.mods.caps_lock;
        }
        let step = |state: &mut Edit, step: Step| state.go(&text, step, shift);
        let moved = |tree: &mut Tree<M>, to: Step| {
            if let Some(state) = tree.states.get_mut(&id) {
                step(&mut state.edit, to);
            }
            tree.touch(id);
        };
        match key.keysym {
            Keysym::Left => moved(self, if ctrl { Step::WordLeft } else { Step::Left }),
            Keysym::Right => moved(self, if ctrl { Step::WordRight } else { Step::Right }),
            Keysym::Home => moved(self, Step::Home),
            Keysym::End => moved(self, Step::End),
            Keysym::BackSpace => out.message = self.change(id, |edit, text| edit.delete(text, true, ctrl)),
            Keysym::Delete => out.message = self.change(id, |edit, text| edit.delete(text, false, ctrl)),
            Keysym::Return | Keysym::KP_Enter => out.message = submit,
            Keysym::Escape => {
                // A selection goes first; with none, Escape is the shell's.
                match self.states.get_mut(&id) {
                    Some(state) if state.edit.has_selection() => state.edit = Edit::at(state.edit.cursor),
                    _ => out.passed = true,
                }
            }
            _ if ctrl && shortcut(key, KEY_A, 'a') => {
                if let Some(state) = self.states.get_mut(&id) {
                    state.edit.select_all(&text);
                }
                self.touch(id);
            }
            _ if ctrl && shortcut(key, KEY_C, 'c') => {
                out.copy = self.selected(id, &text, password);
            }
            _ if ctrl && shortcut(key, KEY_X, 'x') => {
                out.copy = self.selected(id, &text, password);
                if out.copy.is_some() {
                    out.message = self.change(id, |edit, text| edit.delete(text, true, false));
                }
            }
            _ if ctrl && shortcut(key, KEY_V, 'v') => out.paste = true,
            _ => match &key.text {
                Some(typed) if !ctrl && !key.mods.alt => {
                    let typed = typed.clone();
                    out.message = self.change(id, |edit, text| edit.insert(text, &typed));
                }
                _ => out.passed = true,
            },
        }
        out
    }

    /// What is selected in the field `id`, to copy: never a password's.
    fn selected(&self, id: Id, text: &str, password: bool) -> Option<String> {
        let edit = self.states.get(&id)?.edit;
        (!password && edit.has_selection()).then(|| edit.selected(text).to_owned())
    }

    /// Clipboard text put into the field `id`, on one line.
    pub(super) fn field_paste(&mut self, id: Id, pasted: &str) -> Option<M> {
        let line: String = pasted.lines().collect::<Vec<_>>().join(" ");
        self.change(id, |edit, text| edit.insert(text, &line))
    }

    /// The byte index in the field `id` under `x`.
    fn index_at(&self, id: Id, x: f32) -> Option<usize> {
        let (field, rect) = self.field(id)?;
        let state = self.states.get(&id)?;
        let shown = self.paragraphs.get(&id)?.first()?;
        let at = shown.index_at(x - field::text_x(rect, state.scroll_x), 1.0);
        Some(field::from_shown(field, at))
    }

    /// A press in the field `id`: the caret to where it was, the selection
    /// stretched to it with Shift, the word under it on a double click.
    pub(super) fn field_press(&mut self, id: Id, x: f32, shift: bool) {
        let Some(at) = self.index_at(id, x) else { return };
        let text = self.field(id).map(|(field, _)| field.text.clone()).unwrap_or_default();
        let now = crate::app::monotonic();
        if let Some(state) = self.states.get_mut(&id) {
            let double = now - state.clicked.0 < DOUBLE_CLICK && (x - state.clicked.1).abs() < 4.0;
            state.clicked = (now, x);
            if shift {
                state.edit.cursor = at;
            } else {
                state.edit = Edit::at(at);
            }
            if double {
                state.edit.select_word(&text);
            }
        }
        self.touch(id);
    }

    /// The pointer dragged across the field `id`: the selection follows.
    pub(super) fn field_drag(&mut self, id: Id, x: f32) {
        let Some(at) = self.index_at(id, x) else { return };
        if let Some(state) = self.states.get_mut(&id)
            && state.edit.cursor != at
        {
            state.edit.cursor = at;
            self.touch(id);
        }
    }

    /// After layout: each field's cursor kept in its text, and the text
    /// scrolled to keep the caret in view.
    pub(super) fn settle_fields(&mut self) {
        let fields: Vec<(Id, Rect, String, usize)> = self
            .nodes
            .iter()
            .filter_map(|node| {
                let (field, rect) = self.field(node.id)?;
                let caret = self.states.get(&node.id).map_or(0, |state| state.edit.cursor);
                Some((node.id, rect, field.text.clone(), caret))
            })
            .collect();
        for (id, rect, text, _) in fields {
            let Some(caret) = self.field_caret_x(id) else { continue };
            let Some(state) = self.states.get_mut(&id) else {
                continue;
            };
            state.edit.fit(&text);
            let room = (rect.w - 2.0 * field::PAD).max(1.0);
            if caret - state.scroll_x > room {
                state.scroll_x = caret - room;
            } else if caret < state.scroll_x {
                state.scroll_x = caret;
            }
            state.scroll_x = state.scroll_x.max(0.0);
        }
    }

    fn field_caret_x(&self, id: Id) -> Option<f32> {
        let (field, _) = self.field(id)?;
        let state = self.states.get(&id)?;
        let shown = self.paragraphs.get(&id)?.first()?;
        Some(
            shown
                .caret(field::to_shown(field, state.edit.cursor.min(field.text.len())))
                .x,
        )
    }

    /// The focused field's caret, on or off as the time says. Its next blink,
    /// while it still blinks.
    pub(super) fn blink(&mut self, now: f64) -> Option<Duration> {
        let id = self.focus?;
        self.field(id)?;
        let state = self.states.get_mut(&id)?;
        let since = now - state.activity;
        if since >= BLINK_FOR {
            state.caret_on = true;
            return None;
        }
        state.caret_on = since % (2.0 * BLINK) < BLINK;
        let next = BLINK - since % BLINK;
        Some(Duration::from_secs_f64(next.max(0.01)))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        el::{El, Size, tree::Tree},
        geometry::Rect,
        input::{Input, Key, KeyState, Keysym, Mods},
    };

    #[derive(Clone, Debug, PartialEq)]
    enum Msg {
        Text(String),
        Go,
    }

    fn tree(text: &str, password: bool) -> Tree<Msg> {
        let mut tree = Tree::default();
        let mut field = El::field(text, "", Some(Msg::Text))
            .on_submit(Msg::Go)
            .width(Size::Fixed(200.0));
        if password {
            field = field.password();
        }
        tree.set(El::column(0.0, vec![field]));
        tree.layout_with(Rect::new(0.0, 0.0, 300.0, 100.0), &mut |_: &El<Msg>, _| (0.0, 48.0));
        tree.input(&press(Keysym::Tab, None, Mods::default()));
        tree
    }

    fn press(keysym: Keysym, text: Option<&str>, mods: Mods) -> Input {
        Input::Key(Key {
            keysym,
            code: 0,
            text: text.map(str::to_owned),
            state: KeyState::Pressed,
            mods,
        })
    }

    fn ctrl() -> Mods {
        Mods {
            ctrl: true,
            ..Mods::default()
        }
    }

    #[test]
    fn typing_goes_in_where_the_cursor_is_and_return_submits() {
        let mut tree = tree("", false);
        assert_eq!(
            tree.input(&press(Keysym::a, Some("a"), Mods::default())).messages,
            vec![Msg::Text("a".into())]
        );
        // The shell takes the change: the description has it next.
        tree.set(El::column(
            0.0,
            vec![El::field("ab", "", Some(Msg::Text)).on_submit(Msg::Go)],
        ));
        tree.layout_with(Rect::new(0.0, 0.0, 300.0, 100.0), &mut |_: &El<Msg>, _| (0.0, 48.0));
        tree.input(&press(Keysym::Left, None, Mods::default()));
        let typed = tree.input(&press(Keysym::x, Some("x"), Mods::default()));
        assert_eq!(
            typed.messages,
            vec![Msg::Text("xab".into())],
            "the cursor was at 1, then home"
        );
        assert_eq!(
            tree.input(&press(Keysym::Return, None, Mods::default())).messages,
            vec![Msg::Go]
        );
    }

    #[test]
    fn ctrl_backspace_takes_a_word_and_tab_still_moves_on() {
        let mut tree = tree("hello world", false);
        tree.input(&press(Keysym::End, None, Mods::default()));
        let gone = tree.input(&press(Keysym::BackSpace, None, ctrl()));
        assert_eq!(gone.messages, vec![Msg::Text("hello ".into())]);
        // Tab is not the field's: it passes to focus, and Escape to the shell.
        assert!(tree.input(&press(Keysym::Escape, None, Mods::default())).escaped);
    }

    #[test]
    fn a_selection_copies_but_never_a_passwords() {
        let mut tree = tree("secret", false);
        let mut all = press(Keysym::a, Some("a"), ctrl());
        if let Input::Key(key) = &mut all {
            key.code = super::KEY_A;
        }
        let mut copy = press(Keysym::c, Some("c"), ctrl());
        if let Input::Key(key) = &mut copy {
            key.code = super::KEY_C;
        }
        tree.input(&all);
        assert_eq!(tree.input(&copy).copy.as_deref(), Some("secret"));
        let mut tree = super::tests::tree("secret", true);
        tree.input(&all);
        assert_eq!(tree.input(&copy).copy, None);
    }

    #[test]
    fn a_paste_goes_in_on_one_line() {
        let mut tree = tree("", false);
        let done = tree.input(&Input::Paste("one\ntwo".into()));
        assert_eq!(done.messages, vec![Msg::Text("one two".into())]);
    }
}

#[cfg(test)]
mod shortcuts {
    use super::*;
    use crate::input::{KeyState, Mods};

    fn key(keysym: Keysym, code: u32) -> Key {
        Key {
            keysym,
            code,
            text: None,
            state: KeyState::Pressed,
            mods: Mods::default(),
        }
    }

    #[test]
    fn a_shortcut_goes_by_its_latin_letter_else_by_its_place() {
        // QWERTY: C where C is.
        assert!(shortcut(&key(Keysym::c, KEY_C), KEY_C, 'c'));
        // Dvorak: the key where QWERTY has C types j: not copy.
        assert!(!shortcut(&key(Keysym::j, KEY_C), KEY_C, 'c'));
        // A Cyrillic layout: no Latin letter, so the place counts.
        assert!(shortcut(&key(Keysym::Cyrillic_es, KEY_C), KEY_C, 'c'));
    }
}
