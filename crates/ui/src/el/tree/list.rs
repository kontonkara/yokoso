//! Lists in the tree: the rows in view built before layout, scrolling by
//! wheel and by fingers with their fling after, and choosing by click and
//! keys.

use std::collections::HashMap;

use super::{Id, ROOT, State, Tree, mix};
use crate::{
    el::{BAR_LANE, El, Kind, ListEl, Size, thumb},
    geometry::Rect,
    input::{Keysym, Scroll, ScrollSource},
    motion::Part,
    theme::Theme,
};

/// A notch of the wheel scrolls this many rows.
const ROWS_A_NOTCH: f32 = 3.0;
/// A fling slows by this much a second, as `e^-k`.
const FLING_DECAY: f32 = 3.0;
/// Below this speed, pixels a second, a fling stops.
const FLING_STOP: f32 = 30.0;

/// Before layout: every list's children become the rows in view, from where
/// it is scrolled, in the rect it had last time.
pub(super) fn expand<M>(el: &mut El<M>, id: Id, rects: &HashMap<Id, Rect>, states: &mut HashMap<Id, State>) {
    if let Kind::List(list) = &mut el.kind {
        let rect = rects.get(&id).copied();
        let state = states.entry(id).or_default();
        let height = rect.map_or(600.0, |rect| rect.h);
        state.max_scroll = (list.len as f32 * list.row_height - height).max(0.0);
        let scroll = state.scroll.value.clamp(0.0, state.max_scroll);
        if scroll != state.scroll.value {
            state.scroll.snap(scroll);
            state.fling = 0.0;
        }
        let row_height = list.row_height.max(1.0);
        let first = (scroll / row_height).floor() as usize;
        let count = (height / row_height).ceil() as usize + 1;
        let last = (first + count).min(list.len);
        list.first = first;
        list.scroll = scroll;
        list.bar = state.max_scroll > 0.0;
        let row = list.row.clone();
        el.children = (first..last)
            .map(|at| {
                row(at)
                    .key(("row", at))
                    .width(Size::Fill(1.0))
                    .height(Size::Fixed(row_height))
            })
            .collect();
    }
    for (index, child) in el.children.iter_mut().enumerate() {
        let name = child.key.unwrap_or(index as u64 ^ 0x5bd1_e995);
        expand(child, mix(id, name), rects, states);
    }
}

/// The row at `y` in a list in `rect`, if there is one there.
pub(super) fn row_at<M>(list: &ListEl<M>, rect: Rect, y: f32) -> Option<usize> {
    if y < rect.y || y >= rect.bottom() {
        return None;
    }
    let row = ((y - rect.y + list.scroll) / list.row_height.max(1.0)).floor();
    (row >= 0.0 && (row as usize) < list.len).then_some(row as usize)
}

impl<M: Clone> Tree<M> {
    /// Each list's rows built anew for where it is scrolled now.
    pub(super) fn expand_lists(&mut self) {
        let rects: HashMap<Id, Rect> = self.nodes.iter().map(|node| (node.id, node.rect)).collect();
        if let Some(el) = &mut self.el {
            expand(el, ROOT, &rects, &mut self.states);
        }
    }

    /// The list `id`, and the rect it is in.
    pub(super) fn list(&self, id: Id) -> Option<(&ListEl<M>, Rect)> {
        let (el, rect) = self.element(id)?;
        match &el.kind {
            Kind::List(list) => Some((list, rect)),
            _ => None,
        }
    }

    /// The pointer moved over the list `id`: which row it is on. True when
    /// that changed.
    pub(super) fn list_hover(&mut self, id: Id, y: Option<f32>) -> bool {
        let row = match (self.list(id), y) {
            (Some((list, rect)), Some(y)) => row_at(list, rect, y),
            _ => None,
        };
        let Some(state) = self.states.get_mut(&id) else {
            return false;
        };
        let changed = state.hover_row != row;
        state.hover_row = row;
        changed
    }

    /// The list the wheel scrolls: the one under the pointer, or the nearest
    /// one the button or row under it is in.
    pub(super) fn list_under(&self) -> Option<Id> {
        let mut at = self.nodes.iter().position(|node| Some(node.id) == self.hovered)?;
        loop {
            let id = self.nodes[at].id;
            if self.list(id).is_some() {
                return Some(id);
            }
            at = self.nodes[at].parent?;
        }
    }

    /// A press on the list `id` at `x`, `y`: in its scrollbar's lane, the
    /// thumb taken where it is pressed, or brought there by its middle. True
    /// when it was in the lane.
    pub(super) fn bar_press(&mut self, id: Id, x: f32, y: f32) -> bool {
        let Some((list, rect)) = self.list(id) else {
            return false;
        };
        if !list.bar || x < rect.right() - BAR_LANE {
            return false;
        }
        let scroll = list.scroll;
        let Some(state) = self.states.get_mut(&id) else {
            return false;
        };
        let at = thumb(rect, scroll, state.max_scroll);
        let on = y >= at.y && y < at.bottom();
        state.grab = Some(if on { y - at.y } else { at.h / 2.0 });
        self.bar_drag(id, y);
        true
    }

    /// The pointer at `y` while the list `id`'s thumb is held: the list
    /// scrolled for the thumb to be there. True when it is held.
    pub(super) fn bar_drag(&mut self, id: Id, y: f32) -> bool {
        let Some((list, rect)) = self.list(id) else {
            return false;
        };
        let scroll = list.scroll;
        let Some(state) = self.states.get_mut(&id) else {
            return false;
        };
        let Some(grab) = state.grab else {
            return false;
        };
        let max = state.max_scroll;
        let room = rect.h - thumb(rect, scroll, max).h;
        let to = if room > 0.0 {
            ((y - grab - rect.y) / room).clamp(0.0, 1.0) * max
        } else {
            0.0
        };
        state.fling = 0.0;
        state.flinging = false;
        state.scroll.snap(to);
        true
    }

    /// The list `id`'s thumb let go. True when it was held.
    pub(super) fn bar_release(&mut self, id: Id) -> bool {
        self.states.get_mut(&id).and_then(|state| state.grab.take()).is_some()
    }

    /// A click on the list `id` at `y`: the row there is chosen.
    pub(super) fn list_click(&self, id: Id, y: f32) -> Option<M> {
        let (list, rect) = self.list(id)?;
        let row = row_at(list, rect, y)?;
        list.on_select.as_ref().map(|select| select(row))
    }

    /// Scrolling over the list `id`. True when it moved.
    pub(super) fn list_scroll(&mut self, id: Id, scroll: &Scroll) -> bool {
        let Some(row_height) = self.list(id).map(|(list, _)| list.row_height) else {
            return false;
        };
        let Some(state) = self.states.get_mut(&id) else {
            return false;
        };
        let max = state.max_scroll;
        let before = state.scroll.value;
        match scroll.source {
            ScrollSource::Wheel | ScrollSource::WheelTilt => {
                let target = (state.scroll.target + scroll.steps_y * ROWS_A_NOTCH * row_height).clamp(0.0, max);
                state.fling = 0.0;
                state.scroll.to(target);
                return target != before;
            }
            ScrollSource::Finger => {
                // Fingers move it pixel for pixel; the speed they had when
                // they lift carries on as a fling.
                let now = crate::app::monotonic();
                let dt = (now - state.touched).clamp(1.0 / 480.0, 0.1) as f32;
                state.touched = now;
                if scroll.stop {
                    state.flinging = state.fling.abs() > FLING_STOP;
                } else {
                    state.flinging = false;
                    let speed = scroll.y / dt;
                    state.fling = if dt >= 0.1 {
                        speed
                    } else {
                        state.fling * 0.4 + speed * 0.6
                    };
                    state.scroll.snap((before + scroll.y).clamp(0.0, max));
                }
            }
            ScrollSource::Continuous => {
                state.fling = 0.0;
                state.flinging = false;
                state.scroll.snap((before + scroll.y).clamp(0.0, max));
            }
        }
        state.scroll.value != before || state.flinging
    }

    /// A key on the focused list `id`: arrows choose the next row and the
    /// one before, pages a view's worth, Home and End the ends, Return uses
    /// the chosen one. The chosen row is scrolled into view.
    pub(super) fn list_key(&mut self, id: Id, keysym: Keysym) -> Option<M> {
        let (list, rect) = self.list(id)?;
        let page = ((rect.h / list.row_height.max(1.0)).floor() as usize).max(1);
        let last = list.len.checked_sub(1)?;
        let now = list.selected;
        let row = match keysym {
            Keysym::Down => now.map_or(0, |row| (row + 1).min(last)),
            Keysym::Up => now.map_or(0, |row| row.saturating_sub(1)),
            Keysym::Page_Down => now.map_or(0, |row| (row + page).min(last)),
            Keysym::Page_Up => now.map_or(0, |row| row.saturating_sub(page)),
            Keysym::Home => 0,
            Keysym::End => last,
            Keysym::Return | Keysym::KP_Enter => {
                return now.and_then(|row| list.on_activate.as_ref().map(|activate| activate(row)));
            }
            _ => return None,
        };
        let message = list.on_select.as_ref().map(|select| select(row));
        let (row_height, height) = (list.row_height, rect.h);
        if let Some(state) = self.states.get_mut(&id) {
            let (top, bottom) = (row as f32 * row_height, (row + 1) as f32 * row_height);
            let target = state.scroll.target;
            let target = if top < target {
                top
            } else if bottom > target + height {
                bottom - height
            } else {
                target
            };
            state.fling = 0.0;
            state.flinging = false;
            state.scroll.to(target.clamp(0.0, state.max_scroll));
        }
        message
    }

    /// Every list's scroll moved `dt` seconds on: its spring, and its fling.
    /// True while one still moves; marks the tree for layout when one did.
    pub(super) fn update_lists(&mut self, dt: f32, theme: &Theme) -> bool {
        // Scrolled, a list comes to where it was sent and no further: a
        // bounce there reads as the content wobbling.
        let mut feel = theme.motion.feel(Part::Lists);
        feel.params.damping = feel.params.damping.max(1.0);
        let mut moving = false;
        let mut moved = false;
        for state in self.states.values_mut() {
            let before = state.scroll.value;
            if state.flinging {
                let next = (before + state.fling * dt).clamp(0.0, state.max_scroll);
                state.fling *= (-FLING_DECAY * dt).exp();
                // At an end, or slow enough: it stops.
                if next == 0.0 || next == state.max_scroll || state.fling.abs() < FLING_STOP {
                    state.flinging = false;
                    state.fling = 0.0;
                }
                state.scroll.snap(next);
                moving |= state.flinging;
            }
            moving |= state.scroll.step(dt, &feel);
            moved |= state.scroll.value != before;
        }
        if moved {
            self.stale = true;
        }
        moving
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        el::{BAR_LANE, El, Size, tree::Tree},
        geometry::Rect,
        input::{Input, Key, KeyState, Keysym, Mods, Scroll, ScrollSource},
        theme::Theme,
    };

    #[derive(Clone, Debug, PartialEq)]
    enum Msg {
        Select(usize),
        Open(usize),
    }

    /// 10 000 rows of 20 in a list 200 high, row 3 chosen.
    fn tree(selected: Option<usize>) -> Tree<Msg> {
        let mut tree = Tree::default();
        tree.set(El::column(
            0.0,
            vec![
                El::list(10_000, 20.0, |at| El::text(format!("row {at}"), crate::el::Type::Body))
                    .choosing(selected, Msg::Select, Msg::Open)
                    .height(Size::Fixed(200.0)),
            ],
        ));
        lay(&mut tree);
        tree
    }

    fn lay(tree: &mut Tree<Msg>) {
        let space = Rect::new(0.0, 0.0, 300.0, 400.0);
        tree.layout_with(space, &mut |_: &El<Msg>, _| (0.0, 0.0));
        tree.expand_lists();
        tree.layout_with(space, &mut |_: &El<Msg>, _| (0.0, 0.0));
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

    #[test]
    fn only_the_rows_in_view_are_built() {
        let tree = tree(None);
        // The column, the list, and 11 rows: 200 / 20 and one more.
        assert_eq!(tree.nodes().len(), 2 + 11);
    }

    #[test]
    fn the_wheel_scrolls_and_the_rows_follow() {
        let mut tree = tree(None);
        tree.input(&Input::PointerMove { x: 10.0, y: 10.0 });
        let wheel = Scroll {
            steps_y: 2.0,
            ..Default::default()
        };
        assert!(tree.input(&Input::Scroll(wheel)).redraw);
        // Two notches of three rows: 120 pixels, once the spring settles.
        for _ in 0..240 {
            tree.update(1.0 / 240.0, 0.0, &Theme::default());
        }
        lay(&mut tree);
        let list = tree.nodes()[1].id;
        assert_eq!(tree.state(list).unwrap().scroll.value, 120.0);
        assert_eq!(tree.nodes()[2].rect.y, 0.0, "row 6 at the top");
    }

    #[test]
    fn the_wheel_on_a_button_in_a_row_scrolls_the_list() {
        let mut tree = Tree::default();
        tree.set(El::column(
            0.0,
            vec![
                El::list(100, 20.0, |at| {
                    El::press(
                        0.0,
                        vec![El::text(format!("row {at}"), crate::el::Type::Body)],
                        Some(Msg::Open(at)),
                    )
                })
                .height(Size::Fixed(200.0)),
            ],
        ));
        lay(&mut tree);
        tree.input(&Input::PointerMove { x: 10.0, y: 30.0 });
        let wheel = Scroll {
            steps_y: 1.0,
            ..Default::default()
        };
        assert!(
            tree.input(&Input::Scroll(wheel.clone())).redraw,
            "the row's button keeps it"
        );
        let list = tree.nodes()[1].id;
        assert_eq!(tree.state(list).unwrap().scroll.target, 60.0);
        // The rows built anew for where it went, the pointer still: it goes on.
        for _ in 0..240 {
            tree.update(1.0 / 240.0, 0.0, &Theme::default());
        }
        lay(&mut tree);
        assert!(
            tree.input(&Input::Scroll(wheel)).redraw,
            "the wheel lost under a new row"
        );
        assert_eq!(tree.state(list).unwrap().scroll.target, 120.0);
    }

    #[test]
    fn the_scrollbar_has_a_lane_of_its_own_and_its_thumb_drags_the_list() {
        let mut tree = Tree::default();
        tree.set(El::column(
            0.0,
            vec![
                El::list(100, 20.0, |at| {
                    El::press(
                        0.0,
                        vec![El::text(format!("row {at}"), crate::el::Type::Body)],
                        Some(Msg::Open(at)),
                    )
                })
                .height(Size::Fixed(200.0)),
            ],
        ));
        lay(&mut tree);
        assert_eq!(tree.nodes()[2].rect.w, 300.0 - BAR_LANE, "the rows clear of the lane");
        let list = tree.nodes()[1].id;
        // The thumb, 24 high at the top, taken 10 down and moved to 98: half
        // of the way it can go, half of the 1800 the list can scroll.
        tree.input(&Input::PointerMove { x: 295.0, y: 10.0 });
        tree.input(&Input::Press {
            button: 0x110,
            x: 295.0,
            y: 10.0,
        });
        tree.input(&Input::PointerMove { x: 250.0, y: 98.0 });
        assert_eq!(tree.state(list).unwrap().scroll.value, 900.0);
        let done = tree.input(&Input::Release {
            button: 0x110,
            x: 250.0,
            y: 98.0,
        });
        assert!(done.messages.is_empty(), "letting go is no click");
        // A press low in the lane brings the thumb there: the end.
        lay(&mut tree);
        tree.input(&Input::PointerMove { x: 295.0, y: 195.0 });
        tree.input(&Input::Press {
            button: 0x110,
            x: 295.0,
            y: 195.0,
        });
        assert_eq!(tree.state(list).unwrap().scroll.value, 1800.0);
    }

    #[test]
    fn the_wheel_never_scrolls_past_where_it_sends_the_list() {
        let mut tree = tree(None);
        let mut theme = Theme::default();
        theme.motion.global = crate::motion::Preset::Bouncy.params();
        tree.input(&Input::PointerMove { x: 10.0, y: 10.0 });
        let wheel = Scroll {
            steps_y: 2.0,
            ..Default::default()
        };
        tree.input(&Input::Scroll(wheel));
        let list = tree.nodes()[1].id;
        let mut furthest = 0.0f32;
        for _ in 0..240 {
            tree.update(1.0 / 240.0, 0.0, &theme);
            furthest = furthest.max(tree.state(list).unwrap().scroll.value);
        }
        assert!(furthest <= 120.0 + 0.01, "it went on to {furthest}");
    }

    #[test]
    fn arrows_choose_and_bring_the_row_into_view() {
        let mut tree = tree(Some(9));
        tree.input(&key(Keysym::Tab));
        assert_eq!(tree.input(&key(Keysym::Down)).messages, vec![Msg::Select(10)]);
        for _ in 0..240 {
            tree.update(1.0 / 240.0, 0.0, &Theme::default());
        }
        let list = tree.nodes()[1].id;
        // Row 10 ends at 220: scrolled so it shows at the bottom.
        assert_eq!(tree.state(list).unwrap().scroll.value, 20.0);
        assert_eq!(tree.input(&key(Keysym::Return)).messages, vec![Msg::Open(9)]);
        assert_eq!(tree.input(&key(Keysym::End)).messages, vec![Msg::Select(9_999)]);
    }

    #[test]
    fn a_click_chooses_the_row_under_it() {
        let mut tree = tree(None);
        tree.input(&Input::PointerMove { x: 10.0, y: 45.0 });
        tree.input(&Input::Press {
            button: 0x110,
            x: 10.0,
            y: 45.0,
        });
        let done = tree.input(&Input::Release {
            button: 0x110,
            x: 10.0,
            y: 45.0,
        });
        assert_eq!(done.messages, vec![Msg::Select(2)]);
    }

    #[test]
    fn a_fling_slows_and_stops() {
        let mut tree = tree(None);
        tree.input(&Input::PointerMove { x: 10.0, y: 10.0 });
        let fingers = |y: f32, stop: bool| {
            Input::Scroll(Scroll {
                y,
                source: ScrollSource::Finger,
                stop,
                ..Default::default()
            })
        };
        let list = tree.nodes()[1].id;
        tree.input(&fingers(30.0, false));
        // A fast swipe: the speed as the fingers left.
        tree.states.get_mut(&list).unwrap().fling = 2000.0;
        tree.input(&fingers(0.0, true));
        let mut frames = 0;
        while tree.update(1.0 / 240.0, 0.0, &Theme::default()) {
            frames += 1;
            assert!(frames < 240 * 5, "never stops");
        }
        let scrolled = tree.state(list).unwrap().scroll.value;
        assert!(scrolled > 400.0, "it flew on: {scrolled}");
    }
}
