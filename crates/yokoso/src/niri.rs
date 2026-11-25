//! niri: what it says of its workspaces, windows and keyboard layouts, from
//! its event stream, kept as one picture; and the actions the bar asks of
//! it. Only what the bar shows is read; events it doesn't know are passed
//! over, so a newer niri never breaks it.

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    time::Duration,
};

use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Workspace {
    pub id: u64,
    pub idx: u32,
    pub name: Option<String>,
    pub output: Option<String>,
    pub is_urgent: bool,
    pub is_active: bool,
    pub is_focused: bool,
    pub active_window_id: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Window {
    pub id: u64,
    pub title: Option<String>,
    pub app_id: Option<String>,
    pub workspace_id: Option<u64>,
    pub is_focused: bool,
    pub is_floating: bool,
    pub is_urgent: bool,
    pub layout: Layout,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Layout {
    /// Its column and its place in the column, from 1; none when floating.
    pub pos_in_scrolling_layout: Option<(u32, u32)>,
    pub tile_size: (f64, f64),
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct KeyboardLayouts {
    pub names: Vec<String>,
    pub current_idx: usize,
}

/// One change, as niri tells it.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Workspaces(Vec<Workspace>),
    WorkspaceActivated {
        id: u64,
        focused: bool,
    },
    WorkspaceActiveWindow {
        workspace: u64,
        window: Option<u64>,
    },
    WorkspaceUrgency {
        id: u64,
        urgent: bool,
    },
    Windows(Vec<Window>),
    WindowOpenedOrChanged(Window),
    WindowClosed(u64),
    WindowFocus(Option<u64>),
    WindowUrgency {
        id: u64,
        urgent: bool,
    },
    WindowLayouts(Vec<(u64, Layout)>),
    KeyboardLayouts(KeyboardLayouts),
    KeyboardLayoutSwitched(usize),
    /// The stream is gone, or never came: niri is not there now.
    Lost,
}

/// One line of the event stream; None for an event the bar has no use for.
pub fn parse(line: &str) -> Option<Event> {
    let value: Value = serde_json::from_str(line).ok()?;
    let (name, body) = value.as_object()?.iter().next()?;
    fn field<T: serde::de::DeserializeOwned>(body: &Value, key: &str) -> Option<T> {
        serde_json::from_value(body.get(key).cloned().unwrap_or(Value::Null)).ok()
    }
    Some(match name.as_str() {
        "WorkspacesChanged" => Event::Workspaces(field(body, "workspaces")?),
        "WorkspaceActivated" => Event::WorkspaceActivated {
            id: field(body, "id")?,
            focused: field(body, "focused").unwrap_or(false),
        },
        "WorkspaceActiveWindowChanged" => Event::WorkspaceActiveWindow {
            workspace: field(body, "workspace_id")?,
            window: field(body, "active_window_id").flatten(),
        },
        "WorkspaceUrgencyChanged" => Event::WorkspaceUrgency {
            id: field(body, "id")?,
            urgent: field(body, "urgent")?,
        },
        "WindowsChanged" => Event::Windows(field(body, "windows")?),
        "WindowOpenedOrChanged" => Event::WindowOpenedOrChanged(field(body, "window")?),
        "WindowClosed" => Event::WindowClosed(field(body, "id")?),
        "WindowFocusChanged" => Event::WindowFocus(field(body, "id").flatten()),
        "WindowUrgencyChanged" => Event::WindowUrgency {
            id: field(body, "id")?,
            urgent: field(body, "urgent")?,
        },
        "WindowLayoutsChanged" => Event::WindowLayouts(field(body, "changes")?),
        "KeyboardLayoutsChanged" => Event::KeyboardLayouts(field(body, "keyboard_layouts")?),
        "KeyboardLayoutSwitched" => Event::KeyboardLayoutSwitched(field(body, "idx")?),
        _ => return None,
    })
}

/// All niri has said so far.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    /// niri answers: its stream is open.
    pub there: bool,
    pub workspaces: HashMap<u64, Workspace>,
    pub windows: HashMap<u64, Window>,
    pub layouts: KeyboardLayouts,
}

impl State {
    /// Takes `event` in. True when anything changed.
    pub fn apply(&mut self, event: Event) -> bool {
        let before = self.clone();
        self.there = true;
        match event {
            Event::Workspaces(all) => {
                self.workspaces = all.into_iter().map(|ws| (ws.id, ws)).collect();
            }
            Event::WorkspaceActivated { id, focused } => {
                let output = self.workspaces.get(&id).and_then(|ws| ws.output.clone());
                for ws in self.workspaces.values_mut() {
                    if ws.output == output {
                        ws.is_active = ws.id == id;
                    }
                    if focused {
                        ws.is_focused = ws.id == id;
                    }
                }
            }
            Event::WorkspaceActiveWindow { workspace, window } => {
                if let Some(ws) = self.workspaces.get_mut(&workspace) {
                    ws.active_window_id = window;
                }
            }
            Event::WorkspaceUrgency { id, urgent } => {
                if let Some(ws) = self.workspaces.get_mut(&id) {
                    ws.is_urgent = urgent;
                }
            }
            Event::Windows(all) => {
                self.windows = all.into_iter().map(|window| (window.id, window)).collect();
            }
            Event::WindowOpenedOrChanged(window) => {
                if window.is_focused {
                    for other in self.windows.values_mut() {
                        other.is_focused = false;
                    }
                }
                self.windows.insert(window.id, window);
            }
            Event::WindowClosed(id) => {
                self.windows.remove(&id);
            }
            Event::WindowFocus(id) => {
                for window in self.windows.values_mut() {
                    window.is_focused = Some(window.id) == id;
                }
            }
            Event::WindowUrgency { id, urgent } => {
                if let Some(window) = self.windows.get_mut(&id) {
                    window.is_urgent = urgent;
                }
            }
            Event::WindowLayouts(changes) => {
                for (id, layout) in changes {
                    if let Some(window) = self.windows.get_mut(&id) {
                        window.layout = layout;
                    }
                }
            }
            Event::KeyboardLayouts(layouts) => self.layouts = layouts,
            Event::KeyboardLayoutSwitched(idx) => self.layouts.current_idx = idx,
            Event::Lost => *self = Self::default(),
        }
        *self != before
    }

    /// The workspaces on `output`, in niri's order.
    pub fn workspaces_on(&self, output: &str) -> Vec<&Workspace> {
        let mut on: Vec<&Workspace> = self
            .workspaces
            .values()
            .filter(|ws| ws.output.as_deref() == Some(output))
            .collect();
        on.sort_by_key(|ws| ws.idx);
        on
    }

    /// The columns of the workspace `id`, left to right: each its windows,
    /// top to bottom. Floating windows are in none.
    pub fn columns(&self, id: u64) -> Vec<Vec<&Window>> {
        let mut tiled: Vec<&Window> = self
            .windows
            .values()
            .filter(|window| window.workspace_id == Some(id) && !window.is_floating)
            .filter(|window| window.layout.pos_in_scrolling_layout.is_some())
            .collect();
        tiled.sort_by_key(|window| window.layout.pos_in_scrolling_layout);
        let mut columns: Vec<Vec<&Window>> = Vec::new();
        let mut last = None;
        for window in tiled {
            let column = window.layout.pos_in_scrolling_layout.map(|(column, _)| column);
            if column != last || columns.is_empty() {
                columns.push(Vec::new());
                last = column;
            }
            columns.last_mut().expect("pushed").push(window);
        }
        columns
    }
}

/// Where niri listens, if it runs.
fn socket() -> Option<String> {
    std::env::var("NIRI_SOCKET").ok().filter(|path| !path.is_empty())
}

/// Listens to niri on a thread of its own, sending each event it knows to
/// `send`. Lost, it says so, and tries again a while later: niri restarted
/// sends everything anew.
pub fn listen(send: impl Fn(Event) -> bool + Send + 'static) {
    let spawned = std::thread::Builder::new().name("niri".into()).spawn(move || {
        let mut wait = Duration::from_millis(250);
        loop {
            if let Some(path) = socket()
                && let Ok(mut stream) = UnixStream::connect(&path)
                && writeln!(stream, "\"EventStream\"").is_ok()
            {
                wait = Duration::from_millis(250);
                let mut lines = BufReader::new(stream).lines();
                // The first line answers the request; events come after.
                let _ = lines.next();
                for line in lines {
                    let Ok(line) = line else { break };
                    if let Some(event) = parse(&line)
                        && !send(event)
                    {
                        return;
                    }
                }
            }
            if !send(Event::Lost) {
                return;
            }
            std::thread::sleep(wait);
            wait = (wait * 2).min(Duration::from_secs(5));
        }
    });
    if let Err(error) = spawned {
        tracing::warn!(%error, "no thread for niri");
    }
}

/// Asks niri to do `action`, as its IPC spells it, on a thread of its own:
/// the interface never waits on it.
pub fn act(action: Value) {
    let request = serde_json::json!({ "Action": action });
    let _ = std::thread::Builder::new().name("niri-action".into()).spawn(move || {
        let Some(path) = socket() else { return };
        let done = UnixStream::connect(&path).and_then(|mut stream| {
            stream.set_read_timeout(Some(Duration::from_secs(2)))?;
            writeln!(stream, "{request}")?;
            let mut reply = String::new();
            BufReader::new(stream).read_line(&mut reply)?;
            Ok(reply)
        });
        match done {
            Ok(reply) if reply.contains("\"Err\"") => tracing::warn!(%reply, "niri refused"),
            Ok(_) => {}
            Err(error) => tracing::warn!(%error, "niri not reached"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fed(lines: &[&str]) -> State {
        let mut state = State::default();
        for line in lines {
            if let Some(event) = parse(line) {
                state.apply(event);
            }
        }
        state
    }

    const WORKSPACES: &str = r#"{"WorkspacesChanged":{"workspaces":[
        {"id":8,"idx":2,"name":null,"output":"eDP-1","is_urgent":false,"is_active":false,"is_focused":false,"active_window_id":31},
        {"id":2,"idx":1,"name":null,"output":"eDP-1","is_urgent":false,"is_active":true,"is_focused":true,"active_window_id":4},
        {"id":5,"idx":1,"name":null,"output":"HDMI-A-1","is_urgent":false,"is_active":true,"is_focused":false,"active_window_id":null}]}}"#;

    fn window(id: u64, workspace: u64, column: u32, tile: u32, focused: bool) -> String {
        format!(
            r#"{{"id":{id},"title":"w{id}","app_id":"kitty","pid":1,"workspace_id":{workspace},"is_focused":{focused},"is_floating":false,"is_urgent":false,"layout":{{"pos_in_scrolling_layout":[{column},{tile}],"tile_size":[800.0,600.0],"window_size":[800,600],"tile_pos_in_workspace_view":null,"window_offset_in_tile":[0.0,0.0]}},"focus_timestamp":null}}"#
        )
    }

    #[test]
    fn workspaces_come_in_order_per_screen_and_one_is_active_on_each() {
        let mut state = fed(&[WORKSPACES]);
        let ids = |state: &State, output| state.workspaces_on(output).iter().map(|ws| ws.id).collect::<Vec<_>>();
        assert_eq!(ids(&state, "eDP-1"), [2, 8]);
        assert_eq!(ids(&state, "HDMI-A-1"), [5]);
        // Another becomes active on its screen: the one before isn't, and
        // the other screen's stays.
        state.apply(parse(r#"{"WorkspaceActivated":{"id":8,"focused":true}}"#).unwrap());
        assert!(state.workspaces[&8].is_active && state.workspaces[&8].is_focused);
        assert!(!state.workspaces[&2].is_active && !state.workspaces[&2].is_focused);
        assert!(state.workspaces[&5].is_active);
    }

    #[test]
    fn windows_make_columns_and_follow_focus_moves_and_closing() {
        let windows = format!(
            r#"{{"WindowsChanged":{{"windows":[{},{},{},{}]}}}}"#,
            window(1, 2, 1, 1, true),
            window(2, 2, 2, 1, false),
            window(3, 2, 2, 2, false),
            window(4, 8, 1, 1, false)
        );
        let mut state = fed(&[WORKSPACES, &windows]);
        let columns = |state: &State| {
            state
                .columns(2)
                .iter()
                .map(|column| column.iter().map(|w| w.id).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        };
        assert_eq!(columns(&state), [vec![1], vec![2, 3]]);
        // Focus moves; a window opens focused; one closes; one moves over.
        state.apply(parse(r#"{"WindowFocusChanged":{"id":3}}"#).unwrap());
        assert!(state.windows[&3].is_focused && !state.windows[&1].is_focused);
        let opened = format!(
            r#"{{"WindowOpenedOrChanged":{{"window":{}}}}}"#,
            window(5, 2, 3, 1, true)
        );
        state.apply(parse(&opened).unwrap());
        assert!(!state.windows[&3].is_focused);
        state.apply(parse(r#"{"WindowClosed":{"id":1}}"#).unwrap());
        let moved = r#"{"WindowLayoutsChanged":{"changes":[[2,{"pos_in_scrolling_layout":[3,2],"tile_size":[400.0,600.0],"window_size":[400,600],"tile_pos_in_workspace_view":null,"window_offset_in_tile":[0.0,0.0]}]]}}"#;
        state.apply(parse(moved).unwrap());
        assert_eq!(columns(&state), [vec![3], vec![5, 2]]);
    }

    #[test]
    fn layouts_switch_and_unknown_events_pass_by() {
        let mut state = fed(&[
            r#"{"KeyboardLayoutsChanged":{"keyboard_layouts":{"names":["English (US)","Russian"],"current_idx":0}}}"#,
        ]);
        assert!(state.apply(parse(r#"{"KeyboardLayoutSwitched":{"idx":1}}"#).unwrap()));
        assert_eq!(state.layouts.current_idx, 1);
        assert_eq!(parse(r#"{"OverviewOpenedOrClosed":{"is_open":true}}"#), None);
        assert_eq!(parse(r#"{"SomethingNew":{"x":1}}"#), None);
        assert_eq!(parse("not json"), None);
        // Lost: nothing is known any more.
        state.apply(Event::Lost);
        assert!(state.layouts.names.is_empty() && !state.there);
    }
}
