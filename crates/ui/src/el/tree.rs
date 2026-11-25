//! The tree the toolkit keeps beside the shell's descriptions: every
//! element's id, rect and role after layout, and what it keeps between
//! rebuilds — its springs and the like — by its id.

mod field;
mod list;
mod menu;

use std::collections::{HashMap, HashSet};

use super::{El, Kind, SliderEl, kinds, layout};
use crate::{
    app::{Frame, View},
    geometry::Rect,
    input::{Input, KeyState, Keysym},
    motion::{Part, Spring},
    text::{Paragraph, Text},
    theme::Theme,
};

/// How long the pointer rests before a tooltip shows.
const TOOLTIP_DELAY: std::time::Duration = std::time::Duration::from_millis(600);

/// The evdev code of the left button.
const BTN_LEFT: u32 = 0x110;

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
    Tooltip,
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
    /// The keyboard reaches it.
    pub focusable: bool,
    /// Where it shows, inside a list it scrolls in.
    pub clip: Option<Rect>,
}

/// What an element keeps between rebuilds.
#[derive(Clone, Debug)]
pub struct State {
    pub hover: Spring,
    pub press: Spring,
    pub focus: Spring,
    /// For a toggle's thumb, a slider's handle and the like.
    pub value: Spring,
    /// Seconds into a pulse, while the element waits on something.
    pub phase: f32,
    /// Its springs were set from a description once: after that they move
    /// to new values rather than jump.
    pub known: bool,
    /// A list's: how far it is scrolled, its fling's speed and whether it
    /// flings, when fingers last touched it, how far it can scroll, and the
    /// row the pointer is on.
    pub scroll: Spring,
    pub fling: f32,
    pub flinging: bool,
    pub touched: f64,
    pub max_scroll: f32,
    pub hover_row: Option<usize>,
    /// A field's: whether it is being edited, its cursor and selection, how
    /// far its text is scrolled, whether its caret shows, when it was last
    /// touched, its last click (when and where) and Caps Lock.
    pub editing: bool,
    pub edit: super::edit::Edit,
    pub scroll_x: f32,
    pub caret_on: bool,
    pub activity: f64,
    pub clicked: (f64, f32),
    pub caps: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            hover: Spring::new(0.0),
            press: Spring::new(0.0),
            focus: Spring::new(0.0),
            value: Spring::new(0.0),
            phase: 0.0,
            known: false,
            scroll: Spring::pixels(0.0),
            fling: 0.0,
            flinging: false,
            touched: 0.0,
            max_scroll: 0.0,
            hover_row: None,
            editing: false,
            edit: super::edit::Edit::default(),
            scroll_x: 0.0,
            caret_on: true,
            activity: 0.0,
            clicked: (0.0, 0.0),
            caps: false,
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
    /// Ask for a frame this long from now, with none before: a tooltip
    /// waiting to show.
    pub wake: Option<std::time::Duration>,
    /// Escape went unused here: the shell unwinds a level.
    pub escaped: bool,
    /// Text to put on the clipboard.
    pub copy: Option<String>,
    /// The clipboard's text is wanted, to come back as `Input::Paste`.
    pub paste: bool,
}

pub struct Tree<M> {
    el: Option<El<M>>,
    nodes: Vec<Node>,
    states: HashMap<Id, State>,
    /// Text laid out, by element.
    paragraphs: HashMap<Id, Vec<Paragraph>>,
    /// The last view laid out for, and whether the description changed since.
    view: Option<View>,
    stale: bool,
    pub hovered: Option<Id>,
    pub pressed: Option<Id>,
    /// Where the last press was, across: which part of a tile it hit.
    press_x: f32,
    /// Elements waiting on something, which pulse meanwhile.
    pending: HashSet<Id>,
    /// The tooltip the pointer rests on, and since when (monotonic seconds).
    tip: Option<(Id, f64)>,
    pub focus: Option<Id>,
    /// The focus ring shows: a key was pressed since the last click.
    pub focus_visible: bool,
    /// The modifiers as the last key had them: Shift for a click.
    mods: crate::input::Mods,
    /// The focused field's next blink.
    blink_in: Option<std::time::Duration>,
    /// The menu open now, and whether the release of the press that closed
    /// one is still to come, to be swallowed.
    menu_open: Option<Id>,
    swallow_release: bool,
    /// The theme of the last layout, for what input measures.
    theme: Theme,
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
            press_x: 0.0,
            tip: None,
            focus: None,
            focus_visible: false,
            pending: HashSet::new(),
            mods: crate::input::Mods::default(),
            blink_in: None,
            menu_open: None,
            swallow_release: false,
            theme: Theme::default(),
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

    /// How many tooltips show now, more than half faded in.
    pub fn tooltips_shown(&self) -> usize {
        self.nodes
            .iter()
            .filter(|node| node.role == Role::Tooltip)
            .filter(|node| self.states.get(&node.id).is_some_and(|s| s.value.value > 0.5))
            .count()
    }

    /// Moves every spring `dt` seconds on. True while one still moves.
    pub fn update(&mut self, dt: f32, now: f64, theme: &Theme) -> bool {
        let feel = theme.motion.feel(Part::Micro);
        let mut moving = false;
        // A tooltip shows once the pointer has rested long enough.
        if let Some((id, since)) = self.tip
            && now - since >= TOOLTIP_DELAY.as_secs_f64() - 0.01
            && let Some(state) = self.states.get_mut(&id)
        {
            state.value.to(1.0);
        }
        // What waits pulses, and keeps frames coming while it does.
        for id in &self.pending {
            if let Some(state) = self.states.get_mut(id) {
                state.phase += dt;
                moving = true;
            }
        }
        for state in self.states.values_mut() {
            for spring in state.springs() {
                moving |= spring.step(dt, &feel);
            }
        }
        moving |= self.update_lists(dt, theme);
        self.blink_in = self.blink(now);
        moving
    }

    /// How long until a frame is wanted though nothing moves: a blink.
    pub fn wake(&self) -> Option<std::time::Duration> {
        self.blink_in
    }

    /// Lays the description out for `view`, text at its scale.
    pub fn layout(&mut self, view: &View, text: &mut Text, theme: &Theme) {
        let scale = view.scale;
        let space = Rect::new(0.0, 0.0, view.width, view.height);
        self.layout_with(space, &mut |el: &El<M>, max| {
            kinds::measure(el, max, text, theme, scale)
        });
        // Lists build the rows now in view, and all is laid out again.
        if self.has_lists() {
            self.expand_lists();
            self.layout_with(space, &mut |el: &El<M>, max| {
                kinds::measure(el, max, text, theme, scale)
            });
        }
        // Text laid out once more, at the width it got.
        self.paragraphs.clear();
        if let Some(el) = &self.el {
            let mut all = Vec::new();
            walk(el, ROOT, None, &mut all);
            for ((el, id, _), node) in all.iter().zip(&self.nodes) {
                let laid = kinds::paragraphs(el, node.rect, text, theme, scale);
                if !laid.is_empty() {
                    self.paragraphs.insert(*id, laid);
                }
            }
        }
        self.theme = theme.clone();
        self.settle_fields();
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
        self.pending.clear();
        // What each element lets its children show in.
        let mut clips: Vec<Option<Rect>> = Vec::with_capacity(rects.len());
        for ((el, id, parent), rect) in all.into_iter().zip(rects) {
            let clip = parent.and_then(|parent| clips[parent]);
            clips.push(match &el.kind {
                Kind::List(_) => Some(clip.map_or(rect, |clip| clip.intersect(&rect))),
                _ => clip,
            });
            seen.insert(id);
            let state = self.states.entry(id).or_default();
            kinds::settle(&el.kind, state);
            if kinds::pending(&el.kind) {
                self.pending.insert(id);
            } else {
                state.phase = 0.0;
            }
            self.nodes.push(Node {
                id,
                rect,
                role: kinds::role(&el.kind),
                parent,
                interactive: kinds::interactive(&el.kind),
                focusable: kinds::focusable(&el.kind),
                clip,
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
        if self
            .focus
            .is_some_and(|id| !self.nodes.iter().any(|node| node.id == id && node.focusable))
        {
            self.focus = None;
        }
    }

    /// The topmost element that takes the pointer at `x`, `y`.
    pub fn hit(&self, x: f32, y: f32) -> Option<Id> {
        self.nodes
            .iter()
            .rev()
            .find(|node| {
                node.interactive && node.rect.contains(x, y) && node.clip.is_none_or(|clip| clip.contains(x, y))
            })
            .map(|node| node.id)
    }

    /// Takes `input`: what it hovers and presses changes, and the shell
    /// hears what the elements make of it.
    pub fn input(&mut self, input: &Input) -> Response<M> {
        let mut redraw = false;
        let mut escaped = false;
        let mut wake = None;
        let mut copy = None;
        let mut paste = false;
        let mut messages = Vec::new();
        // An open menu takes all input; the release of the press that
        // closed one is swallowed.
        let theme = self.theme.clone();
        if let Some((menu_messages, menu_redraw)) = self.menu_input(input, &theme) {
            messages = menu_messages;
            redraw = menu_redraw;
        } else if self.swallow_release && matches!(input, Input::Release { .. }) {
            self.swallow_release = false;
            let pressed = self.pressed.take();
            self.set_spring(pressed, |s| s.press.to(0.0));
            redraw = true;
        } else {
            match input {
                Input::PointerEnter { x, y } | Input::PointerMove { x, y } => {
                    // Dragging a slider: its value follows the pointer. In a field,
                    // the selection does.
                    if let Some(change) = self.pressed.and_then(|id| self.slide_to(id, *x)) {
                        messages.push(change);
                    }
                    if let Some(id) = self.pressed.filter(|id| self.field(*id).is_some()) {
                        self.field_drag(id, *x);
                        redraw = true;
                    }
                    let now = self.hit(*x, *y);
                    if let Some(id) = now {
                        redraw |= self.list_hover(id, Some(*y));
                    }
                    if now != self.hovered {
                        if let Some(gone) = self.hovered {
                            self.list_hover(gone, None);
                        }
                        self.set_spring(self.hovered, |s| s.hover.to(0.0));
                        self.set_spring(now, |s| s.hover.to(1.0));
                        self.hovered = now;
                        redraw = true;
                        wake = self.rest_on(now);
                    }
                }
                Input::PointerLeave => {
                    let gone = self.hovered.take();
                    self.set_spring(gone, |s| s.hover.to(0.0));
                    self.rest_on(None);
                    redraw = true;
                }
                Input::Press {
                    button: BTN_LEFT, x, ..
                } => {
                    // A click focuses without a ring.
                    self.focus_visible = false;
                    self.move_focus(self.hovered);
                    self.press_x = *x;
                    // A click puts a tooltip away until the pointer rests again.
                    self.rest_on(None);
                    redraw = true;
                    if let Some(id) = self.hovered {
                        self.pressed = Some(id);
                        self.set_spring(Some(id), |s| s.press.to(1.0));
                        redraw = true;
                        messages.extend(self.slide_to(id, *x));
                        if self.field(id).is_some() {
                            let shift = self.mods.shift;
                            self.field_press(id, *x, shift);
                        }
                    }
                }
                Input::Release { button: BTN_LEFT, x, y } => {
                    if let Some(id) = self.pressed.take() {
                        self.set_spring(Some(id), |s| s.press.to(0.0));
                        redraw = true;
                        // A click is a press and a release on the same element.
                        if self.hit(*x, *y) == Some(id) {
                            let more = self.more_at(id, self.press_x);
                            messages.extend(self.activate(id, more));
                            messages.extend(self.list_click(id, *y));
                            if self.menu(id).is_some() {
                                self.open_menu(id);
                            }
                        }
                    }
                }
                Input::Scroll(scroll) => {
                    if let Some(id) = self.hovered
                        && self.list(id).is_some()
                    {
                        if self.list_scroll(id, scroll) {
                            self.stale = true;
                            redraw = true;
                        }
                    } else if let Some(id) = self.hovered
                        && let Some((slider, track)) = self.slider(id)
                    {
                        // Up and right is more. A wheel goes by steps, fingers by
                        // the pixel along the track.
                        let by = if scroll.steps_y != 0.0 || scroll.steps_x != 0.0 {
                            (scroll.steps_x - scroll.steps_y) * slider.step.max((slider.max - slider.min) / 100.0)
                        } else {
                            (scroll.x - scroll.y) / track.w.max(1.0) * (slider.max - slider.min)
                        };
                        let value = slider.fit(slider.value + by);
                        if value != slider.value
                            && let Some(change) = &slider.on_change
                        {
                            messages.push(change(value));
                        }
                    }
                }
                Input::Paste(text) => {
                    if let Some(id) = self.focus.filter(|id| self.field(*id).is_some()) {
                        messages.extend(self.field_paste(id, text));
                        redraw = true;
                    }
                }
                Input::Key(key)
                    if key.state != KeyState::Released
                        && self.focus.is_some_and(|id| self.field(id).is_some())
                        && {
                            // A field takes its keys first; what it passes on goes on below.
                            let id = self.focus.expect("checked");
                            self.mods = key.mods;
                            let out = self.field_key(id, key);
                            messages.extend(out.message);
                            copy = out.copy;
                            paste = out.paste;
                            redraw = true;
                            !out.passed
                        } => {}
                Input::Key(key) if key.state != KeyState::Released => {
                    redraw = true;
                    self.mods = key.mods;
                    let was_visible = self.focus_visible;
                    self.focus_visible = true;
                    let on_menu = self.focus.filter(|id| self.menu(*id).is_some());
                    let on_list = self.focus.filter(|id| self.list(*id).is_some());
                    if let Some(id) = on_menu
                        && matches!(
                            key.keysym,
                            Keysym::Down | Keysym::Return | Keysym::KP_Enter | Keysym::space
                        )
                    {
                        self.open_menu(id);
                    } else if let Some(id) = on_list
                        && let Some(change) = self.list_key(id, key.keysym)
                    {
                        messages.push(change);
                    } else if let Some(change) = self.focus.and_then(|id| self.slider_key(id, key.keysym)) {
                        messages.push(change);
                    } else {
                        match key.keysym {
                            Keysym::Tab if !key.mods.shift => self.step_focus(1, false),
                            Keysym::Tab | Keysym::ISO_Left_Tab => self.step_focus(-1, false),
                            Keysym::Right | Keysym::Down => self.step_focus(1, true),
                            Keysym::Left | Keysym::Up => self.step_focus(-1, true),
                            Keysym::Return | Keysym::KP_Enter | Keysym::space => {
                                // Shift+Return goes into a tile's details.
                                if let Some(id) = self.focus {
                                    messages.extend(self.activate(id, key.mods.shift));
                                }
                            }
                            Keysym::Escape => escaped = true,
                            _ => {}
                        }
                    }
                    if !was_visible {
                        let focus = self.focus;
                        self.set_spring(focus, |s| s.focus.to(1.0));
                    }
                }
                Input::KeyboardLeave => {
                    self.focus_visible = false;
                    let focus = self.focus;
                    self.set_spring(focus, |s| s.focus.to(0.0));
                    redraw = true;
                }
                _ => {}
            }
        }
        Response {
            messages,
            redraw,
            wake,
            escaped,
            copy,
            paste,
        }
    }

    /// The pointer now on `hovered`: the tooltip it is in, if any, starts its
    /// wait, and the one it left goes. The wait, where one starts.
    fn rest_on(&mut self, hovered: Option<Id>) -> Option<std::time::Duration> {
        let tip = hovered.and_then(|id| self.tooltip_of(id));
        if tip == self.tip.map(|(id, _)| id) {
            return None;
        }
        if let Some((old, _)) = self.tip.take() {
            self.set_spring(Some(old), |s| s.value.to(0.0));
        }
        let tip = tip?;
        self.tip = Some((tip, crate::app::monotonic()));
        Some(TOOLTIP_DELAY)
    }

    /// The tooltip `id` is in: itself, or the nearest one above it.
    fn tooltip_of(&self, id: Id) -> Option<Id> {
        let mut at = self.nodes.iter().position(|node| node.id == id);
        while let Some(index) = at {
            if self.nodes[index].role == Role::Tooltip {
                return Some(self.nodes[index].id);
            }
            at = self.nodes[index].parent;
        }
        None
    }

    /// Focus to `id`: its ring fades in where rings show, the old one's out.
    fn move_focus(&mut self, id: Option<Id>) {
        let old = self.focus;
        if old == id {
            return;
        }
        self.set_spring(old, |s| {
            s.focus.to(0.0);
            s.editing = false;
        });
        let shown = if self.focus_visible { 1.0 } else { 0.0 };
        self.set_spring(id, |s| s.focus.to(shown));
        self.focus = id;
        // A field focused is a field being edited.
        if let Some(id) = id.filter(|id| self.field(*id).is_some()) {
            self.set_spring(Some(id), |s| s.editing = true);
            self.touch_field(id);
        }
    }

    /// Focus to the next (`by` 1) or the previous (-1) element that takes
    /// it; with `sibling`, only among the focused one's siblings, as arrows
    /// move in a row or a column. Tab wraps around; arrows stop at the ends.
    fn step_focus(&mut self, by: i32, sibling: bool) {
        let at = self
            .focus
            .and_then(|id| self.nodes.iter().position(|node| node.id == id));
        let parent = at.and_then(|at| self.nodes[at].parent);
        let order: Vec<usize> = (0..self.nodes.len())
            .filter(|&index| self.nodes[index].focusable)
            .filter(|&index| !sibling || at.is_none() || self.nodes[index].parent == parent)
            .collect();
        if order.is_empty() {
            return;
        }
        let here = at.and_then(|at| order.iter().position(|&index| index == at));
        let next = match (here, sibling) {
            (None, _) => {
                if by > 0 {
                    0
                } else {
                    order.len() - 1
                }
            }
            (Some(here), false) => (here as i32 + by).rem_euclid(order.len() as i32) as usize,
            (Some(here), true) => (here as i32 + by).clamp(0, order.len() as i32 - 1) as usize,
        };
        let id = self.nodes[order[next]].id;
        self.move_focus(Some(id));
    }

    /// Whether any list is in the description.
    fn has_lists(&self) -> bool {
        self.nodes.iter().any(|node| node.role == Role::List) || self.nodes.is_empty()
    }

    /// The element `id`, and its rect.
    fn element(&self, id: Id) -> Option<(&El<M>, Rect)> {
        let el = self.el.as_ref()?;
        let mut all = Vec::new();
        walk(el, ROOT, None, &mut all);
        let at = all.iter().position(|(_, found, _)| *found == id)?;
        Some((all[at].0, self.nodes.get(at)?.rect))
    }

    /// The slider `id`, and where its handle runs.
    fn slider(&self, id: Id) -> Option<(&SliderEl<M>, Rect)> {
        let el = self.el.as_ref()?;
        let mut all = Vec::new();
        walk(el, ROOT, None, &mut all);
        let at = all.iter().position(|(_, found, _)| *found == id)?;
        match &all[at].0.kind {
            Kind::Slider(slider) if slider.on_change.is_some() => {
                Some((slider, kinds::slider_track(slider, self.nodes.get(at)?.rect)))
            }
            _ => None,
        }
    }

    /// The slider `id` set to the value at `x`, if it is a slider and the
    /// value is new.
    fn slide_to(&self, id: Id, x: f32) -> Option<M> {
        let (slider, track) = self.slider(id)?;
        let value = kinds::slider_value_at(slider, track, x);
        let change = slider.on_change.as_ref()?;
        (value != slider.value).then(|| change(value))
    }

    /// A key on the focused slider: arrows by a step, pages by ten, Home
    /// and End to the ends.
    fn slider_key(&self, id: Id, keysym: Keysym) -> Option<M> {
        let (slider, _) = self.slider(id)?;
        let step = slider.step.max((slider.max - slider.min) / 100.0);
        let value = match keysym {
            Keysym::Right | Keysym::Up => slider.value + step,
            Keysym::Left | Keysym::Down => slider.value - step,
            Keysym::Page_Up => slider.value + 10.0 * step,
            Keysym::Page_Down => slider.value - 10.0 * step,
            Keysym::Home => slider.min,
            Keysym::End => slider.max,
            _ => return None,
        };
        let value = slider.fit(value);
        // At an end already: the key is spent all the same.
        Some((slider.on_change.as_ref()?)(value))
    }

    /// Whether `x` is on the arrow into a tile's details.
    fn more_at(&self, id: Id, x: f32) -> bool {
        let Some(el) = &self.el else { return false };
        let mut all = Vec::new();
        walk(el, ROOT, None, &mut all);
        let Some(at) = all.iter().position(|(_, found, _)| *found == id) else {
            return false;
        };
        match (&all[at].0.kind, self.nodes.get(at)) {
            (Kind::Tile(tile), Some(node)) => kinds::tile_more_at(tile, node.rect, x),
            _ => false,
        }
    }

    /// What the element `id` sends when it is used: a button's message, a
    /// tile's, or with `more` its way into details.
    fn activate(&self, id: Id, more: bool) -> Option<M> {
        let el = self.el.as_ref()?;
        let mut all = Vec::new();
        walk(el, ROOT, None, &mut all);
        let (el, _, _) = all.into_iter().find(|(_, found, _)| *found == id)?;
        match &el.kind {
            Kind::Tile(tile) if more && tile.on_more.is_some() => tile.on_more.clone(),
            Kind::Tile(tile) if !tile.pending => tile.on_press.clone(),
            super::Kind::Button { on_press, .. } => on_press.clone(),
            super::Kind::Toggle {
                on_toggle,
                pending: false,
                ..
            } => on_toggle.clone(),
            _ => None,
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
        let mut clip = None;
        let mut popups = Vec::new();
        for ((el, id, _), node) in all.iter().zip(&self.nodes) {
            let Some(state) = self.states.get(id) else { continue };
            let focused = state.focus.value > 0.0;
            let laid = self.paragraphs.get(id).map_or(&[][..], Vec::as_slice);
            if node.clip != clip {
                clip = node.clip;
                frame.clip(clip);
            }
            kinds::paint(el, node.rect, state, laid, focused, frame, theme);
            // A field clips its own text: its element's clip comes back after.
            if node.role == Role::TextField {
                frame.clip(clip);
            }
            if let Kind::Menu(menu) = &el.kind
                && state.value.value > 0.0
            {
                popups.push((menu, node.rect, state, laid));
            }
        }
        frame.clip(None);
        // Open menus over everything but tooltips.
        let bounds = self.bounds();
        for (menu, rect, state, laid) in popups {
            super::menu::paint_popup(menu, rect, bounds, state, laid, frame, theme);
        }
        frame.clip(None);
        // Tooltips last, over everything.
        let view = frame.view();
        let bounds = Rect::new(0.0, 0.0, view.width, view.height);
        for node in self.nodes.iter().filter(|node| node.role == Role::Tooltip) {
            if let (Some(state), Some([paragraph, ..])) = (
                self.states.get(&node.id),
                self.paragraphs.get(&node.id).map(Vec::as_slice),
            ) {
                kinds::paint_tooltip(node.rect, bounds, state, paragraph, frame, theme);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::el::{Kind, Size, Type};

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
    fn a_click_is_a_press_and_a_release_on_the_same_button() {
        use crate::el::ButtonStyle;
        let mut tree = Tree::<u8>::default();
        tree.set(El::row(
            8.0,
            vec![
                El::button("one", ButtonStyle::Filled, Some(1))
                    .width(Size::Fixed(50.0))
                    .height(Size::Fixed(20.0)),
                El::button("off", ButtonStyle::Filled, None)
                    .width(Size::Fixed(50.0))
                    .height(Size::Fixed(20.0)),
            ],
        ));
        tree.layout_with(Rect::new(0.0, 0.0, 200.0, 20.0), &mut |_: &El<u8>, _| (0.0, 0.0));
        let at = |x| Input::PointerMove { x, y: 10.0 };
        tree.input(&at(10.0));
        tree.input(&Input::Press {
            button: BTN_LEFT,
            x: 10.0,
            y: 10.0,
        });
        let done = tree.input(&Input::Release {
            button: BTN_LEFT,
            x: 10.0,
            y: 10.0,
        });
        assert_eq!(done.messages, vec![1]);
        // Pressed on it, released away: nothing.
        tree.input(&Input::Press {
            button: BTN_LEFT,
            x: 10.0,
            y: 10.0,
        });
        tree.input(&at(150.0));
        let done = tree.input(&Input::Release {
            button: BTN_LEFT,
            x: 150.0,
            y: 10.0,
        });
        assert!(done.messages.is_empty());
        // A disabled button takes no pointer at all.
        assert_eq!(tree.hit(80.0, 10.0), None);
    }

    fn key(keysym: Keysym, shift: bool) -> Input {
        Input::Key(crate::input::Key {
            keysym,
            code: 0,
            text: None,
            state: KeyState::Pressed,
            mods: crate::input::Mods {
                shift,
                ..Default::default()
            },
        })
    }

    #[test]
    fn tab_walks_the_buttons_round_and_enter_presses_the_focused_one() {
        use crate::el::ButtonStyle;
        let mut tree = Tree::<u8>::default();
        let button = |n: u8| {
            El::button("b", ButtonStyle::Tonal, Some(n))
                .width(Size::Fixed(40.0))
                .height(Size::Fixed(20.0))
        };
        tree.set(El::column(
            0.0,
            vec![El::row(4.0, vec![button(1), button(2)]), button(3)],
        ));
        tree.layout_with(Rect::new(0.0, 0.0, 200.0, 100.0), &mut |_: &El<u8>, _| (0.0, 0.0));
        let press = |tree: &mut Tree<u8>, keysym, shift| tree.input(&key(keysym, shift));
        assert!(!tree.focus_visible);
        press(&mut tree, Keysym::Tab, false);
        assert!(tree.focus_visible);
        assert_eq!(press(&mut tree, Keysym::Return, false).messages, vec![1]);
        press(&mut tree, Keysym::Tab, false);
        press(&mut tree, Keysym::Tab, false);
        assert_eq!(press(&mut tree, Keysym::space, false).messages, vec![3]);
        // Round to the first, and back.
        press(&mut tree, Keysym::Tab, false);
        assert_eq!(press(&mut tree, Keysym::Return, false).messages, vec![1]);
        press(&mut tree, Keysym::ISO_Left_Tab, true);
        assert_eq!(press(&mut tree, Keysym::Return, false).messages, vec![3]);
        // Arrows stay among siblings and stop at the ends.
        press(&mut tree, Keysym::Tab, false);
        press(&mut tree, Keysym::Right, false);
        press(&mut tree, Keysym::Right, false);
        assert_eq!(press(&mut tree, Keysym::Return, false).messages, vec![2]);
        // A click hides the ring; Escape goes to the shell.
        tree.input(&Input::PointerMove { x: 5.0, y: 5.0 });
        tree.input(&Input::Press {
            button: BTN_LEFT,
            x: 5.0,
            y: 5.0,
        });
        assert!(!tree.focus_visible);
        assert!(press(&mut tree, Keysym::Escape, false).escaped);
    }

    #[test]
    fn a_slider_follows_a_drag_steps_by_wheel_and_keys() {
        let mut tree = Tree::<f32>::default();
        tree.set(El::row(
            0.0,
            vec![
                El::slider(20.0, 0.0, 100.0, 1.0, Some(|v| v))
                    .width(Size::Fixed(104.0))
                    .height(Size::Fixed(44.0)),
            ],
        ));
        tree.layout_with(Rect::new(0.0, 0.0, 200.0, 44.0), &mut |_: &El<f32>, _| (0.0, 0.0));
        tree.input(&Input::PointerMove { x: 52.0, y: 20.0 });
        assert_eq!(
            tree.input(&Input::Press {
                button: BTN_LEFT,
                x: 52.0,
                y: 20.0
            })
            .messages,
            vec![50.0]
        );
        // Dragged on, past the end: the end.
        assert_eq!(
            tree.input(&Input::PointerMove { x: 300.0, y: 20.0 }).messages,
            vec![100.0]
        );
        tree.input(&Input::Release {
            button: BTN_LEFT,
            x: 300.0,
            y: 20.0,
        });
        // The wheel: up is more, a notch a step.
        tree.input(&Input::PointerMove { x: 50.0, y: 20.0 });
        let wheel = crate::input::Scroll {
            steps_y: -2.0,
            ..Default::default()
        };
        assert_eq!(tree.input(&Input::Scroll(wheel)).messages, vec![22.0]);
        // Keys on the focused slider move it, not the focus.
        tree.input(&key(Keysym::Tab, false));
        assert_eq!(tree.input(&key(Keysym::Right, false)).messages, vec![21.0]);
        assert_eq!(tree.input(&key(Keysym::End, false)).messages, vec![100.0]);
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
