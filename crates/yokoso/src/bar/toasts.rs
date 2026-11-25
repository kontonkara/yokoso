//! Toasts: notifications as they come, in a column at the top right, the
//! newest on top. Each comes in from the right, the others make room for it
//! on a spring; it goes back out to the right, fading, and the others close
//! the gap. Each has its content laid out on its own, as tall as it needs.

use yokoso_ui::{
    Rect,
    app::{Frame, View},
    el::{Align, ButtonStyle, Edges, El, Ink, Size, Type, tree::Tree},
    motion::{Part, Spring},
    text::Text,
    theme::Theme,
};

use super::Msg;
use crate::notify::{Notification, Urgency};

/// A toast's width; the room around for its shadow; between two.
pub const WIDTH: f32 = 360.0;
pub const PAD: f32 = 10.0;
const GAP: f32 = 8.0;
/// The strip they show in: as wide as a toast and its shadow.
pub const STRIP: f32 = WIDTH + 2.0 * PAD;

pub struct Toast {
    pub id: u32,
    pub tree: Tree<Msg>,
    /// Its top, and how far in it has come: 0 out at the right, 1 in place.
    y: Spring,
    slide: Spring,
    pub height: f32,
    pub leaving: bool,
    pub critical: bool,
    /// Its content to be built anew: new, or replaced.
    pub stale: bool,
    placed: bool,
}

impl Toast {
    fn rect(&self) -> Rect {
        let x = PAD + (1.0 - self.slide.value.clamp(0.0, 1.0)) * (WIDTH + PAD);
        Rect::new(x, self.y.value, WIDTH, self.height)
    }

    /// Gone out at the right, and still.
    fn out(&self) -> bool {
        self.leaving && self.slide.value <= 0.02 && !self.slide.is_moving()
    }
}

#[derive(Default)]
pub struct Toasts {
    /// Newest first.
    pub list: Vec<Toast>,
    /// What waits for room, oldest first.
    pub waiting: Vec<u32>,
    /// The toast under the pointer: its time stands still.
    pub hovered: Option<u32>,
}

impl Toasts {
    pub fn is_empty(&self) -> bool {
        self.list.is_empty() && self.waiting.is_empty()
    }

    /// Shown, of those not on their way out.
    fn showing(&self) -> usize {
        self.list.iter().filter(|toast| !toast.leaving).count()
    }

    /// One more, or one anew in its place; waits while `most` show.
    pub fn show(&mut self, id: u32, critical: bool, most: usize) {
        if let Some(toast) = self.list.iter_mut().find(|toast| toast.id == id && !toast.leaving) {
            toast.stale = true;
            toast.critical = critical;
            return;
        }
        if self.waiting.contains(&id) {
            return;
        }
        if self.showing() >= most.max(1) {
            self.waiting.push(id);
            return;
        }
        self.list.insert(
            0,
            Toast {
                id,
                tree: Tree::default(),
                y: Spring::pixels(PAD),
                slide: Spring::new(0.0),
                height: 0.0,
                leaving: false,
                critical,
                stale: true,
                placed: false,
            },
        );
    }

    /// Out it goes; the next waiting one comes, if any. True if it was there.
    pub fn leave(&mut self, id: u32) -> Option<u32> {
        self.waiting.retain(|other| *other != id);
        let toast = self.list.iter_mut().find(|toast| toast.id == id && !toast.leaving)?;
        toast.leaving = true;
        toast.slide.to(0.0);
        if self.hovered == Some(id) {
            self.hovered = None;
        }
        (!self.waiting.is_empty()).then(|| self.waiting.remove(0))
    }

    pub fn has(&self, id: u32) -> bool {
        self.list.iter().any(|toast| toast.id == id && !toast.leaving)
    }

    pub fn any_critical(&self) -> bool {
        self.list.iter().any(|toast| toast.critical)
    }

    /// Moves them `dt` seconds on; those gone out are let go. True while
    /// one moves.
    pub fn update(&mut self, dt: f32, now: f64, theme: &Theme) -> bool {
        let place = theme.motion.feel(Part::Micro);
        let slide = theme.motion.feel(Part::Content);
        let mut moving = false;
        for toast in &mut self.list {
            moving |= toast.y.step(dt, &place);
            moving |= toast.slide.step(dt, &slide);
            moving |= toast.tree.update(dt, now, theme);
        }
        self.list.retain(|toast| !toast.out());
        moving
    }

    pub fn wake(&self) -> Option<std::time::Duration> {
        self.list.iter().filter_map(|toast| toast.tree.wake()).min()
    }

    /// Lays each out, `content` giving what a stale one shows, and sets
    /// where each goes: one under another from the top, the newest first;
    /// one leaving stays where it is.
    pub fn layout(&mut self, view: &View, text: &mut Text, theme: &Theme, content: impl Fn(u32) -> Option<El<Msg>>) {
        let inside = View {
            width: WIDTH,
            height: view.height,
            scale: view.scale,
        };
        for toast in &mut self.list {
            if toast.stale
                && let Some(el) = content(toast.id)
            {
                // Under a column: the toast as tall as it needs, not the room.
                toast.tree.set(El::column(0.0, vec![el]));
                toast.stale = false;
            }
            if toast.tree.needs_layout(&inside) {
                toast.tree.layout(&inside, text, theme);
            }
            toast.height = toast.tree.nodes().get(1).map_or(0.0, |toast| toast.rect.h);
        }
        let mut y = PAD;
        for toast in &mut self.list {
            if toast.leaving {
                continue;
            }
            if toast.placed {
                toast.y.to(y);
            } else {
                // New: in place at once, coming in from the right.
                toast.y.snap(y);
                toast.slide.to(1.0);
                toast.placed = true;
            }
            y += toast.height + GAP;
        }
    }

    pub fn render(&self, frame: &mut Frame, theme: &Theme) {
        let colors = &theme.colors;
        let radius = theme.sizes.radius;
        let base = frame.offset();
        for toast in self.list.iter().rev() {
            let shown = toast.slide.value.clamp(0.0, 1.0);
            if shown <= 0.0 {
                continue;
            }
            let rect = toast.rect();
            frame.alpha(shown);
            frame.shadow(rect, radius, 12.0, colors.shadow.alpha(0.35));
            frame.fill(rect, radius, colors.surface_container_high);
            if toast.critical {
                frame.ring(rect, radius, 1.5, colors.error);
            }
            frame.clip(Some(rect));
            frame.set_offset(base.0 + rect.x, base.1 + rect.y);
            toast.tree.paint(frame, theme);
            frame.set_offset(base.0, base.1);
            frame.clip(None);
        }
        frame.alpha(1.0);
    }

    /// Where they take the pointer: each where it is now.
    pub fn rects(&self) -> Vec<Rect> {
        self.list
            .iter()
            .filter(|toast| !toast.leaving)
            .map(Toast::rect)
            .collect()
    }

    /// The toast at `x`, `y`, and where in it that is.
    pub fn at(&self, x: f32, y: f32) -> Option<(u32, f32, f32)> {
        self.list
            .iter()
            .filter(|toast| !toast.leaving)
            .map(|toast| (toast.id, toast.rect()))
            .find(|(_, rect)| rect.contains(x, y))
            .map(|(id, rect)| (id, x - rect.x, y - rect.y))
    }

    pub fn tree_mut(&mut self, id: u32) -> Option<&mut Tree<Msg>> {
        self.list
            .iter_mut()
            .find(|toast| toast.id == id && !toast.leaving)
            .map(|toast| &mut toast.tree)
    }

    pub fn moving(&self) -> bool {
        self.list
            .iter()
            .any(|toast| toast.y.is_moving() || toast.slide.is_moving())
    }
}

/// A toast's content: the app with its icon and a close button; the summary
/// and the body, a picture of its own beside them; its buttons; a field for a
/// reply where the app takes one, `draft` in it.
pub fn view(notification: &Notification, draft: &str, theme: &Theme) -> El<Msg> {
    let id = notification.id;
    let faint = Ink::Own(theme.colors.on_surface.alpha(0.6));
    let icon = match &notification.icon {
        Some(picture) => El::picture(picture.clone())
            .width(Size::Fixed(16.0))
            .height(Size::Fixed(16.0)),
        None => El::icon("bell")
            .ink(faint)
            .width(Size::Fixed(16.0))
            .height(Size::Fixed(16.0)),
    };
    let app = if notification.app.is_empty() {
        "Notification".to_owned()
    } else {
        notification.app.clone()
    };
    let close = El::press(0.0, vec![El::icon("close").ink(faint)], Some(Msg::ToastClose(id)))
        .key(("close", id))
        .padding(Edges::all(2.0));
    let head = El::row(
        8.0,
        vec![
            icon,
            El::text(app, Type::Label).ink(faint).ellipsis().width(Size::Fill(1.0)),
            close,
        ],
    )
    .align(Align::Center)
    .width(Size::Fill(1.0));
    let summary_ink = if notification.urgency == Urgency::Critical {
        Ink::Error
    } else {
        Ink::OnSurface
    };
    let mut words = vec![
        El::text(notification.summary.clone(), Type::Title)
            .weight(600.0)
            .ink(summary_ink)
            .width(Size::Fill(1.0)),
    ];
    if !notification.body.is_empty() {
        words.push(
            El::text(notification.body.clone(), Type::Body)
                .ink(Ink::OnSurfaceVariant)
                .width(Size::Fill(1.0)),
        );
    }
    let mut middle = vec![El::column(4.0, words).width(Size::Fill(1.0))];
    if let Some(image) = &notification.image {
        middle.push(
            El::picture(image.clone())
                .width(Size::Fixed(48.0))
                .height(Size::Fixed(48.0)),
        );
    }
    let mut rows = vec![head, El::row(12.0, middle).width(Size::Fill(1.0))];
    if !notification.actions.is_empty() {
        rows.push(El::row(
            8.0,
            notification
                .actions
                .iter()
                .map(|(key, label)| {
                    El::button(
                        label.clone(),
                        ButtonStyle::Tonal,
                        Some(Msg::ToastAction(id, key.clone())),
                    )
                    .key(("action", id, key.as_str()))
                })
                .collect(),
        ));
    }
    if let Some(placeholder) = &notification.reply {
        rows.push(
            El::field(draft, placeholder.clone(), Some(move |text| Msg::ToastDraft(id, text)))
                .on_submit(Msg::ToastReply(id))
                .key(("reply", id))
                .width(Size::Fill(1.0)),
        );
    }
    El::press(
        8.0,
        vec![El::column(8.0, rows).width(Size::Fill(1.0))],
        notification.default_action.then_some(Msg::ToastOpen(id)),
    )
    .key(("toast", id))
    .padding(Edges::all(14.0))
    .width(Size::Fixed(WIDTH))
}
