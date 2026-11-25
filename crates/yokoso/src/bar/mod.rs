//! The bar: a floating strip on the main screen, its parts at its start, in
//! its middle and at its end. Each part shows what the system says now and
//! changes the frame it hears of a change; at rest it draws nothing.

mod battery;
mod calendar;
pub mod clock;
mod keyboard;
mod media;
mod network;
mod popup;
mod privacy;
mod sound;
mod tray;
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
    mpris, niri, nm, pw, sni, upower,
};

/// What the bar's services send it.
pub enum Message {
    Config(Result<Config, String>),
    Niri(niri::Event),
    Power(upower::Power),
    Net(nm::Net),
    Sound(pw::Heard),
    Tray(sni::Said),
    Media(Option<mpris::Playing>),
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
    /// The volume by wheel steps, up louder; muted or heard again.
    Volume(f32),
    Mute,
    /// To a tray item, by its address.
    TrayActivate(String),
    TraySecondary(String),
    TrayMenu(String),
    TrayScroll(String, f32),
    /// The clock: its calendar; a month on (more) or back; today.
    Calendar,
    CalendarStep(f32),
    CalendarToday,
    /// In a tray menu: a row chosen, a row that opens more, the way back.
    MenuPick(i32),
    MenuOpen(i32),
    MenuBack,
    /// The player shown: played or paused; its next track.
    PlayPause,
    NextTrack,
}

/// Where a bar is: on a screen, or in a window of its own, to try it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    Screen(OutputId),
    Window,
    /// What opens from the bar on a screen: over all of it.
    Popup(OutputId),
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
    sound: pw::Sound,
    /// What records now.
    using: Vec<pw::Use>,
    tray: Vec<sni::Item>,
    /// Where what the tray is asked goes.
    tray_asks: Option<async_channel::Sender<sni::Ask>>,
    media: Option<mpris::Playing>,
    media_asks: Option<async_channel::Sender<mpris::Ask>>,
    /// The panel open now, or closing.
    popup: Option<popup::Popup>,
    /// A tray menu asked for, on this screen: it opens as it comes.
    menu_asked: Option<(OutputId, String)>,
    /// Each screen's connector and logical width, as the surfaces were asked
    /// for: what a bar there shows depends on it.
    screens: RefCell<HashMap<OutputId, (String, f32, f32)>>,
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
            sound: pw::Sound::default(),
            using: Vec::new(),
            tray: Vec::new(),
            tray_asks: None,
            media: None,
            media_asks: None,
            popup: None,
            menu_asked: None,
            screens: RefCell::default(),
        }
    }

    /// Starts its services, and its clock: from now on it wakes only when
    /// what it says changes.
    pub fn start(ui: &mut Ui<Self>, bar: &mut Self) {
        let sender = ui.sender();
        niri::listen(move |event| sender.send(Message::Niri(event)).is_ok());
        let sender = ui.sender();
        upower::listen(move |power| sender.send(Message::Power(power)).is_ok());
        let sender = ui.sender();
        nm::listen(move |net| sender.send(Message::Net(net)).is_ok());
        let sender = ui.sender();
        pw::listen(move |sound| sender.send(Message::Sound(sound)).is_ok());
        let sender = ui.sender();
        bar.tray_asks = Some(sni::listen(move |said| sender.send(Message::Tray(said)).is_ok()));
        let sender = ui.sender();
        bar.media_asks = Some(mpris::listen(move |playing| {
            sender.send(Message::Media(playing)).is_ok()
        }));
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
            Place::Screen(_) | Place::Popup(_) => Rect::new(0.0, 0.0, view.width, view.height),
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
            Place::Screen(id) | Place::Popup(id) => {
                self.screens.borrow().get(&id).map(|(name, w, _)| (name.clone(), *w))
            }
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
                    .find(|(name, _, _)| *name == output)
                    .map_or(1920.0, |(_, width, _)| *width);
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

    /// Passes `ask` on to the tray.
    fn ask_tray(&self, ask: sni::Ask) {
        if let Some(asks) = &self.tray_asks {
            let _ = asks.try_send(ask);
        }
    }

    /// Where the bar on `output` is, on the screen: its own rect.
    fn bar_on_screen(&self, output: OutputId) -> Option<Rect> {
        let (_, width, height) = self.screens.borrow().get(&output).cloned()?;
        let bar = &self.config.bar;
        let y = match bar.position {
            Edge::Top => bar.margin,
            Edge::Bottom => height - bar.margin - bar.height,
        };
        Some(Rect::new(bar.margin, y, width - 2.0 * bar.margin, bar.height))
    }

    /// Where the part `key` of the bar on `output` is, on the screen.
    fn part_on_screen(&self, output: OutputId, key: &str) -> Option<Rect> {
        let bar = self.bar_on_screen(output)?;
        let tree = &self.surfaces.get(&Place::Screen(output))?.tree;
        let rect = match key.strip_prefix("tray:") {
            Some(address) => tree.rect_of(("tray", address)),
            None => tree.rect_of(key),
        }?;
        Some(Rect::new(bar.x + rect.x, bar.y + rect.y, rect.w, rect.h))
    }

    /// The size of what `shows`.
    fn panel_size(shows: &popup::Shows) -> (f32, f32) {
        match shows {
            popup::Shows::Calendar { .. } => (calendar::WIDTH, calendar::HEIGHT),
            popup::Shows::Menu { entries, trail, .. } => popup::menu_size(entries, !trail.is_empty()),
        }
    }

    /// Opens a panel showing `shows` on `output`, grown from its part: in
    /// place of another open one; the same one again closes it.
    fn show(&mut self, ui: &mut Ui<Self>, output: OutputId, shows: popup::Shows) {
        let Some(button) = self.part_on_screen(output, &shows.part()) else {
            return;
        };
        let (Some(bar), Some((_, width, height))) =
            (self.bar_on_screen(output), self.screens.borrow().get(&output).cloned())
        else {
            return;
        };
        let top = self.config.bar.position == Edge::Top;
        let to = popup::place(
            button,
            Self::panel_size(&shows),
            (width, height),
            bar,
            top,
            self.config.bar.margin,
        );
        match &mut self.popup {
            Some(open)
                if open.output == output && open.panel.open && open.shows.same(&shows) && open.shows == shows =>
            {
                self.close(ui);
                return;
            }
            Some(open) if open.output == output => {
                // The same panel showing something else stays where it is;
                // another one goes over to its own part.
                if !open.shows.same(&shows) || !open.panel.open {
                    open.panel.retarget(button, to);
                } else {
                    open.panel.retarget(open.button, to);
                }
                open.button = button;
                open.shows = shows;
                open.stale = true;
            }
            _ => {
                let mut panel = crate::panel::Panel::default();
                panel.toggle(button, to);
                self.popup = Some(popup::Popup {
                    output,
                    shows,
                    panel,
                    button,
                    stale: true,
                    pointer: None,
                    on_bar: false,
                });
                ui.refresh_surfaces();
            }
        }
        ui.dirty(Place::Popup(output));
        self.changed(ui);
    }

    /// Closes the open panel back into its part; its layer goes once it has.
    fn close(&mut self, ui: &mut Ui<Self>) {
        let Some(open) = &mut self.popup else { return };
        if open.panel.open {
            let to = open.panel.content_rect();
            open.panel.toggle(open.button, to);
        }
        let place = Place::Popup(open.output);
        self.generation += 1;
        ui.dirty_all();
        ui.dirty(place);
        let _ = ui.handle().insert_source(
            Timer::from_duration(Duration::from_millis(50)),
            move |_, _, app| match &app.shell.popup {
                Some(open) if !open.panel.open && open.panel.visible() => {
                    TimeoutAction::ToDuration(Duration::from_millis(50))
                }
                Some(open) if !open.panel.open => {
                    app.shell.popup = None;
                    app.ui.refresh_surfaces();
                    TimeoutAction::Drop
                }
                _ => TimeoutAction::Drop,
            },
        );
    }

    /// The part whose panel is open now, by its key.
    fn open_part(&self) -> Option<String> {
        self.popup
            .as_ref()
            .filter(|open| open.panel.open)
            .map(|open| open.shows.part())
    }

    /// The month the calendar shows now, else today's.
    fn calendar_month(&self) -> (i32, u32) {
        use chrono::Datelike;
        match &self.popup {
            Some(popup::Popup {
                shows: popup::Shows::Calendar { shown },
                ..
            }) => *shown,
            _ => {
                let today = clock::now().date_naive();
                (today.year(), today.month())
            }
        }
    }

    /// Does what a part asks, on `place`.
    fn act(&mut self, ui: &mut Ui<Self>, place: Place, msg: &Msg) {
        let output = match place {
            Place::Screen(id) | Place::Popup(id) => Some(id),
            Place::Window => None,
        };
        match msg {
            Msg::Calendar => {
                use chrono::Datelike;
                let today = clock::now().date_naive();
                if let Some(output) = output {
                    let shown = (today.year(), today.month());
                    let showing = self
                        .popup
                        .as_ref()
                        .is_some_and(|open| open.panel.open && matches!(open.shows, popup::Shows::Calendar { .. }));
                    if showing {
                        self.close(ui);
                    } else {
                        self.show(ui, output, popup::Shows::Calendar { shown });
                    }
                }
            }
            Msg::CalendarStep(steps) => {
                let (year, month) = self.calendar_month();
                // Up the wheel, back in time.
                let shown = calendar::step(year, month, -(steps.signum() as i32));
                if let Some(open) = &mut self.popup {
                    open.shows = popup::Shows::Calendar { shown };
                    open.stale = true;
                    ui.dirty(Place::Popup(open.output));
                }
            }
            Msg::CalendarToday => {
                use chrono::Datelike;
                let today = clock::now().date_naive();
                if let Some(open) = &mut self.popup {
                    open.shows = popup::Shows::Calendar {
                        shown: (today.year(), today.month()),
                    };
                    open.stale = true;
                    ui.dirty(Place::Popup(open.output));
                }
            }
            Msg::TrayMenu(address) => {
                if let Some(output) = output {
                    let showing = self.popup.as_ref().is_some_and(|open| {
                        open.panel.open && matches!(&open.shows, popup::Shows::Menu { address: a, .. } if a == address)
                    });
                    if showing {
                        self.close(ui);
                    } else {
                        self.menu_asked = Some((output, address.clone()));
                        self.ask_tray(sni::Ask::Menu(address.clone(), 0));
                    }
                }
            }
            Msg::MenuOpen(id) => {
                if let Some(popup::Popup {
                    output,
                    shows: popup::Shows::Menu {
                        address, parent, trail, ..
                    },
                    ..
                }) = &mut self.popup
                {
                    trail.push(*parent);
                    self.menu_asked = Some((*output, address.clone()));
                    let ask = sni::Ask::Menu(address.clone(), *id);
                    self.ask_tray(ask);
                }
            }
            Msg::MenuBack => {
                if let Some(popup::Popup {
                    output,
                    shows: popup::Shows::Menu { address, trail, .. },
                    ..
                }) = &mut self.popup
                    && let Some(back) = trail.pop()
                {
                    self.menu_asked = Some((*output, address.clone()));
                    let ask = sni::Ask::Menu(address.clone(), back);
                    self.ask_tray(ask);
                }
            }
            Msg::MenuPick(id) => {
                if let Some(popup::Popup {
                    shows: popup::Shows::Menu { address, .. },
                    ..
                }) = &self.popup
                {
                    let ask = sni::Ask::Pick(address.clone(), *id);
                    self.ask_tray(ask);
                }
                self.close(ui);
            }
            _ => self.act_plain(msg),
        }
    }

    /// Input on an open panel's layer: over the bar it is the bar's, over
    /// the panel the panel's; a press anywhere else closes it. Keys are the
    /// panel's, and Escape goes back a level, then closes.
    fn popup_input(&mut self, ui: &mut Ui<Self>, output: OutputId, input: &Input) {
        let layer = Place::Popup(output);
        let beneath = Place::Screen(output);
        let Some(bar) = self.bar_on_screen(output) else { return };
        let Some(open) = &mut self.popup else { return };
        if let Some(at) = popup::at(input) {
            open.pointer = Some(at);
        }
        if matches!(input, Input::PointerLeave) {
            open.pointer = None;
        }
        let at = open.pointer;
        let over_bar = at.is_some_and(|(x, y)| bar.contains(x, y));
        let over_panel = at.is_some_and(|(x, y)| open.panel.open && open.panel.shape().contains(x, y));
        let mut messages = Vec::new();
        if matches!(
            input,
            Input::Key(_) | Input::KeyboardEnter | Input::KeyboardLeave | Input::Paste(_)
        ) {
            let response = open.panel.tree.input(input);
            if response.redraw {
                ui.dirty(layer);
            }
            if response.escaped {
                let deeper = matches!(&open.shows, popup::Shows::Menu { trail, .. } if !trail.is_empty());
                if deeper {
                    messages.push(Msg::MenuBack);
                } else {
                    self.close(ui);
                    return;
                }
            }
            messages.extend(response.messages);
        } else if over_bar {
            if !open.on_bar {
                open.on_bar = true;
                open.panel.tree.input(&Input::PointerLeave);
                ui.dirty(layer);
            }
            let hovering = matches!(input, Input::PointerMove { .. } | Input::PointerEnter { .. });
            if let Some(surface) = self.surfaces.get_mut(&beneath) {
                let response = surface.tree.input(&popup::shifted(input, bar.x, bar.y));
                if response.redraw {
                    ui.dirty(beneath);
                }
                if let Some(wake) = response.wake {
                    ui.wake_after(beneath, wake);
                }
                messages.extend(response.messages);
            }
            // With a panel open, the pointer on another part with one opens
            // that one in its place.
            if hovering
                && self.config.bar.hover_switch
                && let Some((x, y)) = at
            {
                messages.extend(self.switch_to(output, x, y));
            }
        } else {
            if open.on_bar {
                open.on_bar = false;
                if let Some(surface) = self.surfaces.get_mut(&beneath) {
                    surface.tree.input(&Input::PointerLeave);
                    ui.dirty(beneath);
                }
            }
            let Some(open) = &mut self.popup else { return };
            if over_panel || !matches!(input, Input::Press { .. }) {
                let response = open
                    .panel
                    .tree
                    .input(if over_panel { input } else { &Input::PointerLeave });
                if response.redraw {
                    ui.dirty(layer);
                }
                if let Some(wake) = response.wake {
                    ui.wake_after(layer, wake);
                }
                messages.extend(response.messages);
            } else {
                self.close(ui);
                return;
            }
        }
        for message in &messages {
            self.act(ui, layer, message);
        }
    }

    /// With a panel open, what the pointer at `x`, `y` on the bar opens in
    /// its place, if it is on another part with a panel.
    fn switch_to(&self, output: OutputId, x: f32, y: f32) -> Option<Msg> {
        let open = self.popup.as_ref().filter(|open| open.panel.open)?;
        if !matches!(open.shows, popup::Shows::Calendar { .. })
            && self
                .part_on_screen(output, "clock")
                .is_some_and(|rect| rect.contains(x, y))
        {
            return Some(Msg::Calendar);
        }
        let item = self.tray.iter().find(|item| {
            self.part_on_screen(output, &format!("tray:{}", item.address))
                .is_some_and(|rect| rect.contains(x, y))
        })?;
        let showing = matches!(&open.shows, popup::Shows::Menu { address, .. } if *address == item.address);
        let asked = self
            .menu_asked
            .as_ref()
            .is_some_and(|(_, address)| *address == item.address);
        (!showing && !asked && item.menu.is_some()).then(|| Msg::TrayMenu(item.address.clone()))
    }

    /// Does what a part asks that opens nothing of the bar's own.
    fn act_plain(&mut self, msg: &Msg) {
        match msg {
            Msg::Workspace(_) | Msg::Window(_) | Msg::WorkspaceStep(_) => workspaces::act(msg),
            Msg::LayoutStep(steps) => keyboard::act(*steps),
            Msg::Open(module) => self.open(*module),
            Msg::Volume(steps) => {
                let sound = &self.config.bar.sound;
                pw::change_volume(steps * sound.step, sound.max);
            }
            Msg::Mute => pw::toggle_mute(),
            Msg::PlayPause | Msg::NextTrack => {
                if let (Some(player), Some(asks)) = (&self.media, &self.media_asks) {
                    let name = player.name.clone();
                    let ask = if *msg == Msg::PlayPause {
                        mpris::Ask::PlayPause(name)
                    } else {
                        mpris::Ask::Next(name)
                    };
                    let _ = asks.try_send(ask);
                }
            }
            Msg::TrayActivate(address) => self.ask_tray(sni::Ask::Activate(address.clone())),
            Msg::TraySecondary(address) => self.ask_tray(sni::Ask::Secondary(address.clone())),
            Msg::TrayScroll(address, steps) => self.ask_tray(sni::Ask::Scroll(address.clone(), steps.round() as i32)),
            // Opening things of the bar its own goes through `act`.
            _ => {}
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
                    El::press(sizes.gap * 0.75, parts, Some(Msg::Calendar))
                        .selected(self.open_part().as_deref() == Some("clock"))
                        .key("clock")
                        .padding(pad)
                        .height(Size::Fill(1.0)),
                )
            }
            Module::Layout => keyboard::view(&self.niri, pad),
            Module::Battery => battery::view(&self.power, &self.theme, pad, self.opens(module)),
            Module::Media => media::view(self.media.as_ref(), &self.theme, pad),
            Module::Privacy => privacy::view(&self.using, self.niri.casts, &self.theme, pad),
            Module::Tray => tray::view(&self.tray, self.theme.sizes.icon, self.open_part().as_deref()),
            Module::Sound => Some(sound::view(&self.sound, &self.theme, pad, self.opens(module))),
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
            Message::Media(playing) => {
                if playing != self.media {
                    self.media = playing;
                    self.changed(ui);
                }
            }
            Message::Tray(sni::Said::Items(items)) => {
                if items != self.tray {
                    self.tray = items;
                    self.changed(ui);
                }
            }
            Message::Tray(sni::Said::Menu {
                address,
                parent,
                entries,
            }) => {
                // As asked for: the menu opens, or goes a level on or back.
                if let Some((output, asked)) = self.menu_asked.take()
                    && asked == address
                {
                    let trail = match &self.popup {
                        Some(popup::Popup {
                            shows: popup::Shows::Menu { address: a, trail, .. },
                            panel,
                            ..
                        }) if *a == address && panel.open => trail.clone(),
                        _ => Vec::new(),
                    };
                    let trail = if parent == 0 { Vec::new() } else { trail };
                    self.show(
                        ui,
                        output,
                        popup::Shows::Menu {
                            address,
                            parent,
                            entries,
                            trail,
                        },
                    );
                }
            }
            Message::Sound(heard) => {
                if heard.sound != self.sound || heard.using != self.using {
                    self.sound = heard.sound;
                    self.using = heard.using;
                    self.changed(ui);
                }
            }
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
            .map(|output| {
                (
                    output.id,
                    (output.name.clone(), output.size.0 as f32, output.size.1 as f32),
                )
            })
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
        let mut all: Vec<(Place, Spec)> = chosen(outputs, &bar.screens)
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
            .collect();
        // An open panel: over the whole screen, the bar too, and the keys
        // its own while it is there.
        if let Some(open) = &self.popup
            && outputs.iter().any(|output| output.id == open.output)
        {
            let spec = LayerSpec {
                output: open.output,
                layer: Layer::Overlay,
                anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
                size: (0, 0),
                margin: (0, 0, 0, 0),
                exclusive_zone: -1,
                keyboard: KeyboardInteractivity::Exclusive,
                namespace: "yokoso-panel".into(),
            };
            all.push((Place::Popup(open.output), Spec::Layer(spec)));
        }
        all
    }

    fn input(&mut self, ui: &mut Ui<Self>, key: Place, input: &Input) {
        if let Place::Popup(output) = key {
            self.popup_input(ui, output, input);
            return;
        }
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
            self.act(ui, key, message);
        }
    }

    fn update(&mut self, key: Place, time: &FrameTime) -> bool {
        if let Place::Popup(_) = key {
            return match &mut self.popup {
                Some(open) => open.panel.update(time.dt as f32, time.now, &self.theme),
                None => false,
            };
        }
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
        if let Place::Popup(_) = key {
            let content = self
                .popup
                .as_ref()
                .filter(|open| open.stale)
                .map(|open| match &open.shows {
                    popup::Shows::Calendar { shown } => calendar::view(*shown, clock::now().date_naive(), &self.theme),
                    popup::Shows::Menu { entries, trail, .. } => {
                        popup::menu_view(entries, !trail.is_empty(), &self.theme)
                    }
                });
            if let Some(open) = &mut self.popup {
                if let Some(content) = content {
                    open.panel.set(content);
                    open.stale = false;
                }
                open.panel.layout(view, text, &self.theme);
            }
            return;
        }
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
        let theme = &self.theme;
        if let Place::Popup(_) = key {
            if let Some(open) = &self.popup {
                open.panel.render(frame, theme);
            }
            return;
        }
        let view = frame.view();
        let rect = self.rect(key, &view);
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
        if let Place::Popup(_) = key {
            return self.popup.as_ref().and_then(|open| open.panel.wake());
        }
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
                "using": self.using.iter().map(|used| format!("{used:?}")).collect::<Vec<_>>(),
                "media": self.media.as_ref().map(|p| json!({ "title": p.title, "playing": p.playing })),
                "popup": self.popup.as_ref().map(|open| json!({
                    "open": open.panel.open,
                    "visible": open.panel.visible(),
                    "shape": [open.panel.shape().x, open.panel.shape().y, open.panel.shape().w, open.panel.shape().h],
                    "to": [open.panel.content_rect().x, open.panel.content_rect().y, open.panel.content_rect().w, open.panel.content_rect().h],
                    "shows": match &open.shows {
                        popup::Shows::Calendar { shown } => {
                            // The arrows' icons, and the Saturday and Sunday of the first week below them.
                            let tree = &open.panel.tree;
                            let rect = |r: Option<yokoso_ui::geometry::Rect>| r.map(|r| [r.x, r.y, r.w, r.h]);
                            let first = calendar::weeks(shown.0, shown.1)[0].1;
                            let day = |d: chrono::NaiveDate| {
                                use chrono::Datelike;
                                rect(tree.rect_of(("day", d.num_days_from_ce())))
                            };
                            json!({
                                "calendar": [shown.0, shown.1],
                                "arrows": [rect(tree.rect_of(("arrow", "chevron-left"))), rect(tree.rect_of(("arrow", "chevron-right")))],
                                "weekend": [day(first[5]), day(first[6])],
                            })
                        }
                        popup::Shows::Menu { entries, trail, .. } => json!({
                            "menu": entries.iter().map(|e| e.label.clone()).collect::<Vec<_>>(),
                            "depth": trail.len(),
                        }),
                    },
                    "rows": open.panel.tree.nodes().iter().filter(|n| n.role == yokoso_ui::el::tree::Role::Button).map(|n| [n.rect.x, n.rect.y, n.rect.w, n.rect.h]).collect::<Vec<_>>(),
                })),
                "tray": self.tray.iter().map(|item| json!({ "title": item.title, "status": item.status, "icon": item.icon.as_ref().map(|p| [p.width, p.height]) })).collect::<Vec<_>>(),
                "sound": match &self.sound {
                    pw::Sound::Out(out) => json!({
                        "name": out.name,
                        "volume": (f64::from(out.volume) * 100.0).round() / 100.0,
                        "muted": out.muted,
                    }),
                    other => json!(format!("{other:?}")),
                },
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
                    "sound": surface.tree.rect_of("sound").map(|r| [r.x, r.y, r.w, r.h]),
                    "media": surface.tree.rect_of("media").map(|r| [r.x, r.y, r.w, r.h]),
                    "privacy": surface.tree.rect_of("privacy").map(|r| [r.x, r.y, r.w, r.h]),
                    "tray": self.tray.iter().map(|item| surface.tree.rect_of(("tray", item.address.as_str())).map(|r| [r.x, r.y, r.w, r.h])).collect::<Vec<_>>(),
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
