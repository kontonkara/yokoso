//! The bar: a floating strip on the main screen, its parts at its start, in
//! its middle and at its end. Each part shows what the system says now and
//! changes the frame it hears of a change; at rest it draws nothing.

pub mod clock;

use std::{collections::HashMap, time::Duration};

use serde_json::{Value, json};
use yokoso_ui::{
    Rect,
    app::{
        Anchor, Frame, FrameTime, KeyboardInteractivity, Layer, LayerSpec, Output, OutputId, Shell, Spec, Ui, View,
        WindowSpec,
    },
    calloop::timer::{TimeoutAction, Timer},
    el::{Align, Edges, El, Size, Type, tree::Tree},
    input::Input,
    text::Text,
    theme::Theme,
};

use crate::config::{Config, Edge, Module};

/// What the bar's services send it.
pub enum Message {
    Config(Result<Config, String>),
}

/// What its parts send when used.
#[derive(Clone, Debug, PartialEq)]
pub enum Msg {}

/// Where a bar is: on a screen, or in a window of its own, to try it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    Screen(OutputId),
    Window,
}

/// Each bar's elements, and the description they were last built from.
#[derive(Default)]
struct Surface {
    tree: Tree<Msg>,
    built: u64,
}

pub struct Bar {
    window: bool,
    config: Config,
    /// Why the config on disk is not the one in force, if it isn't.
    config_error: Option<String>,
    theme: Theme,
    /// Put away by `msg bar hide`.
    hidden: bool,
    surfaces: HashMap<Place, Surface>,
    /// Counts changes to what the bar says: a bar built under an older count
    /// is built anew.
    generation: u64,
    time: String,
    date: Option<String>,
}

/// The window's size, to try the bar beside the session.
const WINDOW_WIDTH: u32 = 1280;

impl Bar {
    pub fn new(window: bool, config: Config) -> Self {
        let (time, date) = clock::says(&clock::now(), &config.bar.clock);
        Self {
            window,
            config,
            config_error: None,
            theme: Theme::default(),
            hidden: false,
            surfaces: HashMap::new(),
            generation: 0,
            time,
            date,
        }
    }

    /// Starts its clock: from now on it wakes only when what it says changes.
    pub fn start(ui: &mut Ui<Self>, bar: &Self) {
        let first = clock::next_change(&clock::now(), &bar.config.bar.clock);
        let _ = ui
            .handle()
            .insert_source(Timer::from_duration(first + SLACK), |_, _, app| {
                let next = app.shell.tick(&mut app.ui);
                TimeoutAction::ToDuration(next + SLACK)
            });
    }

    /// The clock's tick: what it says now, and how long until it changes.
    fn tick(&mut self, ui: &mut Ui<Self>) -> Duration {
        let now = clock::now();
        let (time, date) = clock::says(&now, &self.config.bar.clock);
        if (&time, &date) != (&self.time, &self.date) {
            self.time = time;
            self.date = date;
            self.changed(ui);
        }
        clock::next_change(&now, &self.config.bar.clock)
    }

    /// What it says has changed: every bar is built anew and drawn.
    fn changed(&mut self, ui: &mut Ui<Self>) {
        self.generation += 1;
        ui.dirty_all();
    }

    fn apply(&mut self, ui: &mut Ui<Self>, loaded: Result<Config, String>) {
        match loaded {
            Ok(config) => {
                self.config_error = None;
                if config != self.config {
                    tracing::info!("config changed");
                    let placed = |config: &Config| {
                        let bar = &config.bar;
                        (bar.position, bar.height, bar.margin, bar.screens.clone())
                    };
                    if placed(&config) != placed(&self.config) {
                        ui.refresh_surfaces();
                    }
                    self.config = config;
                    let (time, date) = clock::says(&clock::now(), &self.config.bar.clock);
                    (self.time, self.date) = (time, date);
                    self.changed(ui);
                }
            }
            Err(error) => {
                tracing::warn!(%error, "config kept as it was");
                self.config_error = Some(error);
            }
        }
    }

    /// The bar's own rect on `place`: all of a screen's surface, or in from
    /// a window's edges as it would float on a screen.
    fn rect(&self, place: Place, view: &View) -> Rect {
        match place {
            Place::Screen(_) => Rect::new(0.0, 0.0, view.width, view.height),
            Place::Window => {
                let margin = self.config.bar.margin;
                Rect::new(margin, margin, view.width - 2.0 * margin, self.config.bar.height)
            }
        }
    }

    /// A part, as it shows now; None where it has nothing to show.
    fn module(&self, module: Module) -> Option<El<Msg>> {
        let sizes = &self.theme.sizes;
        let pad = Edges {
            left: sizes.pad * 0.75,
            right: sizes.pad * 0.75,
            ..Edges::default()
        };
        match module {
            Module::Clock => {
                let mut parts = vec![El::text(&self.time, Type::Body).weight(600.0).tabular().rolling()];
                if let Some(date) = &self.date {
                    parts.push(El::text(date, Type::Body).weight(500.0));
                }
                Some(
                    El::press(sizes.gap * 0.75, parts, None)
                        .key("clock")
                        .padding(pad)
                        .height(Size::Fill(1.0)),
                )
            }
            _ => None,
        }
    }

    /// The whole bar: its start, its middle, and its end, the middle in the
    /// middle however long the others are.
    fn view(&self) -> El<Msg> {
        let bar = &self.config.bar;
        let gap = self.theme.sizes.gap / 3.0;
        let parts =
            |modules: &[Module]| -> Vec<El<Msg>> { modules.iter().filter_map(|module| self.module(*module)).collect() };
        let mut end = vec![El::spacer().width(Size::Fill(1.0))];
        end.extend(parts(&bar.right));
        El::row(
            0.0,
            vec![
                El::row(gap, parts(&bar.left))
                    .align(Align::Center)
                    .width(Size::Fill(1.0))
                    .height(Size::Fill(1.0)),
                El::row(gap, parts(&bar.center))
                    .align(Align::Center)
                    .height(Size::Fill(1.0)),
                El::row(gap, end)
                    .align(Align::Center)
                    .width(Size::Fill(1.0))
                    .height(Size::Fill(1.0)),
            ],
        )
        .align(Align::Center)
        .padding(Edges::all(4.0))
        .fill()
    }
}

/// Timers fire this long after the change, never just before it.
const SLACK: Duration = Duration::from_millis(5);

/// The screens a bar goes on: those the config names, by description or
/// connector; else the main one, the built-in screen or the first.
pub fn chosen<'a>(outputs: &'a [Output], screens: &[String]) -> Vec<&'a Output> {
    if !screens.is_empty() {
        return outputs
            .iter()
            .filter(|output| {
                screens
                    .iter()
                    .any(|screen| *screen == output.name || output.description.contains(screen.as_str()))
            })
            .collect();
    }
    let built_in = |output: &&Output| ["eDP", "LVDS", "DSI"].iter().any(|kind| output.name.starts_with(kind));
    outputs.iter().find(built_in).or(outputs.first()).into_iter().collect()
}

impl Shell for Bar {
    type Key = Place;
    type Message = Message;

    fn message(&mut self, ui: &mut Ui<Self>, message: Message) {
        match message {
            Message::Config(loaded) => self.apply(ui, loaded),
        }
    }

    fn surfaces(&self, outputs: &[Output]) -> Vec<(Place, Spec)> {
        let bar = &self.config.bar;
        if self.hidden {
            return Vec::new();
        }
        if self.window {
            let spec = WindowSpec {
                title: "yokoso bar".into(),
                app_id: "yokoso".into(),
                size: (WINDOW_WIDTH, (bar.height + 2.0 * bar.margin) as u32),
            };
            return vec![(Place::Window, Spec::Window(spec))];
        }
        let margin = bar.margin.round() as i32;
        let (edge, margins) = match bar.position {
            Edge::Top => (Anchor::TOP, (margin, margin, 0, margin)),
            Edge::Bottom => (Anchor::BOTTOM, (0, margin, margin, margin)),
        };
        chosen(outputs, &bar.screens)
            .into_iter()
            .map(|output| {
                let spec = LayerSpec {
                    output: output.id,
                    layer: Layer::Top,
                    anchor: edge | Anchor::LEFT | Anchor::RIGHT,
                    size: (0, bar.height.round() as u32),
                    margin: margins,
                    exclusive_zone: bar.height.round() as i32,
                    keyboard: KeyboardInteractivity::None,
                    namespace: "yokoso-bar".into(),
                };
                (Place::Screen(output.id), Spec::Layer(spec))
            })
            .collect()
    }

    fn input(&mut self, ui: &mut Ui<Self>, key: Place, input: &Input) {
        let Some(surface) = self.surfaces.get_mut(&key) else {
            return;
        };
        let response = surface.tree.input(input);
        if response.redraw {
            ui.dirty(key);
        }
        if let Some(wake) = response.wake {
            ui.wake_after(key, wake);
        }
    }

    fn update(&mut self, key: Place, time: &FrameTime) -> bool {
        self.surfaces
            .get_mut(&key)
            .is_some_and(|surface| surface.tree.update(time.dt as f32, time.now, &self.theme))
    }

    fn layout(&mut self, key: Place, view: &View, text: &mut Text) {
        let rect = self.rect(key, view);
        let fresh = self
            .surfaces
            .get(&key)
            .is_none_or(|surface| surface.built != self.generation);
        let content = fresh.then(|| self.view());
        let generation = self.generation;
        let surface = self.surfaces.entry(key).or_default();
        if let Some(content) = content {
            surface.tree.set(content);
            surface.built = generation;
        }
        surface.tree.set_origin(rect.x, rect.y);
        let inside = View {
            width: rect.w,
            height: rect.h,
            scale: view.scale,
        };
        if surface.tree.needs_layout(&inside) {
            surface.tree.layout(&inside, text, &self.theme);
        }
    }

    fn render(&self, key: Place, frame: &mut Frame) {
        let view = frame.view();
        let rect = self.rect(key, &view);
        frame.fill(rect, self.theme.sizes.radius, self.theme.colors.surface_container);
        if let Some(surface) = self.surfaces.get(&key) {
            surface.tree.paint(frame, &self.theme);
        }
    }

    fn wake(&self, key: Place) -> Option<Duration> {
        self.surfaces.get(&key).and_then(|surface| surface.tree.wake())
    }
}

impl crate::ipc::Answer for Bar {
    fn answer(&mut self, ui: &mut Ui<Self>, words: &[String]) -> Result<Value, String> {
        match words.first().map(String::as_str) {
            Some("ping") => Ok(json!("pong")),
            Some("reload") => {
                self.apply(ui, crate::config::load(&crate::config::dir()));
                match &self.config_error {
                    Some(error) => Err(error.clone()),
                    None => Ok(json!(null)),
                }
            }
            Some("bar") => {
                self.hidden = match words.get(1).map(String::as_str) {
                    Some("show") => false,
                    Some("hide") => true,
                    Some("toggle") | None => !self.hidden,
                    Some(other) => return Err(format!("show, hide or toggle, not {other}")),
                };
                ui.refresh_surfaces();
                Ok(json!(!self.hidden))
            }
            Some("stats") => {
                let stats = ui.stats();
                Ok(json!({
                    "wakeups": stats.wakeups,
                    "frames": stats.frames,
                    "last_frame_ms": stats.last_frame * 1000.0,
                    "partial": stats.partial,
                }))
            }
            Some("dump") => Ok(json!({
                "config_error": self.config_error,
                "hidden": self.hidden,
                "time": self.time,
                "date": self.date,
                "bars": self.surfaces.iter().map(|(place, surface)| json!({
                    "place": format!("{place:?}"),
                    "nodes": surface.tree.nodes().iter().map(|node| json!({
                        "id": format!("{:016x}", node.id),
                        "role": format!("{:?}", node.role),
                        "rect": [node.rect.x, node.rect.y, node.rect.w, node.rect.h],
                    })).collect::<Vec<_>>(),
                    "clock": surface.tree.rect_of("clock").map(|r| [r.x, r.y, r.w, r.h]),
                })).collect::<Vec<_>>(),
                "outputs": ui.outputs().iter().map(|o| json!({
                    "name": o.name, "description": o.description, "size": o.size,
                })).collect::<Vec<_>>(),
                "surfaces": ui.surface_info().iter().map(|s| json!({
                    "key": s.key, "size": s.size, "scale": s.scale, "frames": s.frames,
                })).collect::<Vec<_>>(),
            })),
            Some(other) => Err(format!("no such command: {other}")),
            None => Err("no command".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(id: u32, name: &str, description: &str) -> Output {
        Output {
            id: OutputId::for_tests(id),
            name: name.into(),
            description: description.into(),
            size: (1920, 1080),
        }
    }

    #[test]
    fn it_goes_on_the_built_in_screen_or_the_ones_named() {
        let outputs = [
            output(1, "HDMI-A-1", "Dell Inc. DELL U2720Q"),
            output(2, "eDP-1", "BOE 0x0ABC"),
        ];
        let names = |chosen: Vec<&Output>| chosen.iter().map(|o| o.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(chosen(&outputs, &[])), ["eDP-1"]);
        assert_eq!(
            names(chosen(&outputs[..1], &[])),
            ["HDMI-A-1"],
            "no built-in: the first"
        );
        assert_eq!(names(chosen(&outputs, &["DELL U2720Q".into()])), ["HDMI-A-1"]);
        assert_eq!(
            names(chosen(&outputs, &["eDP-1".into(), "Dell".into()])),
            ["HDMI-A-1", "eDP-1"]
        );
    }
}
