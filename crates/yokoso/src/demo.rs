//! The demo: every element on a page of its own, and the switches for theme,
//! palette, motion and scale, to try them by hand before anything is built
//! of them. In a window, or as a layer over the desktop.

use serde_json::{Value, json};
use yokoso_ui::{
    Rect,
    app::{
        Anchor, Frame, FrameTime, KeyboardInteractivity, Layer, LayerSpec, Output, OutputId, Shell, Spec, Ui, View,
        WindowSpec,
    },
    el::{Edges, El, Ink, Type, tree::Tree},
    input::Input,
    text::Text,
    theme::Theme,
};

/// Where the demo is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    Window,
    Layer(OutputId),
}

/// What its elements tell it.
#[derive(Clone, Debug)]
pub enum Msg {}

pub struct Demo {
    layer: bool,
    theme: Theme,
    tree: Tree<Msg>,
    /// The description is to be built anew before the next layout.
    stale: bool,
}

const WIDTH: u32 = 960;
const HEIGHT: u32 = 640;

impl Demo {
    pub fn new(layer: bool) -> Self {
        Self {
            layer,
            theme: Theme::default(),
            tree: Tree::default(),
            stale: true,
        }
    }

    /// What the elements said, done.
    fn apply(&mut self, message: Msg) {
        match message {}
    }

    /// The description of everything shown, from the state.
    fn view(&self) -> El<Msg> {
        let sizes = &self.theme.sizes;
        El::column(
            sizes.gap,
            vec![
                El::text("yokoso elements", Type::Headline).weight(600.0),
                El::text("Every element of the shell on a page of its own.", Type::Body).ink(Ink::OnSurfaceVariant),
            ],
        )
        .padding(Edges::all(sizes.pad * 1.5))
        .fill()
    }
}

impl Shell for Demo {
    type Key = Place;
    type Message = ();

    fn surfaces(&self, outputs: &[Output]) -> Vec<(Place, Spec)> {
        if !self.layer {
            let spec = WindowSpec {
                title: "yokoso elements".into(),
                app_id: "yokoso".into(),
                size: (WIDTH, HEIGHT),
            };
            return vec![(Place::Window, Spec::Window(spec))];
        }
        // As a layer: in the middle of the first output, over everything.
        outputs
            .first()
            .map(|output| {
                let spec = LayerSpec {
                    output: output.id,
                    layer: Layer::Overlay,
                    anchor: Anchor::empty(),
                    size: (WIDTH, HEIGHT),
                    margin: (0, 0, 0, 0),
                    exclusive_zone: 0,
                    keyboard: KeyboardInteractivity::OnDemand,
                    namespace: "yokoso-demo".into(),
                };
                (Place::Layer(output.id), Spec::Layer(spec))
            })
            .into_iter()
            .collect()
    }

    fn input(&mut self, ui: &mut Ui<Self>, key: Place, input: &Input) {
        let response = self.tree.input(input);
        for message in response.messages {
            self.apply(message);
        }
        if response.redraw {
            ui.dirty(key);
        }
    }

    fn update(&mut self, _: Place, time: &FrameTime) -> bool {
        self.tree.update(time.dt as f32, &self.theme)
    }

    fn layout(&mut self, _: Place, view: &View, text: &mut Text) {
        if self.stale {
            self.tree.set(self.view());
            self.stale = false;
        }
        if self.tree.needs_layout(view) {
            self.tree.layout(view, text, &self.theme);
        }
    }

    fn render(&self, key: Place, frame: &mut Frame) {
        let view = frame.view();
        let whole = Rect::new(0.0, 0.0, view.width, view.height);
        let radius = if key == Place::Window {
            0.0
        } else {
            self.theme.sizes.radius
        };
        frame.fill(whole, radius, self.theme.colors.surface_container);
        self.tree.paint(frame, &self.theme);
    }
}

impl crate::ipc::Answer for Demo {
    fn answer(&mut self, ui: &mut Ui<Self>, words: &[String]) -> Result<Value, String> {
        match words.first().map(String::as_str) {
            Some("ping") => Ok(json!("pong")),
            Some("stats") => {
                let stats = ui.stats();
                Ok(json!({ "wakeups": stats.wakeups, "frames": stats.frames }))
            }
            Some("dump") => Ok(json!({
                "nodes": self.tree.nodes().iter().map(|node| json!({
                    "id": format!("{:016x}", node.id),
                    "role": format!("{:?}", node.role),
                    "rect": [node.rect.x, node.rect.y, node.rect.w, node.rect.h],
                })).collect::<Vec<_>>(),
                "hovered": self.tree.hovered.map(|id| format!("{id:016x}")),
            })),
            Some(other) => Err(format!("no such command: {other}")),
            None => Err("no command".into()),
        }
    }
}
