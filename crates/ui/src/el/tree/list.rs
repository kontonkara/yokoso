//! Lists in the tree: the rows in view built before layout, scrolling by
//! wheel and by fingers with their fling after, and choosing by click and
//! keys.

use std::collections::HashMap;

use super::{Id, ROOT, RowMotion, State, Tree, mix};
use crate::{
    el::{BAR_LANE, El, Kind, ListEl, Size, thumb},
    geometry::Rect,
    input::{Keysym, Scroll, ScrollSource},
    motion::{Part, Spring},
    theme::Theme,
};

/// A notch of the wheel scrolls this many rows.
const ROWS_A_NOTCH: f32 = 3.0;
/// A fling slows by this much a second, as `e^-k`.
const FLING_DECAY: f32 = 3.0;
/// Below this speed, pixels a second, a fling stops.
const FLING_STOP: f32 = 30.0;
/// Scrolled, a list's scrollbar stays plainer this long, seconds.
const BAR_HOLD: f64 = 0.9;

/// What expanding the lists takes: where each element was last laid out,
/// what each keeps, the rows each list had built before the description
/// was new, and whether it is new.
pub(super) struct Expand<'a, M> {
    pub rects: &'a HashMap<Id, Rect>,
    pub states: &'a mut HashMap<Id, State>,
    pub spare: &'a mut HashMap<Id, Vec<(u64, El<M>)>>,
    pub described: bool,
}

/// Before layout: every list's children become the rows in view, from where
/// it is scrolled, in the rect it had last time; a keyed list's rows on
/// their way where they show now, and those gone that still fade.
pub(super) fn expand<M>(el: &mut El<M>, id: Id, cx: &mut Expand<'_, M>) {
    if let Kind::List(list) = &mut el.kind {
        // The rows it had: as the description before had them, when it is
        // new; else as they were built last time.
        let before: Vec<(u64, El<M>)> = if cx.described {
            cx.spare.remove(&id).unwrap_or_default()
        } else {
            let keys = std::mem::take(&mut list.shown_keys);
            keys.into_iter().zip(std::mem::take(&mut el.children)).collect()
        };
        let rect = cx.rects.get(&id).copied();
        let height = rect.map_or(600.0, |rect| rect.h);
        let row_height = list.row_height.max(1.0);
        let state = cx.states.entry(id).or_default();
        state.max_scroll = (list.len as f32 * list.row_height - height).max(0.0);
        let keys = list.keys.clone();
        if let Some(keys) = &keys
            && cx.described
        {
            reorder(state, keys.as_ref(), list.len, row_height, &before);
        }
        // Past its end as its rows close up, a keyed list goes back on its
        // spring with them; else it is put back at once.
        let max = state.max_scroll;
        let gliding = keys.is_some() && (!state.rows.is_empty() || state.scroll.is_moving());
        if gliding && !state.flinging && state.grab.is_none() {
            if state.scroll.target > max {
                state.scroll.to(max);
            }
        } else if state.scroll.value != state.scroll.value.clamp(0.0, max) {
            state.scroll.snap(state.scroll.value.clamp(0.0, max));
            state.fling = 0.0;
        }
        let scroll = state.scroll.value.max(0.0);
        let first = (scroll / row_height).floor() as usize;
        let count = (height / row_height).ceil() as usize + 1;
        let last = (first + count).min(list.len);
        let in_view = |y: f32| y + row_height > scroll && y < scroll + height;
        let mut shown: Vec<usize> = (first..last).collect();
        // Rows on their way that show now, though their place is out of view.
        shown.extend(
            state
                .rows
                .values()
                .filter(|row| {
                    !row.ghost && row.at < list.len && !(first..last).contains(&row.at) && in_view(row.y.value)
                })
                .map(|row| row.at),
        );
        shown.sort_unstable();
        shown.dedup();
        list.first = first;
        list.scroll = scroll;
        list.bar = max > 0.0;
        let mut children = Vec::new();
        let mut places = Vec::new();
        let mut shown_keys = Vec::new();
        // Each row's place, how much of it shows, and whether it is gone.
        let mut looks = Vec::new();
        // Rows gone, first, under the rest, as long as they fade.
        for (key, row) in before {
            if let Some(motion) = state.rows.get(&key).filter(|motion| motion.ghost) {
                places.push(motion.y.value);
                looks.push((motion.shown.value, true));
                shown_keys.push(key);
                children.push(row);
            }
        }
        list.ghosts = children.len();
        let build = list.row.clone();
        for at in shown {
            let key = keys.as_ref().map_or(at as u64, |keys| keys(at));
            let motion = state.rows.get(&key).filter(|motion| !motion.ghost);
            places.push(motion.map_or(at as f32 * row_height, |motion| motion.y.value));
            looks.push((motion.map_or(1.0, |motion| motion.shown.value), false));
            shown_keys.push(key);
            let row = build(at);
            let row = if keys.is_some() {
                row.key(("row", key))
            } else {
                row.key(("row", at))
            };
            children.push(row.width(Size::Fill(1.0)).height(Size::Fixed(row_height)));
        }
        list.places = places;
        list.shown_keys = shown_keys;
        el.children = children;
        for (child, (fade, ghost)) in el.children.iter().zip(looks) {
            let row = cx.states.entry(mix(id, child.key.unwrap_or_default())).or_default();
            row.fade = fade;
            row.ghost = ghost;
        }
    }
    for (index, child) in el.children.iter_mut().enumerate() {
        let name = child.key.unwrap_or(index as u64 ^ 0x5bd1_e995);
        expand(child, mix(id, name), cx);
    }
}

/// A keyed list described anew with `len` rows: what moved glides to its
/// place, what is new comes out from under the row above it that was
/// there, and what is gone, where it showed among `before`, fades away
/// under the row above it that stays.
fn reorder<M>(state: &mut State, keys: &dyn Fn(usize) -> u64, len: usize, row_height: f32, before: &[(u64, El<M>)]) {
    let new: Vec<u64> = (0..len).map(keys).collect();
    if !state.keyed {
        // Described the first time: all is where it is.
        state.keyed = true;
        state.keys = new;
        return;
    }
    if new == state.keys {
        return;
    }
    let old = std::mem::replace(&mut state.keys, new.clone());
    let old_at: HashMap<u64, usize> = old.iter().enumerate().map(|(at, key)| (*key, at)).collect();
    let new_at: HashMap<u64, usize> = new.iter().enumerate().map(|(at, key)| (*key, at)).collect();
    let rows = &mut state.rows;
    // Where a row that was there shows now.
    let now = |rows: &HashMap<u64, RowMotion>, key: &u64| {
        rows.get(key)
            .map(|row| row.y.value)
            .or_else(|| old_at.get(key).map(|at| *at as f32 * row_height))
    };
    for (at, key) in old.iter().enumerate() {
        if new_at.contains_key(key) {
            continue;
        }
        if !before.iter().any(|(had, _)| had == key) {
            rows.remove(key);
            continue;
        }
        let from = now(rows, key).unwrap_or(at as f32 * row_height);
        let to = old[..at]
            .iter()
            .rev()
            .find_map(|above| new_at.get(above))
            .map_or(from, |above| *above as f32 * row_height);
        let row = rows.entry(*key).or_insert(RowMotion {
            y: Spring::pixels(from),
            shown: Spring::new(1.0),
            at,
            ghost: true,
        });
        row.ghost = true;
        row.y.to(to);
        row.shown.to(0.0);
    }
    for (at, key) in new.iter().enumerate() {
        let to = at as f32 * row_height;
        if let Some(row) = rows.get_mut(key) {
            row.ghost = false;
            row.at = at;
            row.y.to(to);
            row.shown.to(1.0);
        } else if let Some(was) = old_at.get(key) {
            if *was != at {
                let mut y = Spring::pixels(*was as f32 * row_height);
                y.to(to);
                rows.insert(
                    *key,
                    RowMotion {
                        y,
                        shown: Spring::new(1.0),
                        at,
                        ghost: false,
                    },
                );
            }
        } else {
            let from = new[..at]
                .iter()
                .rev()
                .find_map(|above| old_at.contains_key(above).then(|| now(rows, above)).flatten())
                .unwrap_or(to);
            let mut y = Spring::pixels(from);
            y.to(to);
            let mut shown = Spring::new(0.0);
            shown.to(1.0);
            rows.insert(
                *key,
                RowMotion {
                    y,
                    shown,
                    at,
                    ghost: false,
                },
            );
        }
    }
}

/// How far a list in `rect`, scrolled `scroll` of `max`, scrolls for its
/// thumb, held `grab` down from its top, to be at `y`.
fn bar_scroll(rect: Rect, scroll: f32, max: f32, grab: f32, y: f32) -> f32 {
    let room = rect.h - thumb(rect, scroll, max).h;
    if room > 0.0 {
        ((y - grab - rect.y) / room).clamp(0.0, 1.0) * max
    } else {
        0.0
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
            let mut cx = Expand {
                rects: &rects,
                states: &mut self.states,
                spare: &mut self.spare,
                described: self.described,
            };
            expand(el, ROOT, &mut cx);
        }
        self.described = false;
        self.spare.clear();
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
    /// thumb taken where it is pressed; or brought there by its middle, the
    /// list gliding to it. True when it was in the lane.
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
        let grab = if on { y - at.y } else { at.h / 2.0 };
        state.grab = Some(grab);
        state.fling = 0.0;
        state.flinging = false;
        state.bar_at = crate::app::monotonic();
        if !on {
            state.scroll.to(bar_scroll(rect, scroll, state.max_scroll, grab, y));
        }
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
        state.fling = 0.0;
        state.flinging = false;
        state.bar_at = crate::app::monotonic();
        state.scroll.snap(bar_scroll(rect, scroll, state.max_scroll, grab, y));
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
        state.bar_at = crate::app::monotonic();
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

    /// Every list's scroll moved `dt` seconds on: its spring, and its fling;
    /// its rows on their way, and its scrollbar. True while one still moves;
    /// marks the tree for layout when one did.
    pub(super) fn update_lists(&mut self, dt: f32, now: f64, theme: &Theme) -> bool {
        // Scrolled, a list comes to where it was sent and no further: a
        // bounce there reads as the content wobbling. Its rows glide the
        // same way, and fade on the small things' spring.
        let mut feel = theme.motion.feel(Part::Lists);
        feel.params.damping = feel.params.damping.max(1.0);
        let mut fade = theme.motion.feel(Part::Micro);
        fade.params.damping = fade.params.damping.max(1.0);
        let mut moving = self.bar_aim(now);
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
            if state.scroll.value != before {
                moved = true;
                state.bar_at = now;
            }
            if !state.rows.is_empty() {
                for row in state.rows.values_mut() {
                    row.y.step(dt, &feel);
                    row.shown.step(dt, &fade);
                }
                // At rest, a row is where it is; gone, it is gone.
                state.rows.retain(|_, row| row.y.is_moving() || row.shown.is_moving());
                moving |= !state.rows.is_empty();
                moved = true;
            }
            moving |= state.bar_shown.step(dt, &fade);
            moving |= state.bar_wide.step(dt, &fade);
        }
        if moved {
            self.stale = true;
        }
        moving
    }

    /// Each list's scrollbar aimed: widest and plainest held or with the
    /// pointer in its lane, plainer on a list pointed at or just scrolled,
    /// faint else. True when one is to move.
    pub(super) fn bar_aim(&mut self, now: f64) -> bool {
        let mut moving = false;
        let mut soonest: Option<f64> = None;
        for node in self.nodes.iter().filter(|node| node.role == super::Role::List) {
            let Some(state) = self.states.get_mut(&node.id) else {
                continue;
            };
            let over = self.pointer.filter(|(x, y)| node.rect.contains(*x, *y));
            let in_lane = over.is_some_and(|(x, _)| x >= node.rect.right() - BAR_LANE);
            let wide = state.grab.is_some() || in_lane;
            let lately = now - state.bar_at < BAR_HOLD;
            if lately {
                let due = state.bar_at + BAR_HOLD - now;
                soonest = Some(soonest.map_or(due, |soonest| soonest.min(due)));
            }
            let shown = if wide {
                0.9
            } else if over.is_some() || lately {
                0.6
            } else {
                super::BAR_REST
            };
            let wide = if wide { 1.0 } else { 0.0 };
            if state.bar_shown.target != shown || state.bar_wide.target != wide {
                state.bar_shown.to(shown);
                state.bar_wide.to(wide);
            }
            moving |= state.bar_shown.is_moving() || state.bar_wide.is_moving();
        }
        // Just scrolled, a bar fades once it has rested a while: a frame then.
        self.bar_wake = soonest.map(|due| std::time::Duration::from_secs_f64(due.max(0.0) + 0.01));
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
        // A press low in the lane brings the thumb there, the end, the list
        // gliding there rather than jumping.
        lay(&mut tree);
        tree.input(&Input::PointerMove { x: 295.0, y: 195.0 });
        tree.input(&Input::Press {
            button: 0x110,
            x: 295.0,
            y: 195.0,
        });
        let state = tree.state(list).unwrap();
        assert_eq!((state.scroll.value, state.scroll.target), (900.0, 1800.0));
        tree.update(1.0 / 60.0, 0.0, &Theme::default());
        let partway = tree.state(list).unwrap().scroll.value;
        assert!(partway > 900.0 && partway < 1800.0, "on its way: {partway}");
        while tree.update(1.0 / 60.0, 0.0, &Theme::default()) {}
        assert_eq!(tree.state(list).unwrap().scroll.value, 1800.0);
    }

    #[test]
    fn the_scrollbar_is_faint_at_rest_plainer_pointed_at_and_wide_in_its_lane() {
        let mut tree = tree(None);
        let list = tree.nodes()[1].id;
        let settle = |tree: &mut Tree<Msg>| while tree.update(1.0 / 60.0, f64::MAX, &Theme::default()) {};
        let bar = |tree: &Tree<Msg>| {
            let state = tree.state(list).unwrap();
            (state.bar_shown.value, state.bar_wide.value)
        };
        settle(&mut tree);
        assert_eq!(bar(&tree), (super::super::BAR_REST, 0.0));
        tree.input(&Input::PointerMove { x: 100.0, y: 50.0 });
        settle(&mut tree);
        assert_eq!(bar(&tree), (0.6, 0.0));
        tree.input(&Input::PointerMove { x: 295.0, y: 50.0 });
        settle(&mut tree);
        assert_eq!(bar(&tree), (0.9, 1.0));
        tree.input(&Input::PointerLeave);
        settle(&mut tree);
        assert_eq!(bar(&tree), (super::super::BAR_REST, 0.0));
    }

    /// Rows of a keyed list, by name, in a list 200 high: each a button.
    fn named(names: &[&'static str]) -> El<Msg> {
        let names = names.to_vec();
        let keys = names.clone();
        El::column(
            0.0,
            vec![
                El::list(names.len(), 20.0, move |at| {
                    El::press(
                        0.0,
                        vec![El::text(names[at], crate::el::Type::Body)],
                        Some(Msg::Open(at)),
                    )
                })
                .keyed(move |at| {
                    use std::hash::{Hash, Hasher};
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    keys[at].hash(&mut hasher);
                    hasher.finish()
                })
                .height(Size::Fixed(200.0)),
            ],
        )
    }

    /// Each row shown now: its name, where it is, how much of it shows, and
    /// whether it takes the pointer.
    fn rows(tree: &Tree<Msg>) -> Vec<(String, f32, f32, bool)> {
        let el = tree.el.as_ref().unwrap();
        let list = &el.children[0];
        let base = tree.nodes()[1].id;
        list.children
            .iter()
            .map(|row| {
                let name = match &row.children[0].kind {
                    crate::el::Kind::Text(text) => text.text.clone(),
                    _ => String::new(),
                };
                let id = super::mix(base, row.key.unwrap());
                let node = tree.nodes().iter().find(|node| node.id == id).unwrap();
                (name, node.rect.y, tree.state(id).unwrap().fade, node.interactive)
            })
            .collect()
    }

    #[test]
    fn a_row_gone_fades_away_under_the_one_above_and_those_below_close_up() {
        let mut tree = Tree::default();
        tree.set(named(&["a", "b", "c", "d"]));
        lay(&mut tree);
        tree.set(named(&["a", "c", "d"]));
        lay(&mut tree);
        // At first all is where it was: b still there, fading, untouchable.
        let first = rows(&tree);
        assert_eq!(first[0].0, "b");
        assert_eq!((first[0].1, first[0].2, first[0].3), (20.0, 1.0, false));
        let at: Vec<(&str, f32)> = first[1..].iter().map(|row| (row.0.as_str(), row.1)).collect();
        assert_eq!(at, vec![("a", 0.0), ("c", 40.0), ("d", 60.0)]);
        // On its way: b fades going under a; c and d come up.
        tree.update(1.0 / 30.0, 0.0, &Theme::default());
        lay(&mut tree);
        let partway = rows(&tree);
        let b = &partway[0];
        assert!(b.1 < 20.0 && b.2 < 1.0, "b goes under a, fading: {b:?}");
        let c = &partway[2];
        assert!(c.1 > 20.0 && c.1 < 40.0, "c comes up: {c:?}");
        while tree.update(1.0 / 60.0, 0.0, &Theme::default()) {}
        lay(&mut tree);
        let done: Vec<(String, f32, f32, bool)> = rows(&tree);
        assert_eq!(
            done,
            vec![
                ("a".into(), 0.0, 1.0, true),
                ("c".into(), 20.0, 1.0, true),
                ("d".into(), 40.0, 1.0, true),
            ]
        );
    }

    #[test]
    fn new_rows_come_out_from_under_the_row_above() {
        let mut tree = Tree::default();
        tree.set(named(&["head", "next"]));
        lay(&mut tree);
        tree.set(named(&["head", "one", "two", "next"]));
        lay(&mut tree);
        // They start under the head, unseen, and next where it was.
        let first: Vec<(String, f32, f32)> = rows(&tree).into_iter().map(|row| (row.0, row.1, row.2)).collect();
        assert_eq!(
            first,
            vec![
                ("head".into(), 0.0, 1.0),
                ("one".into(), 0.0, 0.0),
                ("two".into(), 0.0, 0.0),
                ("next".into(), 20.0, 1.0),
            ]
        );
        while tree.update(1.0 / 60.0, 0.0, &Theme::default()) {}
        lay(&mut tree);
        let done: Vec<(String, f32, f32)> = rows(&tree).into_iter().map(|row| (row.0, row.1, row.2)).collect();
        assert_eq!(
            done,
            vec![
                ("head".into(), 0.0, 1.0),
                ("one".into(), 20.0, 1.0),
                ("two".into(), 40.0, 1.0),
                ("next".into(), 60.0, 1.0),
            ]
        );
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
