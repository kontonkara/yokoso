//! Layout: rows, columns and stacks, with padding, gaps and alignment. Each
//! side of an element hugs its content, takes a share of what is left, or is
//! fixed. The result is a rect for every element, in the tree's pre-order:
//! an element first, then its children, in turn.

use super::{Align, El, Kind, Size};
use crate::geometry::Rect;

/// What a leaf needs: its size for a width it may not pass, where there is
/// one. Text wraps to it; anything else may ignore it.
pub trait Measure<M> {
    fn measure(&mut self, el: &El<M>, max_width: Option<f32>) -> (f32, f32);
}

impl<M, F: FnMut(&El<M>, Option<f32>) -> (f32, f32)> Measure<M> for F {
    fn measure(&mut self, el: &El<M>, max_width: Option<f32>) -> (f32, f32) {
        self(el, max_width)
    }
}

/// Lays `el` out in `space`: a rect for it and each of its descendants, in
/// pre-order.
pub fn layout<M>(el: &El<M>, space: Rect, measure: &mut impl Measure<M>) -> Vec<Rect> {
    let mut out = Vec::new();
    place(el, space, measure, &mut out);
    out
}

/// How large `el` wants to be, given no more than `max_width` across.
pub fn natural<M>(el: &El<M>, max_width: Option<f32>, measure: &mut impl Measure<M>) -> (f32, f32) {
    let pad = el.padding;
    let fixed_w = match el.width {
        Size::Fixed(w) => Some(w),
        _ => None,
    };
    let inner_max = fixed_w.or(max_width).map(|w| (w - pad.horizontal()).max(0.0));
    let (w, h) = match &el.kind {
        Kind::Row { gap, .. } | Kind::Press(super::PressEl { gap, .. }) => {
            let mut w = 0.0f32;
            for child in &el.children {
                w += natural(child, None, measure).0;
            }
            let w = w + gaps(*gap, el.children.len());
            // Its height with the widths its children will get: text in a
            // narrow one wraps, and the row grows to hold it.
            let widths = shares(el, *gap, inner_max.unwrap_or(w), measure);
            let h = el
                .children
                .iter()
                .zip(widths)
                .map(|(child, width)| natural(child, Some(width), measure).1)
                .fold(0.0f32, f32::max);
            (w, h)
        }
        Kind::Column { gap, .. } | Kind::Card { gap, .. } => {
            let mut w = 0.0f32;
            let mut h = 0.0f32;
            for child in &el.children {
                let (cw, ch) = natural(child, inner_max, measure);
                w = w.max(cw);
                h += ch;
            }
            (w, h + gaps(*gap, el.children.len()))
        }
        Kind::Stack | Kind::Tooltip { .. } => el
            .children
            .iter()
            .map(|child| natural(child, inner_max, measure))
            .fold((0.0f32, 0.0f32), |(w, h), (cw, ch)| (w.max(cw), h.max(ch))),
        Kind::Spacer | Kind::Block { .. } | Kind::Battery { .. } | Kind::_Message(_) => (0.0, 0.0),
        Kind::List(list) => (0.0, list.len as f32 * list.row_height),
        _ => measure.measure(el, inner_max),
    };
    let w = fixed_w.unwrap_or(w + pad.horizontal());
    let h = match el.height {
        Size::Fixed(h) => h,
        _ => h + pad.vertical(),
    };
    (w, h)
}

fn gaps(gap: f32, children: usize) -> f32 {
    gap * children.saturating_sub(1) as f32
}

fn place<M>(el: &El<M>, rect: Rect, measure: &mut impl Measure<M>, out: &mut Vec<Rect>) {
    out.push(rect);
    let pad = el.padding;
    let inner = Rect::new(
        rect.x + pad.left,
        rect.y + pad.top,
        (rect.w - pad.horizontal()).max(0.0),
        (rect.h - pad.vertical()).max(0.0),
    );
    match &el.kind {
        Kind::Row { gap, align } => line(el, inner, *gap, *align, true, measure, out),
        Kind::Press(press) => line(el, inner, press.gap, Align::Center, true, measure, out),
        Kind::Column { gap, align } => line(el, inner, *gap, *align, false, measure, out),
        Kind::Card { gap, .. } => line(el, inner, *gap, Align::Stretch, false, measure, out),
        Kind::List(list) => {
            // Its children are the rows in view, from the first: each at its
            // place down the whole list, less how far it is scrolled.
            for (at, child) in el.children.iter().enumerate() {
                let y = inner.y + (list.first + at) as f32 * list.row_height - list.scroll;
                place(child, Rect::new(inner.x, y, inner.w, list.row_height), measure, out);
            }
        }
        Kind::Stack | Kind::Tooltip { .. } => {
            for child in &el.children {
                let (w, h) = natural(child, Some(inner.w), measure);
                let w = along(child.width, w, inner.w);
                let h = along(child.height, h, inner.h);
                place(child, Rect::new(inner.x, inner.y, w, h), measure, out);
            }
        }
        _ => {}
    }
}

/// A child's size along one axis: fixed, all there is for a fill, or its own.
fn along(size: Size, natural: f32, room: f32) -> f32 {
    match size {
        Size::Fixed(fixed) => fixed,
        Size::Fill(_) => room,
        Size::Hug => natural.min(room),
    }
}

/// A row's children's widths in `room`: fixed and hugging ones first, then
/// what is left shared among the filling ones by weight.
fn shares<M>(el: &El<M>, gap: f32, room: f32, measure: &mut impl Measure<M>) -> Vec<f32> {
    let mut sizes = Vec::with_capacity(el.children.len());
    let mut taken = gaps(gap, el.children.len());
    let mut weights = 0.0;
    for child in &el.children {
        let size = match child.width {
            Size::Fixed(fixed) => fixed,
            Size::Fill(weight) => {
                weights += weight.max(0.0);
                0.0
            }
            Size::Hug => natural(child, None, measure).0,
        };
        taken += size;
        sizes.push(size);
    }
    fill(el, &mut sizes, (room - taken).max(0.0), weights, |child| child.width);
    sizes
}

/// A column's children's heights in `room`, each hugging one at the width
/// it will have.
fn shares_down<M>(el: &El<M>, gap: f32, inner: Rect, room: f32, measure: &mut impl Measure<M>) -> Vec<f32> {
    let mut sizes = Vec::with_capacity(el.children.len());
    let mut taken = gaps(gap, el.children.len());
    let mut weights = 0.0;
    for child in &el.children {
        let size = match child.height {
            Size::Fixed(fixed) => fixed,
            Size::Fill(weight) => {
                weights += weight.max(0.0);
                0.0
            }
            Size::Hug => natural(child, Some(inner.w), measure).1,
        };
        taken += size;
        sizes.push(size);
    }
    fill(el, &mut sizes, (room - taken).max(0.0), weights, |child| child.height);
    sizes
}

/// Shares `left` among the filling children, by weight.
fn fill<M>(el: &El<M>, sizes: &mut [f32], left: f32, weights: f32, size_of: impl Fn(&El<M>) -> Size) {
    for (child, size) in el.children.iter().zip(sizes.iter_mut()) {
        if let Size::Fill(weight) = size_of(child) {
            *size = if weights > 0.0 {
                left * weight.max(0.0) / weights
            } else {
                0.0
            };
        }
    }
}

/// Lays out a row (`across` true) or a column.
fn line<M>(
    el: &El<M>,
    inner: Rect,
    gap: f32,
    align: Align,
    across: bool,
    measure: &mut impl Measure<M>,
    out: &mut Vec<Rect>,
) {
    let main_room = if across { inner.w } else { inner.h };
    let cross_room = if across { inner.h } else { inner.w };
    let cross_of = |child: &El<M>| if across { child.height } else { child.width };
    let sizes = if across {
        shares(el, gap, main_room, measure)
    } else {
        shares_down(el, gap, inner, main_room, measure)
    };
    let mut at = if across { inner.x } else { inner.y };
    for (child, size) in el.children.iter().zip(sizes) {
        let cross = match cross_of(child) {
            Size::Fixed(fixed) => fixed,
            Size::Fill(_) => cross_room,
            Size::Hug if align == Align::Stretch => cross_room,
            Size::Hug => {
                // Text wraps to the width it was given.
                let (w, h) = natural(child, Some(if across { size } else { cross_room }), measure);
                (if across { h } else { w }).min(cross_room)
            }
        };
        let offset = match align {
            Align::Start | Align::Stretch => 0.0,
            Align::Center => (cross_room - cross) / 2.0,
            Align::End => cross_room - cross,
        };
        let rect = if across {
            Rect::new(at, inner.y + offset, size, cross)
        } else {
            Rect::new(inner.x + offset, at, cross, size)
        };
        place(child, rect, measure, out);
        at += size + gap;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::el::{Edges, Type};

    type E = El<()>;

    /// Text as seven pixels a character, 17 a line, wrapping where it must.
    fn measure(el: &E, max_width: Option<f32>) -> (f32, f32) {
        match &el.kind {
            Kind::Text(text) => {
                let w = text.text.chars().count() as f32 * 7.0;
                match max_width {
                    Some(max) if w > max && max > 0.0 => (max, (w / max).ceil() * 17.0),
                    _ => (w, 17.0),
                }
            }
            _ => (0.0, 0.0),
        }
    }

    fn boxed(w: f32, h: f32) -> E {
        El::spacer().width(Size::Fixed(w)).height(Size::Fixed(h))
    }

    #[test]
    fn a_row_shares_what_is_left_by_weight() {
        let row = E::row(
            10.0,
            vec![
                boxed(50.0, 20.0),
                El::spacer().width(Size::Fill(1.0)),
                El::spacer().width(Size::Fill(3.0)),
            ],
        );
        let rects = layout(&row, Rect::new(0.0, 0.0, 300.0, 40.0), &mut measure);
        // 300 - 50 - 2 gaps of 10 = 230, split 1:3.
        assert_eq!(rects[1], Rect::new(0.0, 10.0, 50.0, 20.0));
        assert_eq!(rects[2].x, 60.0);
        assert_eq!(rects[2].w, 57.5);
        assert_eq!(rects[3].x, 127.5);
        assert_eq!(rects[3].w, 172.5);
    }

    #[test]
    fn a_column_stretches_and_pads() {
        let column = E::column(8.0, vec![El::text("hello", Type::Body), boxed(40.0, 30.0)]).padding(Edges::all(16.0));
        let rects = layout(&column, Rect::new(0.0, 0.0, 200.0, 300.0), &mut measure);
        assert_eq!(rects[1], Rect::new(16.0, 16.0, 168.0, 17.0));
        // Fixed wins over stretch.
        assert_eq!(rects[2], Rect::new(16.0, 41.0, 40.0, 30.0));
    }

    #[test]
    fn hugging_a_row_measures_its_children() {
        let row =
            E::row(4.0, vec![El::text("ab", Type::Body), El::text("cde", Type::Body)]).padding(Edges::xy(6.0, 2.0));
        assert_eq!(natural(&row, None, &mut measure), (14.0 + 4.0 + 21.0 + 12.0, 21.0));
    }

    #[test]
    fn text_in_a_narrow_column_wraps_and_grows_taller() {
        let column = E::column(0.0, vec![El::text("a".repeat(20), Type::Body)]);
        let rects = layout(&column, Rect::new(0.0, 0.0, 70.0, 500.0), &mut measure);
        assert_eq!(rects[1].h, 34.0);
    }

    #[test]
    fn a_row_is_as_tall_as_its_text_wraps_in_the_width_it_shares() {
        // Two halves of 70: 20 characters, 140 pixels, are 4 lines in 35.
        let half = || El::text("a".repeat(20), Type::Body).width(Size::Fill(1.0));
        let column = E::column(0.0, vec![E::row(0.0, vec![half(), half()])]);
        let rects = layout(&column, Rect::new(0.0, 0.0, 70.0, 500.0), &mut measure);
        assert_eq!(rects[1].h, 4.0 * 17.0);
    }

    #[test]
    fn centered_children_sit_in_the_middle_across() {
        let row = E::row(0.0, vec![boxed(10.0, 10.0)]).align(Align::Center);
        let rects = layout(&row, Rect::new(0.0, 0.0, 100.0, 50.0), &mut measure);
        assert_eq!(rects[1].y, 20.0);
        let row = E::row(0.0, vec![boxed(10.0, 10.0)]).align(Align::End);
        assert_eq!(layout(&row, Rect::new(0.0, 0.0, 100.0, 50.0), &mut measure)[1].y, 40.0);
    }

    #[test]
    fn nothing_goes_negative_when_room_runs_out() {
        let row = E::row(10.0, vec![boxed(80.0, 10.0), El::spacer().width(Size::Fill(1.0))]);
        let rects = layout(&row, Rect::new(0.0, 0.0, 50.0, 10.0), &mut measure);
        assert_eq!(rects[2].w, 0.0);
    }
}
