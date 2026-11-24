//! The probe: a card on every output, to see that the ground works before
//! anything is built on it.

use yokoso_ui::{
    Color, Rect,
    app::{Anchor, Frame, KeyboardInteractivity, Layer, LayerSpec, Output, OutputId, Shell, Spec},
};

pub struct Probe;

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

    fn render(&self, _: OutputId, frame: &mut Frame) {
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
    }
}

impl crate::ipc::Answer for Probe {
    fn answer(&mut self, ui: &mut yokoso_ui::app::Ui<Self>, words: &[String]) -> Result<serde_json::Value, String> {
        use serde_json::json;
        match words.first().map(String::as_str) {
            Some("ping") => Ok(json!("pong")),
            Some("stats") => {
                let stats = ui.stats();
                Ok(json!({
                    "wakeups": stats.wakeups,
                    "frames": stats.frames,
                    "last_frame_ms": stats.last_frame * 1000.0,
                }))
            }
            Some("dump") => Ok(json!({
                "outputs": ui.outputs().iter().map(|o| json!({
                    "name": o.name, "description": o.description, "size": o.size,
                })).collect::<Vec<_>>(),
                "surfaces": ui.surface_info().iter().map(|s| json!({
                    "key": s.key, "size": s.size, "scale": s.scale, "frames": s.frames,
                    "waiting": s.waiting, "moving": s.moving,
                })).collect::<Vec<_>>(),
            })),
            Some(other) => Err(format!("no such command: {other}")),
            None => Err("no command".into()),
        }
    }
}
