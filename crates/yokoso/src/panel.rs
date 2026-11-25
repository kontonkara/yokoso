//! A panel that grows out of a button: the shell's main motion, tried before
//! there is a bar. Its shape grows on the panels' spring; its content stays
//! where it will be and shows once the shape has grown far enough; closing,
//! it shrinks back into its button and fades only at the very end. Asked to
//! turn mid-way, it turns from where it is, at the speed it had.

use yokoso_ui::{
    Rect,
    app::{Frame, View},
    el::{El, tree::Tree},
    motion::{Part, Spring},
    text::Text,
    theme::Theme,
};

/// The content shows from this far into the growing.
const REVEAL: f32 = 0.45;
/// Closing, the shape fades once this little of the way is left.
const FADE: f32 = 0.15;
/// Turned back while closing with less of its content showing than this,
/// the content comes in anew.
const AGAIN: f32 = 0.5;

pub struct Panel<M> {
    pub open: bool,
    /// Its shape: where it is, and how large.
    x: Spring,
    y: Spring,
    w: Spring,
    h: Spring,
    /// How much of the shape and of its content shows.
    shape: Spring,
    content: Spring,
    /// Where it grows from, and to.
    from: Rect,
    to: Rect,
    pub tree: Tree<M>,
}

impl<M: Clone> Default for Panel<M> {
    fn default() -> Self {
        Self {
            open: false,
            x: Spring::pixels(0.0),
            y: Spring::pixels(0.0),
            w: Spring::pixels(0.0),
            h: Spring::pixels(0.0),
            shape: Spring::new(0.0),
            content: Spring::new(0.0),
            from: Rect::default(),
            to: Rect::default(),
            tree: Tree::default(),
        }
    }
}

impl<M: Clone> Panel<M> {
    /// Opens from `button` to `to`, or closes back into `button`: from
    /// wherever it is now.
    pub fn toggle(&mut self, button: Rect, to: Rect) {
        self.open = !self.open;
        self.from = button;
        self.to = to;
        let from_rest = !self.shape.is_moving() && self.shape.value == 0.0;
        if self.open && !from_rest && self.content.value < AGAIN {
            // Turned back while closing, its content mostly gone: it comes in
            // anew, as from rest, where the shape is now.
            self.tree = Tree::default();
        }
        if self.open && from_rest {
            // From rest, closed: it starts as the button, and its content comes
            // in anew, one after another.
            self.tree = Tree::default();
            self.x.snap(button.x);
            self.y.snap(button.y);
            self.w.snap(button.w);
            self.h.snap(button.h);
            self.content.snap(0.0);
        }
        let aim = if self.open { to } else { button };
        self.x.to(aim.x);
        self.y.to(aim.y);
        self.w.to(aim.w);
        self.h.to(aim.h);
        self.shape.to(1.0);
        if !self.open {
            self.content.to(0.0);
        }
    }

    /// Open already: it goes on to `to`, as grown from `button`, its content
    /// coming in anew. Closed, it opens.
    pub fn retarget(&mut self, button: Rect, to: Rect) {
        if !self.open {
            self.toggle(button, to);
            return;
        }
        self.from = button;
        self.to = to;
        self.x.to(to.x);
        self.y.to(to.y);
        self.w.to(to.w);
        self.h.to(to.h);
        self.tree = Tree::default();
        self.content.snap(0.0);
    }

    /// How far the shape has grown, 0 at its button to 1 open.
    fn progress(&self) -> f32 {
        let along = |now: f32, from: f32, to: f32| {
            if (to - from).abs() > 1.0 {
                (now - from) / (to - from)
            } else {
                1.0
            }
        };
        let w = along(self.w.value, self.from.w, self.to.w);
        let h = along(self.h.value, self.from.h, self.to.h);
        ((w + h) / 2.0).clamp(0.0, 1.0)
    }

    /// Held still `at` of the way from its button to open, its content all
    /// there: to see, and test, what shows where mid-way.
    pub fn freeze(&mut self, button: Rect, to: Rect, at: f32) {
        let lerp = |a: f32, b: f32| a + (b - a) * at;
        self.open = true;
        self.from = button;
        self.to = to;
        self.x.snap(lerp(button.x, to.x));
        self.y.snap(lerp(button.y, to.y));
        self.w.snap(lerp(button.w, to.w));
        self.h.snap(lerp(button.h, to.h));
        self.shape.snap(1.0);
        self.content.snap(1.0);
    }

    /// Gone at once, as when what it grew from is gone too.
    pub fn hide(&mut self) {
        self.open = false;
        self.shape.snap(0.0);
        self.content.snap(0.0);
    }

    /// Its shape now.
    pub fn shape(&self) -> Rect {
        // A spring past its mark going into a line is no size, not less.
        Rect::new(self.x.value, self.y.value, self.w.value.max(0.0), self.h.value.max(0.0))
    }

    pub fn visible(&self) -> bool {
        self.shape.value > 0.0 || self.shape.is_moving()
    }

    /// Where its content goes: where it will be when open.
    pub fn content_rect(&self) -> Rect {
        self.to
    }

    /// Moves it `dt` seconds on. True while it moves.
    pub fn update(&mut self, dt: f32, now: f64, theme: &Theme) -> bool {
        let feel = theme.motion.feel(Part::Panels);
        let content = theme.motion.feel(Part::Content);
        let mut moving = false;
        for spring in [&mut self.x, &mut self.y, &mut self.w, &mut self.h] {
            moving |= spring.step(dt, &feel);
        }
        let progress = self.progress();
        if self.open && progress >= REVEAL {
            self.content.to(1.0);
        }
        if !self.open && progress <= FADE {
            self.shape.to(0.0);
        }
        moving |= self.shape.step(dt, &content);
        moving |= self.content.step(dt, &content);
        // Its content's own entrances start as it shows, not before, unseen.
        if self.content.target > 0.0 {
            moving |= self.tree.update(dt, now, theme);
        }
        moving
    }

    /// When its content wants a frame though nothing moves: only once it
    /// shows, as its content moves only then.
    pub fn wake(&self) -> Option<std::time::Duration> {
        (self.content.target > 0.0).then(|| self.tree.wake()).flatten()
    }

    pub fn layout(&mut self, view: &View, text: &mut Text, theme: &Theme) {
        self.tree.set_origin(self.to.x, self.to.y);
        let inside = View {
            width: self.to.w,
            height: self.to.h,
            scale: view.scale,
        };
        if self.tree.needs_layout(&inside) {
            self.tree.layout(&inside, text, theme);
        }
    }

    pub fn set(&mut self, content: El<M>) {
        self.tree.set(content);
    }

    pub fn render(&self, frame: &mut Frame, theme: &Theme) {
        let shown = self.shape.value.clamp(0.0, 1.0);
        if shown <= 0.0 {
            return;
        }
        let colors = &theme.colors;
        let shape = Rect::new(self.x.value, self.y.value, self.w.value.max(0.0), self.h.value.max(0.0));
        let radius = theme.sizes.radius;
        frame.alpha(shown);
        frame.shadow(shape, radius, 16.0, colors.shadow.alpha(0.4));
        frame.fill(shape, radius, colors.surface_container_high);
        frame.clip(Some(shape));
        frame.alpha(shown * self.content.value.clamp(0.0, 1.0));
        self.tree.paint(frame, theme);
        frame.clip(None);
        frame.alpha(1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yokoso_ui::el::TileEl;

    fn content() -> El<()> {
        El::column(
            0.0,
            vec![El::tile(TileEl {
                icon: "bell",
                title: "t".into(),
                subtitle: String::new(),
                on: false,
                pending: false,
                on_press: Some(()),
                on_more: None,
            })],
        )
        .stagger()
    }

    /// Laid out as the shell does after a toggle: its content set anew.
    fn lay(panel: &mut Panel<()>) {
        panel.set(content());
        panel
            .tree
            .layout_with(Rect::new(0.0, 0.0, 300.0, 200.0), &mut |_: &El<()>, _| (10.0, 10.0));
    }

    fn icon_size(panel: &Panel<()>) -> f32 {
        let tile = panel.tree.nodes()[1].id;
        panel.tree.state(tile).unwrap().pop.value
    }

    #[test]
    fn turned_back_with_its_content_mostly_gone_it_comes_in_anew() {
        let theme = Theme::default();
        let (button, to) = (Rect::new(0.0, 0.0, 40.0, 40.0), Rect::new(0.0, 50.0, 300.0, 200.0));
        let mut panel = Panel::<()>::default();
        panel.toggle(button, to);
        lay(&mut panel);
        while panel.update(1.0 / 240.0, 0.0, &theme) {}
        assert_eq!(icon_size(&panel), 1.0);
        // Closing, until little of the content shows; then open again.
        panel.toggle(button, to);
        while panel.content.value >= AGAIN {
            panel.update(1.0 / 240.0, 0.0, &theme);
        }
        assert!(panel.visible(), "still closing");
        panel.toggle(button, to);
        lay(&mut panel);
        assert!(icon_size(&panel) < 1.0, "its icons pop in again");
    }
}
