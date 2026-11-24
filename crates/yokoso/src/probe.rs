//! The probe: a card on every output, to see that the ground works before
//! anything is built on it.

use yokoso_ui::{
    Color, Rect,
    app::{Anchor, Frame, FrameTime, KeyboardInteractivity, Layer, LayerSpec, Output, OutputId, Shell, Spec},
};

#[derive(Default)]
pub struct Probe {
    /// Seconds the square still has to move, once asked to.
    moving_for: Option<f64>,
    /// Per output: when its square started, and where it is, 0 to 1.
    squares: std::collections::HashMap<OutputId, (Option<f64>, f32)>,
    /// Outputs whose last update moved only the square.
    moved: std::collections::HashSet<OutputId>,
}

/// How long one way along the track takes, seconds.
const SWEEP: f64 = 1.0;

const WIDTH: f32 = 420.0;
const HEIGHT: f32 = 260.0;
const MARGIN: f32 = 24.0;

impl Shell for Probe {
    type Key = OutputId;
    type Message = ();

    fn surfaces(&self, outputs: &[Output]) -> Vec<(OutputId, Spec)> {
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
                    keyboard: KeyboardInteractivity::None,
                    namespace: "yokoso-probe".into(),
                };
                (output.id, Spec::Layer(spec))
            })
            .collect()
    }

    fn update(&mut self, key: OutputId, time: &FrameTime) -> bool {
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

    fn damage(&mut self, key: OutputId) -> Option<Vec<Rect>> {
        self.moved.remove(&key).then(|| vec![track()])
    }

    fn render(&self, key: OutputId, frame: &mut Frame) {
        let card = Rect::new(MARGIN, MARGIN, WIDTH, HEIGHT);
        frame.shadow(card.grow(-2.0), 8.0, 16.0, Color::BLACK.alpha(0.45));
        frame.fill(card, 8.0, Color::hex("#1d1b20").unwrap_or(Color::BLACK));
        frame.ring(card, 8.0, 1.0, Color::WHITE.alpha(0.12));
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
        let accent = Color::hex("#f3a17a").unwrap_or(Color::WHITE);
        frame.fill(Rect::new(card.x + 160.0, card.y + 16.0, 120.0, 40.0), 20.0, accent);
        frame.ring(Rect::new(card.x + 160.0, card.y + 72.0, 120.0, 40.0), 8.0, 2.0, accent);
        frame.fill(
            Rect::new(card.x + 300.0, card.y + 16.0, 96.0, 96.0),
            48.0,
            accent.alpha(0.6),
        );
        // The track and its square.
        let track = track();
        frame.fill(track, 16.0, Color::WHITE.alpha(0.08));
        let at = self.squares.get(&key).map_or(0.0, |(_, at)| *at);
        let square = Rect::new(track.x + 4.0 + at * (track.w - 32.0), track.y + 4.0, 24.0, 24.0);
        frame.fill(square, 6.0, accent);
    }
}

/// Where the square runs.
fn track() -> Rect {
    Rect::new(MARGIN + 16.0, MARGIN + 200.0, WIDTH - 32.0, 32.0)
}

impl crate::ipc::Answer for Probe {
    fn answer(&mut self, ui: &mut yokoso_ui::app::Ui<Self>, words: &[String]) -> Result<serde_json::Value, String> {
        use serde_json::json;
        match words.first().map(String::as_str) {
            Some("ping") => Ok(json!("pong")),
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
