//! The workspaces as a map of their columns: each workspace a pill, each
//! column in it a bar as wide as the column is, each window in a column a
//! piece of it. The focused window in the accent, one that asks for
//! attention in the error color; the active workspace framed (the bar draws
//! the frame, so it can glide from one to the next).

use serde_json::json;
use yokoso_ui::{
    el::{Edges, El, Ink, Size},
    theme::Theme,
};

use super::Msg;
use crate::niri::{self, State};

/// How tall a column is, and how wide it may be.
const TALL: f32 = 16.0;
const NARROW: f32 = 5.0;
const WIDE: f32 = 16.0;
/// A column as wide as the screen is this wide on the map.
const SCALE: f32 = 24.0;
const GAP: f32 = 2.0;

/// The key of the workspace `id`'s pill, to find where it was laid out.
pub fn key(id: u64) -> (&'static str, u64) {
    ("workspace", id)
}

/// The map of the workspaces on `output`, `width` logical pixels wide; None
/// while niri says nothing.
pub fn view(niri: &State, output: &str, width: f32, theme: &Theme) -> Option<El<Msg>> {
    if !niri.there {
        return None;
    }
    let colors = &theme.colors;
    let pills: Vec<El<Msg>> = niri
        .workspaces_on(output)
        .into_iter()
        .map(|workspace| {
            let columns = niri.columns(workspace.id);
            let pieces: Vec<El<Msg>> = if columns.is_empty() {
                // An empty one: a faint slot where a column would be.
                vec![
                    El::block(Ink::Own(colors.outline.alpha(0.45)), 2.0)
                        .width(Size::Fixed(NARROW))
                        .height(Size::Fixed(TALL))
                        .glide(),
                ]
            } else {
                columns
                    .iter()
                    .map(|column| {
                        let wide = column
                            .iter()
                            .map(|window| window.layout.tile_size.0 as f32)
                            .fold(0.0, f32::max);
                        let wide = (wide / width.max(1.0) * SCALE).clamp(NARROW, WIDE).round();
                        let total: f32 = column.iter().map(|w| w.layout.tile_size.1 as f32).sum();
                        let room = TALL - GAP * (column.len() as f32 - 1.0);
                        let tiles = column
                            .iter()
                            .map(|window| {
                                let ink = if window.is_urgent {
                                    Ink::Error
                                } else if window.is_focused {
                                    Ink::Primary
                                } else if workspace.is_active {
                                    Ink::OnSurfaceVariant
                                } else {
                                    Ink::Own(colors.outline)
                                };
                                let tall = (window.layout.tile_size.1 as f32 / total.max(1.0) * room).max(2.0);
                                El::press(
                                    0.0,
                                    vec![
                                        El::block(ink, 2.0)
                                            .width(Size::Fixed(wide))
                                            .height(Size::Fixed(tall))
                                            .glide(),
                                    ],
                                    Some(Msg::Window(window.id)),
                                )
                                .key(("window", window.id))
                            })
                            .collect();
                        El::column(GAP, tiles)
                    })
                    .collect()
            };
            let names: Vec<String> = columns
                .iter()
                .flatten()
                .map(|window| match (&window.app_id, &window.title) {
                    (Some(app), Some(title)) if !title.is_empty() => format!("{app}: {title}"),
                    (Some(app), _) => app.clone(),
                    (None, Some(title)) => title.clone(),
                    (None, None) => "a window".into(),
                })
                .collect();
            let about = if names.is_empty() {
                format!("Workspace {}: empty", workspace.idx)
            } else {
                format!("Workspace {}: {}", workspace.idx, names.join(", "))
            };
            El::press(GAP, pieces, Some(Msg::Workspace(workspace.id)))
                .on_scroll(Msg::WorkspaceStep)
                .key(key(workspace.id))
                .padding(Edges {
                    left: 6.0,
                    right: 6.0,
                    ..Edges::default()
                })
                .height(Size::Fill(1.0))
                .tooltip(about)
        })
        .collect();
    Some(El::row(GAP, pills).height(Size::Fill(1.0)).key("workspaces"))
}

/// What a use of the map asks of niri.
pub fn act(msg: &Msg) {
    match *msg {
        Msg::Workspace(id) => niri::act(json!({ "FocusWorkspace": { "reference": { "Id": id } } })),
        Msg::Window(id) => niri::act(json!({ "FocusWindow": { "id": id } })),
        // Up the wheel, up the workspaces.
        Msg::WorkspaceStep(steps) if steps > 0.0 => niri::act(json!({ "FocusWorkspaceUp": {} })),
        Msg::WorkspaceStep(steps) if steps < 0.0 => niri::act(json!({ "FocusWorkspaceDown": {} })),
        _ => {}
    }
}
