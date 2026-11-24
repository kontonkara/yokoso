//! The tree the toolkit keeps beside the shell's descriptions: every
//! element's id, rect and role after layout, and what it keeps between
//! rebuilds — its springs and the like — by its id.

use std::collections::{HashMap, HashSet};

use super::{El, Ink, Kind, TextEl, Type, layout};
use crate::{
    app::{Frame, View},
    geometry::{Color, Rect},
    input::Input,
    motion::{Part, Spring},
    text::{Paragraph, Style, Text},
    theme::Theme,
};

/// An element's id: its key, or its place under its parent, under its
/// parent's id. Stable while the element is where it was.
pub type Id = u64;

const ROOT: Id = 0x9e37_79b9_7f4a_7c15;

/// What an element is, for accessibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Group,
    Text,
    Button,
    Toggle,
    Slider,
    List,
    ListItem,
    TextField,
    Card,
    Tile,
    Menu,
    MenuItem,
}

/// An element after layout.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: Id,
    pub rect: Rect,
    pub role: Role,
    pub parent: Option<usize>,
    /// It takes the pointer: buttons and the like.
    pub interactive: bool,
}

/// What an element keeps between rebuilds.
#[derive(Clone, Debug)]
pub struct State {
    pub hover: Spring,
    pub press: Spring,
    pub focus: Spring,
    /// For a toggle's thumb, a slider's handle and the like.
    pub value: Spring,
}

impl Default for State {
    fn default() -> Self {
        Self {
            hover: Spring::new(0.0),
            press: Spring::new(0.0),
            focus: Spring::new(0.0),
            value: Spring::new(0.0),
        }
    }
}

impl State {
    fn springs(&mut self) -> [&mut Spring; 4] {
        [&mut self.hover, &mut self.press, &mut self.focus, &mut self.value]
    }
}

/// What came of input: the shell's messages, and whether anything shows a
/// change.
pub struct Response<M> {
    pub messages: Vec<M>,
    pub redraw: bool,
}

pub struct Tree<M> {
    el: Option<El<M>>,
    nodes: Vec<Node>,
    states: HashMap<Id, State>,
    /// Text laid out, by element.
    paragraphs: HashMap<Id, Paragraph>,
    /// The last view laid out for, and whether the description changed since.
    view: Option<View>,
    stale: bool,
    pub hovered: Option<Id>,
    pub pressed: Option<Id>,
}

impl<M> Default for Tree<M> {
    fn default() -> Self {
        Self {
            el: None,
            nodes: Vec::new(),
            states: HashMap::new(),
            paragraphs: HashMap::new(),
            view: None,
            stale: true,
            hovered: None,
            pressed: None,
        }
    }
}

/// Walks `el` in pre-order with each element's id and its parent's index.
fn walk<'a, M>(el: &'a El<M>, id: Id, parent: Option<usize>, out: &mut Vec<(&'a El<M>, Id, Option<usize>)>) {
    let here = out.len();
    out.push((el, id, parent));
    for (index, child) in el.children.iter().enumerate() {
        let name = child.key.unwrap_or(index as u64 ^ 0x5bd1_e995);
        walk(child, mix(id, name), Some(here), out);
    }
}

fn mix(parent: Id, name: u64) -> Id {
    let mut x = parent ^ name.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

fn role<M>(kind: &Kind<M>) -> Role {
    match kind {
        Kind::Text(_) => Role::Text,
        _ => Role::Group,
    }
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

impl<M: Clone> Tree<M> {
    /// The shell's new description: laid out before the next paint.
    pub fn set(&mut self, el: El<M>) {
        self.el = Some(el);
        self.stale = true;
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn state(&self, id: Id) -> Option<&State> {
        self.states.get(&id)
    }

    /// Whether layout has to run for `view`.
    pub fn needs_layout(&self, view: &View) -> bool {
        self.stale || self.view.as_ref() != Some(view)
    }

    /// Moves every spring `dt` seconds on. True while one still moves.
    pub fn update(&mut self, dt: f32, theme: &Theme) -> bool {
        let feel = theme.motion.feel(Part::Micro);
        let mut moving = false;
        for state in self.states.values_mut() {
            for spring in state.springs() {
                moving |= spring.step(dt, &feel);
            }
        }
        moving
    }

    /// Lays the description out for `view`, text at its scale.
    pub fn layout(&mut self, view: &View, text: &mut Text, theme: &Theme) {
        let scale = view.scale;
        self.layout_with(
            Rect::new(0.0, 0.0, view.width, view.height),
            &mut |el: &El<M>, max| match &el.kind {
                Kind::Text(t) => {
                    let wrap = if t.ellipsis { None } else { max };
                    let laid = text.layout(&t.text, &style(t, theme), scale, wrap, false);
                    (laid.width().ceil().min(max.unwrap_or(f32::MAX)), laid.height().ceil())
                }
                _ => (0.0, 0.0),
            },
        );
        // Text laid out once more, at the width it got.
        self.paragraphs.clear();
        if let Some(el) = &self.el {
            let mut all = Vec::new();
            walk(el, ROOT, None, &mut all);
            for ((el, id, _), node) in all.iter().zip(&self.nodes) {
                if let Kind::Text(t) = &el.kind {
                    let paragraph = text.layout(&t.text, &style(t, theme), scale, Some(node.rect.w + 0.5), t.ellipsis);
                    self.paragraphs.insert(*id, paragraph);
                }
            }
        }
        self.view = Some(*view);
        self.stale = false;
    }

    /// Layout with any measure: for tests, which have no fonts to ask.
    pub fn layout_with(&mut self, space: Rect, measure: &mut impl layout::Measure<M>) {
        self.nodes.clear();
        let Some(el) = &self.el else { return };
        let rects = layout::layout(el, space, measure);
        let mut all = Vec::new();
        walk(el, ROOT, None, &mut all);
        let mut seen = HashSet::new();
        for ((el, id, parent), rect) in all.into_iter().zip(rects) {
            seen.insert(id);
            self.states.entry(id).or_default();
            self.nodes.push(Node {
                id,
                rect,
                role: role(&el.kind),
                parent,
                interactive: false,
            });
        }
        // What is gone forgets its state.
        self.states.retain(|id, _| seen.contains(id));
        if self.hovered.is_some_and(|id| !seen.contains(&id)) {
            self.hovered = None;
        }
        if self.pressed.is_some_and(|id| !seen.contains(&id)) {
            self.pressed = None;
        }
    }

    /// The topmost element that takes the pointer at `x`, `y`.
    pub fn hit(&self, x: f32, y: f32) -> Option<Id> {
        self.nodes
            .iter()
            .rev()
            .find(|node| node.interactive && node.rect.contains(x, y))
            .map(|node| node.id)
    }

    /// Takes `input`: what it hovers and presses changes, and the shell
    /// hears what the elements make of it.
    pub fn input(&mut self, input: &Input) -> Response<M> {
        let mut redraw = false;
        match input {
            Input::PointerEnter { x, y } | Input::PointerMove { x, y } => {
                let now = self.hit(*x, *y);
                if now != self.hovered {
                    self.set_spring(self.hovered, |s| s.hover.to(0.0));
                    self.set_spring(now, |s| s.hover.to(1.0));
                    self.hovered = now;
                    redraw = true;
                }
            }
            Input::PointerLeave => {
                let gone = self.hovered.take();
                self.set_spring(gone, |s| s.hover.to(0.0));
                redraw = true;
            }
            _ => {}
        }
        Response {
            messages: Vec::new(),
            redraw,
        }
    }

    fn set_spring(&mut self, id: Option<Id>, change: impl FnOnce(&mut State)) {
        if let Some(state) = id.and_then(|id| self.states.get_mut(&id)) {
            change(state);
        }
    }

    /// Paints every element, the state only read.
    pub fn paint(&self, frame: &mut Frame, theme: &Theme) {
        let Some(el) = &self.el else { return };
        let mut all = Vec::new();
        walk(el, ROOT, None, &mut all);
        for ((el, id, _), node) in all.iter().zip(&self.nodes) {
            if let Kind::Text(t) = &el.kind
                && let Some(paragraph) = self.paragraphs.get(id)
            {
                // Centered in its rect up and down: a line in a taller box.
                let y = node.rect.y + ((node.rect.h - paragraph.height()) / 2.0).max(0.0);
                frame.text(paragraph, node.rect.x, y, ink(t.ink, theme));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::el::{Size, Type};

    fn measure(el: &El<()>, _: Option<f32>) -> (f32, f32) {
        match &el.kind {
            Kind::Text(text) => (text.text.len() as f32 * 7.0, 17.0),
            _ => (0.0, 0.0),
        }
    }

    fn ids(tree: &Tree<()>) -> Vec<Id> {
        tree.nodes().iter().map(|node| node.id).collect()
    }

    #[test]
    fn ids_stay_while_elements_stay_and_keys_follow_their_element() {
        let mut tree = Tree::<()>::default();
        let space = Rect::new(0.0, 0.0, 300.0, 300.0);
        tree.set(El::column(
            0.0,
            vec![El::text("a", Type::Body).key("a"), El::text("b", Type::Body).key("b")],
        ));
        tree.layout_with(space, &mut measure);
        let first = ids(&tree);
        // The same again: the same ids.
        tree.set(El::column(
            0.0,
            vec![El::text("a", Type::Body).key("a"), El::text("b", Type::Body).key("b")],
        ));
        tree.layout_with(space, &mut measure);
        assert_eq!(ids(&tree), first);
        // Swapped: each keeps its own.
        tree.set(El::column(
            0.0,
            vec![El::text("b", Type::Body).key("b"), El::text("a", Type::Body).key("a")],
        ));
        tree.layout_with(space, &mut measure);
        assert_eq!(ids(&tree), vec![first[0], first[2], first[1]]);
    }

    #[test]
    fn a_gone_element_forgets_its_state() {
        let mut tree = Tree::<()>::default();
        let space = Rect::new(0.0, 0.0, 300.0, 300.0);
        tree.set(El::column(
            0.0,
            vec![El::spacer().width(Size::Fixed(10.0)), El::text("x", Type::Body)],
        ));
        tree.layout_with(space, &mut measure);
        assert_eq!(tree.states.len(), 3);
        tree.set(El::column(0.0, vec![El::spacer()]));
        tree.layout_with(space, &mut measure);
        assert_eq!(tree.states.len(), 2);
    }

    #[test]
    fn roles_name_what_elements_are() {
        let mut tree = Tree::<()>::default();
        tree.set(El::column(0.0, vec![El::text("x", Type::Body)]));
        tree.layout_with(Rect::new(0.0, 0.0, 100.0, 100.0), &mut measure);
        let roles: Vec<Role> = tree.nodes().iter().map(|node| node.role).collect();
        assert_eq!(roles, vec![Role::Group, Role::Text]);
    }
}
