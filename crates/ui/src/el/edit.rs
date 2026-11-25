//! Editing a line of text: a cursor and a selection in it, moved by
//! character and by word, and what typing, deleting and pasting do to both.
//! Positions are byte offsets, always on a boundary of what a reader sees as
//! one character.

use unicode_segmentation::UnicodeSegmentation;

/// The cursor, and where the selection started: no selection when they
/// meet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Edit {
    pub cursor: usize,
    pub anchor: usize,
}

/// Which way, and how far.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Left,
    Right,
    WordLeft,
    WordRight,
    Home,
    End,
}

impl Edit {
    pub fn at(at: usize) -> Self {
        Self { cursor: at, anchor: at }
    }

    /// What is selected, from its start to its end.
    pub fn selection(&self) -> std::ops::Range<usize> {
        self.cursor.min(self.anchor)..self.cursor.max(self.anchor)
    }

    pub fn has_selection(&self) -> bool {
        self.cursor != self.anchor
    }

    /// Kept inside `text`, on a character's boundary: the text may have
    /// changed under it.
    pub fn fit(&mut self, text: &str) {
        self.cursor = boundary_at_or_before(text, self.cursor.min(text.len()));
        self.anchor = boundary_at_or_before(text, self.anchor.min(text.len()));
    }

    /// Moves the cursor `step` in `text`; with `extend`, the selection
    /// grows or shrinks with it. Without, a selection collapses toward the
    /// side moved to.
    pub fn go(&mut self, text: &str, step: Step, extend: bool) {
        if !extend && self.has_selection() && matches!(step, Step::Left | Step::Right) {
            let range = self.selection();
            *self = Self::at(if step == Step::Left { range.start } else { range.end });
            return;
        }
        self.cursor = match step {
            Step::Left => previous(text, self.cursor),
            Step::Right => next(text, self.cursor),
            Step::WordLeft => word_left(text, self.cursor),
            Step::WordRight => word_right(text, self.cursor),
            Step::Home => 0,
            Step::End => text.len(),
        };
        if !extend {
            self.anchor = self.cursor;
        }
    }

    pub fn select_all(&mut self, text: &str) {
        self.anchor = 0;
        self.cursor = text.len();
    }

    /// The word around the cursor selected: a double click.
    pub fn select_word(&mut self, text: &str) {
        for (at, word) in text.split_word_bound_indices() {
            if self.cursor >= at && self.cursor <= at + word.len() && !word.trim().is_empty() {
                self.anchor = at;
                self.cursor = at + word.len();
                return;
            }
        }
    }

    /// `typed` put where the cursor is, in place of what is selected.
    pub fn insert(&mut self, text: &mut String, typed: &str) {
        let range = self.selection();
        text.replace_range(range.clone(), typed);
        *self = Self::at(range.start + typed.len());
    }

    /// Backspace (`back`) or Delete; by the word with `word`. What is
    /// selected goes first, if anything is.
    pub fn delete(&mut self, text: &mut String, back: bool, word: bool) {
        if !self.has_selection() {
            let to = match (back, word) {
                (true, false) => previous(text, self.cursor),
                (true, true) => word_left(text, self.cursor),
                (false, false) => next(text, self.cursor),
                (false, true) => word_right(text, self.cursor),
            };
            self.anchor = to;
        }
        self.insert(text, "");
    }

    pub fn selected<'a>(&self, text: &'a str) -> &'a str {
        &text[self.selection()]
    }
}

fn boundary_at_or_before(text: &str, at: usize) -> usize {
    text.grapheme_indices(true)
        .map(|(start, _)| start)
        .chain(std::iter::once(text.len()))
        .take_while(|start| *start <= at)
        .last()
        .unwrap_or(0)
}

fn previous(text: &str, at: usize) -> usize {
    text[..at]
        .grapheme_indices(true)
        .next_back()
        .map_or(0, |(start, _)| start)
}

fn next(text: &str, at: usize) -> usize {
    text[at..]
        .graphemes(true)
        .next()
        .map_or(at, |grapheme| at + grapheme.len())
}

/// Back to the start of the word before, over any space between.
fn word_left(text: &str, at: usize) -> usize {
    text[..at]
        .split_word_bound_indices()
        .rev()
        .find(|(_, word)| !word.trim().is_empty())
        .map_or(0, |(start, _)| start)
}

/// On to the end of the word after, over any space between.
fn word_right(text: &str, at: usize) -> usize {
    text[at..]
        .split_word_bound_indices()
        .find(|(_, word)| !word.trim().is_empty())
        .map_or(text.len(), |(start, word)| at + start + word.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_go_by_what_a_reader_sees_as_one_character() {
        let text = "aé👩‍💻b";
        let mut edit = Edit::at(0);
        edit.go(text, Step::Right, false);
        assert_eq!(edit.cursor, 1);
        edit.go(text, Step::Right, false);
        assert_eq!(&text[..edit.cursor], "aé");
        // The emoji is one step, however many code points.
        edit.go(text, Step::Right, false);
        assert_eq!(&text[edit.cursor..], "b");
        edit.go(text, Step::Left, false);
        assert_eq!(&text[..edit.cursor], "aé");
    }

    #[test]
    fn words_by_ctrl_arrows_over_spaces_and_greek() {
        let text = "\u{393}\u{3b5}\u{3b9}\u{3b1},  \u{3ba}\u{3cc}\u{3c3}\u{3bc}\u{3b5} hello";
        let mut edit = Edit::at(text.len());
        edit.go(text, Step::WordLeft, false);
        assert_eq!(&text[edit.cursor..], "hello");
        edit.go(text, Step::WordLeft, false);
        assert_eq!(&text[edit.cursor..], "\u{3ba}\u{3cc}\u{3c3}\u{3bc}\u{3b5} hello");
        edit.go(text, Step::WordRight, false);
        assert_eq!(
            &text[..edit.cursor],
            "\u{393}\u{3b5}\u{3b9}\u{3b1},  \u{3ba}\u{3cc}\u{3c3}\u{3bc}\u{3b5}"
        );
    }

    #[test]
    fn shift_extends_and_a_plain_step_collapses() {
        let text = "hello world";
        let mut edit = Edit::at(0);
        edit.go(text, Step::WordRight, true);
        assert_eq!(edit.selected(text), "hello");
        edit.go(text, Step::Right, false);
        assert_eq!(edit, Edit::at(5));
    }

    #[test]
    fn typing_replaces_the_selection_and_deleting_by_word() {
        let mut text = String::from("hello world");
        let mut edit = Edit::at(0);
        edit.go(&text, Step::WordRight, true);
        edit.insert(&mut text, "goodbye");
        assert_eq!(text, "goodbye world");
        edit.go(&text, Step::End, false);
        edit.delete(&mut text, true, true);
        assert_eq!(text, "goodbye ");
        edit.delete(&mut text, true, false);
        assert_eq!(text, "goodbye");
        edit.select_all(&text);
        edit.delete(&mut text, false, false);
        assert_eq!(text, "");
    }

    #[test]
    fn a_double_click_takes_the_word_and_fit_keeps_it_on_a_boundary() {
        let text = "one two three";
        let mut edit = Edit::at(5);
        edit.select_word(text);
        assert_eq!(edit.selected(text), "two");
        let mut edit = Edit::at(50);
        edit.fit("ok");
        assert_eq!(edit, Edit::at(2));
        // Inside a two byte letter: back to its start.
        let mut edit = Edit::at(1);
        edit.fit("é");
        assert_eq!(edit, Edit::at(0));
    }
}
