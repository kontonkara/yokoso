//! The probe: a card on every output, to see that the ground works before
//! anything is built on it.

use yokoso_ui::{
    Color, Rect,
    app::{
        Anchor, Frame, FrameTime, KeyboardInteractivity, Layer, LayerSpec, Output, OutputId, Shell, Spec, Ui, View,
        WindowSpec,
    },
    input::{Input, KeyState, Keysym, ScrollSource},
    text::{Paragraph, Style, Text},
};

/// What the probe's services send it.
pub enum Message {
    Config(Result<crate::config::Config, String>),
}

/// Where a probe is: on an output, or in a window of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    Output(OutputId),
    Window,
}

#[derive(Default)]
pub struct Probe {
    /// In a window, instead of on every output.
    window: bool,
    /// Each place's last view, from layout.
    views: std::collections::HashMap<Place, View>,
    /// Each place's lines of text, laid out at its scale.
    lines: std::collections::HashMap<Place, Vec<Paragraph>>,
    /// Seconds the square still has to move, once asked to.
    moving_for: Option<f64>,
    /// Per output: when its square started, and where it is, 0 to 1.
    squares: std::collections::HashMap<Place, (Option<f64>, f32)>,
    /// Outputs whose last update moved only the square.
    moved: std::collections::HashSet<Place>,
    /// What was typed into the field.
    typed: String,
    config: crate::config::Config,
    /// Why the config on disk is not the one in force, if it isn't.
    config_error: Option<String>,
    /// The debug overlay as `msg overlay` set it; None follows the config.
    overlay: Option<bool>,
    /// Each place's overlay: what it says, and that laid out.
    overlays: std::collections::HashMap<Place, (String, Option<Paragraph>)>,
    /// The last thing the pointer or keyboard did, said in words.
    heard: String,
    /// The last scroll, kept apart: what comes after must not hide it.
    scrolled: String,
    /// Each place's field and what it heard, laid out; dropped when they change.
    field: std::collections::HashMap<Place, (Paragraph, Paragraph)>,
}

/// How long one way along the track takes, seconds.
const SWEEP: f64 = 1.0;

const WIDTH: f32 = 420.0;
const HEIGHT: f32 = 380.0;
const MARGIN: f32 = 24.0;

impl Shell for Probe {
    type Key = Place;
    type Message = Message;

    fn message(&mut self, ui: &mut Ui<Self>, message: Message) {
        match message {
            Message::Config(loaded) => self.apply(ui, loaded),
        }
    }

    fn surfaces(&self, outputs: &[Output]) -> Vec<(Place, Spec)> {
        if self.window {
            let spec = WindowSpec {
                title: "yokoso probe".into(),
                app_id: "yokoso".into(),
                size: (WIDTH as u32, HEIGHT as u32),
            };
            return vec![(Place::Window, Spec::Window(spec))];
        }
        outputs
            .iter()
            .map(|output| {
                let spec = LayerSpec {
                    output: output.id,
                    layer: Layer::Overlay,
                    anchor: Anchor::BOTTOM | Anchor::RIGHT,
                    size: ((WIDTH + 2.0 * MARGIN) as u32, (HEIGHT + 2.0 * MARGIN) as u32),
                    margin: (0, 0, 0, 0),
                    exclusive_zone: 0,
                    // The keyboard when clicked, for the field.
                    keyboard: KeyboardInteractivity::OnDemand,
                    namespace: "yokoso-probe".into(),
                };
                (Place::Output(output.id), Spec::Layer(spec))
            })
            .collect()
    }

    fn update(&mut self, key: Place, time: &FrameTime) -> bool {
        if self.overlay_on() {
            let says = format!(
                "{:.0} fps \u{b7} {} \u{b7} {:.2} ms \u{b7} {} frames",
                time.fps,
                if time.refresh > 0.0 {
                    format!("{:.0} Hz", time.refresh)
                } else {
                    "? Hz".into()
                },
                time.cpu * 1000.0,
                time.frames
            );
            let entry = self.overlays.entry(key).or_insert((String::new(), None));
            if entry.0 != says {
                *entry = (says, None);
            }
        }
        let Some(duration) = self.moving_for else {
            return false;
        };
        self.moved.insert(key);
        let (started, at) = self.squares.entry(key).or_insert((None, 0.0));
        // Motion starts at its first frame, never at the moment it was asked.
        let start = *started.get_or_insert(time.now);
        let elapsed = time.now - start;
        if elapsed >= duration {
            *started = None;
            *at = 0.0;
            if self.squares.values().all(|(started, _)| started.is_none()) {
                self.moving_for = None;
            }
            return false;
        }
        // There and back, at an even speed: any judder shows.
        let phase = (elapsed / SWEEP) % 2.0;
        *at = if phase < 1.0 { phase } else { 2.0 - phase } as f32;
        true
    }

    fn layout(&mut self, key: Place, view: &View, text: &mut Text) {
        let fresh = self.views.get(&key).is_none_or(|old| old.scale != view.scale);
        self.views.insert(key, *view);
        if fresh || !self.lines.contains_key(&key) {
            self.lines.insert(key, lines(text, view.scale));
            self.field.remove(&key);
        }
        if fresh {
            self.overlays.remove(&key);
        }
        if let Some((says, laid @ None)) = self.overlays.get_mut(&key) {
            let style = Style {
                size: 11.0,
                tabular: true,
                ..Style::default()
            };
            *laid = Some(text.layout(says, &style, view.scale, None, false));
        }
        if !self.field.contains_key(&key) {
            let style = Style {
                size: 13.0,
                ..Style::default()
            };
            let typed = if self.typed.is_empty() {
                "Click and type"
            } else {
                &self.typed
            };
            let heard = if self.heard.is_empty() {
                "No input yet"
            } else {
                &self.heard
            };
            let width = Some(WIDTH - 56.0);
            let field = (
                text.layout(typed, &style, view.scale, width, true),
                text.layout(heard, &style, view.scale, width, true),
            );
            self.field.insert(key, field);
        }
    }

    fn input(&mut self, ui: &mut Ui<Self>, key: Place, input: &Input) {
        tracing::debug!(?input, "input");
        let typed = self.typed.len();
        let heard = match input {
            Input::PointerEnter { x, y } | Input::PointerMove { x, y } => format!("pointer at {x:.0}, {y:.0}"),
            Input::PointerLeave => "pointer left".into(),
            Input::Press { button, .. } => format!("pressed {button:#x}"),
            Input::Release { button, .. } => format!("released {button:#x}"),
            Input::Scroll(scroll) => match scroll.source {
                ScrollSource::Wheel | ScrollSource::WheelTilt => {
                    format!("wheel: {} {} steps", scroll.steps_x, scroll.steps_y)
                }
                ScrollSource::Finger => format!(
                    "fingers: {:.1} {:.1} px{}",
                    scroll.x,
                    scroll.y,
                    if scroll.stop { ", lifted" } else { "" }
                ),
                ScrollSource::Continuous => format!("continuous: {:.1} {:.1} px", scroll.x, scroll.y),
            },
            Input::KeyboardEnter => "keyboard here".into(),
            Input::KeyboardLeave => "keyboard gone".into(),
            Input::Key(k) => {
                if k.state != KeyState::Released {
                    match (k.keysym, &k.text) {
                        (Keysym::BackSpace, _) => {
                            self.typed.pop();
                        }
                        (Keysym::Escape, _) => self.typed.clear(),
                        (_, Some(text)) => self.typed.push_str(text),
                        _ => {}
                    }
                }
                format!("{:?} {:?}{}", k.keysym, k.state, if k.mods.ctrl { " ctrl" } else { "" })
            }
        };
        if matches!(input, Input::Scroll(_)) {
            self.scrolled = heard.clone();
        }
        if heard != self.heard || self.typed.len() != typed {
            self.heard = heard;
            self.field.clear();
            ui.dirty(key);
        }
    }

    fn damage(&mut self, key: Place) -> Option<Vec<Rect>> {
        let card = self.card(key);
        let overlay = self.overlay_on().then(|| self.overlay_rect(key));
        self.moved
            .remove(&key)
            .then(|| [Some(track(card)), overlay].into_iter().flatten().collect())
    }

    fn render(&self, key: Place, frame: &mut Frame) {
        let card = self.card(key);
        let background = Color::hex("#1d1b20").unwrap_or(Color::BLACK);
        let radius = self.config.probe.radius;
        if key == Place::Window {
            frame.fill(card, 0.0, background);
        } else {
            frame.shadow(card.grow(-2.0), radius, 16.0, Color::BLACK.alpha(0.45));
            frame.fill(card, radius, background);
            frame.ring(card, radius, 1.0, Color::WHITE.alpha(0.12));
        }
        // Sharpness: one-pixel lines a pixel apart, and a checker.
        let inner = Rect::new(card.x + 16.0, card.y + 16.0, 120.0, 120.0);
        for step in 0..30 {
            let x = inner.x + step as f32 * 2.0;
            frame.fill(Rect::new(x, inner.y, 1.0, 56.0), 0.0, Color::WHITE);
        }
        for row in 0..8 {
            for col in 0..8 {
                if (row + col) % 2 == 0 {
                    let cell = Rect::new(inner.x + col as f32 * 7.0, inner.y + 64.0 + row as f32 * 7.0, 7.0, 7.0);
                    frame.fill(cell, 0.0, Color::WHITE);
                }
            }
        }
        // Shapes: a pill, a ring, a circle.
        let accent = self.config.probe.accent;
        frame.fill(Rect::new(card.x + 160.0, card.y + 16.0, 120.0, 40.0), 20.0, accent);
        frame.ring(Rect::new(card.x + 160.0, card.y + 72.0, 120.0, 40.0), 8.0, 2.0, accent);
        frame.fill(
            Rect::new(card.x + 300.0, card.y + 16.0, 96.0, 96.0),
            48.0,
            accent.alpha(0.6),
        );
        // Text: a title, scripts and emoji, digits, and a line cut short.
        let mut y = card.y + 148.0;
        for (index, line) in self.lines.get(&key).into_iter().flatten().enumerate() {
            let color = if index == 0 {
                Color::WHITE
            } else {
                Color::WHITE.alpha(0.75)
            };
            frame.text(line, card.x + 16.0, y, color);
            y += line.height() + 4.0;
        }
        // The field: what was typed, and what the pointer and keyboard did.
        let field = Rect::new(card.x + 16.0, card.bottom() - 104.0, card.w - 32.0, 48.0);
        frame.fill(field, 8.0, Color::WHITE.alpha(0.06));
        frame.ring(field, 8.0, 1.0, Color::WHITE.alpha(0.2));
        if let Some((typed, heard)) = self.field.get(&key) {
            frame.text(typed, field.x + 12.0, field.y + 6.0, Color::WHITE);
            frame.text(heard, field.x + 12.0, field.y + 26.0, Color::WHITE.alpha(0.6));
        }
        // The track and its square.
        let track = track(card);
        frame.fill(track, 16.0, Color::WHITE.alpha(0.08));
        let at = self.squares.get(&key).map_or(0.0, |(_, at)| *at);
        let square = Rect::new(track.x + 4.0 + at * (track.w - 32.0), track.y + 4.0, 24.0, 24.0);
        frame.fill(square, 6.0, accent);
        // The debug overlay, over everything.
        if self.overlay_on()
            && let Some((_, Some(says))) = self.overlays.get(&key)
        {
            let at = self.overlay_rect(key);
            frame.fill(at, at.h / 2.0, Color::BLACK.alpha(0.7));
            frame.text(says, at.x + 8.0, at.y + (at.h - says.height()) / 2.0, Color::WHITE);
        }
    }
}

impl Probe {
    pub fn new(window: bool, config: crate::config::Config) -> Self {
        Self {
            window,
            config,
            ..Self::default()
        }
    }

    fn overlay_on(&self) -> bool {
        self.overlay.unwrap_or(self.config.debug.overlay)
    }

    /// Where the overlay goes: in the layer's margin above the card, or on a
    /// window's top left.
    fn overlay_rect(&self, key: Place) -> Rect {
        let card = self.card(key);
        match key {
            Place::Output(_) => Rect::new(card.x, 2.0, 300.0, 20.0),
            Place::Window => Rect::new(card.x + 8.0, card.y + 8.0, 300.0, 20.0),
        }
    }

    /// A config read anew: in force if it read, else the old one stays.
    fn apply(&mut self, ui: &mut Ui<Self>, loaded: Result<crate::config::Config, String>) {
        match loaded {
            Ok(config) => {
                self.config_error = None;
                if config != self.config {
                    tracing::info!("config changed");
                    self.config = config;
                    ui.dirty_all();
                }
            }
            Err(error) => {
                tracing::warn!(%error, "config kept as it was");
                self.config_error = Some(error);
            }
        }
    }

    /// The card: in from the layer's edges, or all of a window.
    fn card(&self, key: Place) -> Rect {
        match (key, self.views.get(&key)) {
            (Place::Window, Some(view)) => Rect::new(0.0, 0.0, view.width, view.height),
            (Place::Window, None) => Rect::new(0.0, 0.0, WIDTH, HEIGHT),
            (Place::Output(_), _) => Rect::new(MARGIN, MARGIN, WIDTH, HEIGHT),
        }
    }
}

/// Where the square runs, in `card`.
fn track(card: Rect) -> Rect {
    Rect::new(card.x + 16.0, card.bottom() - 48.0, card.w - 32.0, 32.0)
}

/// The probe's lines of text at `scale`.
fn lines(text: &mut Text, scale: f32) -> Vec<Paragraph> {
    let body = Style {
        size: 13.0,
        ..Style::default()
    };
    let title = Style {
        size: 15.0,
        weight: 600.0,
        ..Style::default()
    };
    let digits = Style {
        tabular: true,
        ..body.clone()
    };
    vec![
        text.layout(&format!("yokoso probe \u{b7} {scale}\u{d7}"), &title, scale, None, false),
        text.layout("Latin, \u{41a}\u{438}\u{440}\u{438}\u{43b}\u{43b}\u{438}\u{446}\u{430}, \u{451}\u{401}, emoji \u{1f525}\u{1f389}", &body, scale, None, false),
        text.layout("0123456789 \u{b7} 11:11 \u{b7} 00:00 \u{b7} 100%", &digits, scale, None, false),
        text.layout(
            "A line far too long for its place, so it has to end in an ellipsis well before its very end",
            &body,
            scale,
            Some(WIDTH - 32.0),
            true,
        ),
    ]
}

impl crate::ipc::Answer for Probe {
    fn answer(&mut self, ui: &mut yokoso_ui::app::Ui<Self>, words: &[String]) -> Result<serde_json::Value, String> {
        use serde_json::json;
        match words.first().map(String::as_str) {
            Some("ping") => Ok(json!("pong")),
            Some("overlay") => {
                let on = match words.get(1).map(String::as_str) {
                    Some("on") => true,
                    Some("off") => false,
                    None => !self.overlay_on(),
                    Some(other) => return Err(format!("on or off, not {other}")),
                };
                self.overlay = Some(on);
                ui.dirty_all();
                Ok(json!(on))
            }
            Some("reload") => {
                self.apply(ui, crate::config::load(&crate::config::dir()));
                match &self.config_error {
                    Some(error) => Err(error.clone()),
                    None => Ok(json!(null)),
                }
            }
            Some("move") => {
                let seconds = match words.get(1) {
                    Some(text) => text.parse::<f64>().map_err(|_| format!("not seconds: {text}"))?,
                    None => 4.0,
                };
                self.moving_for = Some(seconds.clamp(0.0, 600.0));
                for (started, _) in self.squares.values_mut() {
                    *started = None;
                }
                ui.dirty_all();
                Ok(json!(null))
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
                "typed": self.typed,
                "config_error": self.config_error,
                "accent": format!("{:?}", self.config.probe.accent.0),
                "heard": self.heard,
                "scrolled": self.scrolled,
                "outputs": ui.outputs().iter().map(|o| json!({
                    "name": o.name, "description": o.description, "size": o.size,
                })).collect::<Vec<_>>(),
                "surfaces": ui.surface_info().iter().map(|s| json!({
                    "key": s.key, "size": s.size, "scale": s.scale, "frames": s.frames,
                    "waiting": s.waiting, "moving": s.moving, "refresh": s.refresh, "fps": s.fps,
                })).collect::<Vec<_>>(),
            })),
            Some(other) => Err(format!("no such command: {other}")),
            None => Err("no command".into()),
        }
    }
}
