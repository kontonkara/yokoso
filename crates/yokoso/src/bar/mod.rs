//! The bar: a floating strip on the main screen, its parts at its start, in
//! its middle and at its end. Each part shows what the system says now and
//! changes the frame it hears of a change; at rest it draws nothing.

mod battery;
pub mod clock;
mod keyboard;
mod network;
mod workspaces;

use std::{cell::RefCell, collections::HashMap, time::Duration};

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
    motion::{Part, Spring},
    text::Text,
    theme::Theme,
};

use crate::{
    config::{Config, Edge, Module},
    niri, nm, upower,
};

/// What the bar's services send it.
pub enum Message {
    Config(Result<Config, String>),
    Niri(niri::Event),
    Power(upower::Power),
    Net(nm::Net),
}

/// What its parts send when used.
#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    /// To the workspace, to the window, and up (more) or down the workspaces.
    Workspace(u64),
    Window(u64),
    WorkspaceStep(f32),
    /// The layout before (more) or the next.
    LayoutStep(f32),
    /// What the config says a part opens on a click.
    Open(Module),
}

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
    /// The active workspace's frame as it shows: x, y, width, height.
    frame: Option<[Spring; 4]>,
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
    niri: niri::State,
    power: upower::Power,
    net: nm::Net,
    /// Each screen's connector and logical width, as the surfaces were asked
    /// for: what a bar there shows depends on it.
    screens: RefCell<HashMap<OutputId, (String, f32)>>,
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
            niri: niri::State::default(),
            power: upower::Power::default(),
            net: nm::Net::default(),
            screens: RefCell::default(),
        }
    }

    /// Starts its services, and its clock: from now on it wakes only when
    /// what it says changes.
    pub fn start(ui: &mut Ui<Self>, bar: &Self) {
        let sender = ui.sender();
        niri::listen(move |event| sender.send(Message::Niri(event)).is_ok());
        let sender = ui.sender();
        upower::listen(move |power| sender.send(Message::Power(power)).is_ok());
        let sender = ui.sender();
        nm::listen(move |net| sender.send(Message::Net(net)).is_ok());
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

    /// The screen a bar at `place` is about, by connector, and its logical
    /// width: a window's, the one with the focused workspace.
    fn screen(&self, place: Place) -> Option<(String, f32)> {
        match place {
            Place::Screen(id) => self.screens.borrow().get(&id).cloned(),
            Place::Window => {
                let output = self
                    .niri
                    .workspaces
                    .values()
                    .find(|workspace| workspace.is_focused)
                    .and_then(|workspace| workspace.output.clone())?;
                let width = self
                    .screens
                    .borrow()
                    .values()
                    .find(|(name, _)| *name == output)
                    .map_or(1920.0, |(_, width)| *width);
                Some((output, width))
            }
        }
    }

    /// The workspace that is active on `place`'s screen.
    fn active_workspace(&self, place: Place) -> Option<u64> {
        let (output, _) = self.screen(place)?;
        self.niri
            .workspaces_on(&output)
            .into_iter()
            .find(|workspace| workspace.is_active)
            .map(|workspace| workspace.id)
    }

    /// The command a part runs on a click, as the config has it.
    fn command(&self, module: Module) -> &[String] {
        let bar = &self.config.bar;
        match module {
            Module::Battery => &bar.battery.on_click,
            Module::Network => &bar.network.on_click,
            Module::Sound => &bar.sound.on_click,
            _ => &[],
        }
    }

    /// What a click on `module` sends: None where the config gives it nothing
    /// to open.
    fn opens(&self, module: Module) -> Option<Msg> {
        (!self.command(module).is_empty()).then_some(Msg::Open(module))
    }

    /// Runs what the config says `module` opens.
    fn open(&self, module: Module) {
        if let Some((program, args)) = self.command(module).split_first() {
            run(program, args);
        }
    }

    /// Does what a part asks.
    fn act(&mut self, msg: &Msg) {
        match msg {
            Msg::Workspace(_) | Msg::Window(_) | Msg::WorkspaceStep(_) => workspaces::act(msg),
            Msg::LayoutStep(steps) => keyboard::act(*steps),
            Msg::Open(module) => self.open(*module),
        }
    }

    /// A part, as it shows on `place` now; None where it has nothing to show.
    fn module(&self, place: Place, module: Module) -> Option<El<Msg>> {
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
            Module::Layout => keyboard::view(&self.niri, pad),
            Module::Battery => battery::view(&self.power, &self.theme, pad, self.opens(module)),
            Module::Network => Some(network::view(
                &self.net,
                &self.config.bar.network.vpn,
                &self.theme,
                pad,
                self.opens(module),
            )),
            Module::Workspaces => {
                let (output, width) = self.screen(place)?;
                workspaces::view(&self.niri, &output, width, &self.theme)
            }
            _ => None,
        }
    }

    /// The whole bar on `place`: its start, its middle, and its end, the
    /// middle in the middle however long the others are.
    fn view(&self, place: Place) -> El<Msg> {
        let bar = &self.config.bar;
        let gap = self.theme.sizes.gap / 3.0;
        let parts = |modules: &[Module]| -> Vec<El<Msg>> {
            modules
                .iter()
                .filter_map(|module| self.module(place, *module))
                .collect()
        };
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

/// Starts `program` on its own: the bar never waits on it, and it is
/// reaped when it ends.
fn run(program: &str, args: &[String]) {
    match std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .spawn()
    {
        Ok(mut child) => {
            let _ = std::thread::Builder::new()
                .name("reaper".into())
                .spawn(move || child.wait());
        }
        Err(error) => tracing::warn!(%error, program, "could not run"),
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
            Message::Net(net) => {
                if net != self.net {
                    self.net = net;
                    self.changed(ui);
                }
            }
            Message::Power(power) => {
                if power != self.power {
                    self.power = power;
                    self.changed(ui);
                }
            }
            Message::Niri(event) => {
                if self.niri.apply(event) {
                    self.changed(ui);
                }
            }
        }
    }

    fn surfaces(&self, outputs: &[Output]) -> Vec<(Place, Spec)> {
        *self.screens.borrow_mut() = outputs
            .iter()
            .map(|output| (output.id, (output.name.clone(), output.size.0 as f32)))
            .collect();
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
        for message in &response.messages {
            self.act(message);
        }
    }

    fn update(&mut self, key: Place, time: &FrameTime) -> bool {
        let Some(surface) = self.surfaces.get_mut(&key) else {
            return false;
        };
        let dt = time.dt as f32;
        let mut moving = surface.tree.update(dt, time.now, &self.theme);
        if let Some(frame) = &mut surface.frame {
            let feel = self.theme.motion.feel(Part::Micro);
            for spring in frame {
                moving |= spring.step(dt, &feel);
            }
        }
        moving
    }

    fn layout(&mut self, key: Place, view: &View, text: &mut Text) {
        let rect = self.rect(key, view);
        let fresh = self
            .surfaces
            .get(&key)
            .is_none_or(|surface| surface.built != self.generation);
        let content = fresh.then(|| self.view(key));
        let active = self.active_workspace(key);
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
        // The active workspace's frame: there at once the first time, then
        // gliding from one workspace to the next.
        match active.and_then(|id| surface.tree.rect_of(workspaces::key(id))) {
            Some(to) => {
                let to = [to.x, to.y, to.w, to.h];
                match &mut surface.frame {
                    Some(frame) => {
                        for (spring, to) in frame.iter_mut().zip(to) {
                            spring.to(to);
                        }
                    }
                    None => surface.frame = Some(to.map(Spring::pixels)),
                }
            }
            None => surface.frame = None,
        }
    }

    fn render(&self, key: Place, frame: &mut Frame) {
        let view = frame.view();
        let rect = self.rect(key, &view);
        let theme = &self.theme;
        frame.fill(rect, theme.sizes.radius, theme.colors.surface_container);
        let Some(surface) = self.surfaces.get(&key) else {
            return;
        };
        surface.tree.paint(frame, theme);
        if let Some(springs) = &surface.frame {
            let [x, y, w, h] = springs.map(|spring| spring.value);
            let at = Rect::new(x, y, w, h);
            frame.ring(at, (h / 2.0).min(theme.sizes.radius), 1.5, theme.colors.primary);
        }
    }

    fn wake(&self, key: Place) -> Option<Duration> {
        let surface = self.surfaces.get(&key)?;
        // The frame was sent somewhere new at layout: it moves from the next
        // frame on.
        let gliding = surface.frame.is_some_and(|frame| frame.iter().any(Spring::is_moving));
        if gliding {
            return Some(Duration::ZERO);
        }
        surface.tree.wake()
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
                "layouts": self.niri.layouts.names,
                "network": match &self.net {
                    nm::Net::Up(network) => json!({
                        "name": network.name,
                        "link": format!("{:?}", network.link),
                        "vpns": network::vpns(network, &self.config.bar.network.vpn),
                    }),
                    nm::Net::Unknown => json!(null),
                },
                "battery": match self.power {
                    upower::Power::Battery(battery) => json!({ "percentage": battery.percentage, "charge": format!("{:?}", battery.charge) }),
                    other => json!(format!("{other:?}")),
                },
                "layout": self.niri.layouts.names.get(self.niri.layouts.current_idx),
                "bars": self.surfaces.iter().map(|(place, surface)| json!({
                    "place": format!("{place:?}"),
                    "nodes": surface.tree.nodes().iter().map(|node| json!({
                        "id": format!("{:016x}", node.id),
                        "role": format!("{:?}", node.role),
                        "rect": [node.rect.x, node.rect.y, node.rect.w, node.rect.h],
                    })).collect::<Vec<_>>(),
                    "clock": surface.tree.rect_of("clock").map(|r| [r.x, r.y, r.w, r.h]),
                    "layout": surface.tree.rect_of("layout").map(|r| [r.x, r.y, r.w, r.h]),
                    "battery": surface.tree.rect_of("battery").map(|r| [r.x, r.y, r.w, r.h]),
                    "network": surface.tree.rect_of("network").map(|r| [r.x, r.y, r.w, r.h]),
                    "workspaces": self.screen(*place).map(|(output, _)| self.niri.workspaces_on(&output)
                        .iter()
                        .map(|ws| json!({
                            "id": ws.id,
                            "idx": ws.idx,
                            "active": ws.is_active,
                            "columns": self.niri.columns(ws.id).len(),
                            "rect": surface.tree.rect_of(workspaces::key(ws.id)).map(|r| [r.x, r.y, r.w, r.h]),
                        }))
                        .collect::<Vec<_>>()),
                    "frame": surface.frame.map(|springs| springs.map(|spring| spring.value)),
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
