//! The demo: every element on a page of its own, and the switches for theme,
//! palette, motion and scale, to try them by hand before anything is built
//! of them. In a window, or as a layer over the desktop.

use std::time::Duration;

use serde_json::{Value, json};
use yokoso_ui::{
    Rect,
    app::{
        Anchor, Frame, FrameTime, KeyboardInteractivity, Layer, LayerSpec, Output, OutputId, Shell, Spec, Ui, View,
        WindowSpec,
    },
    calloop::timer::{TimeoutAction, Timer},
    el::{Align, ButtonStyle, CardStyle, Edges, El, Ink, Size, TileEl, Type, tree::Tree},
    input::Input,
    motion::{Mode, Motion, Preset},
    text::Text,
    theme::{Density, Palette, Theme, Variant},
};

/// Where the demo is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    Window,
    Layer(OutputId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Buttons,
    Toggles,
    Sliders,
    Cards,
    Lists,
    Fields,
    Settings,
    Panel,
}

impl Page {
    const ALL: [Page; 8] = [
        Page::Buttons,
        Page::Toggles,
        Page::Sliders,
        Page::Cards,
        Page::Lists,
        Page::Fields,
        Page::Settings,
        Page::Panel,
    ];

    fn name(self) -> &'static str {
        match self {
            Page::Buttons => "Buttons",
            Page::Toggles => "Toggles",
            Page::Sliders => "Sliders",
            Page::Cards => "Cards and tiles",
            Page::Lists => "Lists",
            Page::Fields => "Fields",
            Page::Settings => "Theme and motion",
            Page::Panel => "Panel",
        }
    }
}

/// What its elements tell it.
#[derive(Clone, Debug)]
pub enum Msg {
    Page(Page),
    Pressed(ButtonStyle),
    /// The quick switch: it changes at once.
    Quick,
    /// The slow one: as a system setting, it waits for an answer.
    Slow,
    Volume(f32),
    Brightness(f32),
    Fine(f32),
    /// A tile pressed, by its place in the grid.
    Tile(usize),
    /// A tile's way into its details.
    More(usize),
    Select(usize),
    Open(usize),
    /// The list with its rows, or with none.
    Empty,
    Name(String),
    Secret(String),
    Sent,
    Dark(usize),
    Variant(usize),
    Accent(usize),
    Density(usize),
    Preset(usize),
    Mode(usize),
    Speed(f32),
    Zoom(usize),
    /// The panel opened, or closed.
    Panel,
}

/// A rect as `[x, y, w, h]`, for dumps.
fn rect(r: Rect) -> [f32; 4] {
    [r.x, r.y, r.w, r.h]
}

/// Milliseconds of the wall clock.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as u64)
}

/// The time of day, UTC, as hh:mm:ss: its digits roll.
fn clock() -> String {
    let seconds = now_ms() / 1000 % 86_400;
    format!("{:02}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60)
}

/// Accents to try, by name and seed.
const ACCENTS: [(&str, &str); 6] = [
    ("Peach", "#f3a17a"),
    ("Rose", "#f06292"),
    ("Violet", "#9575cd"),
    ("Blue", "#64b5f6"),
    ("Teal", "#4db6ac"),
    ("Green", "#81c784"),
];
const DENSITIES: [Density; 3] = [Density::Compact, Density::Normal, Density::Comfortable];
const MODES: [Mode; 3] = [Mode::Normal, Mode::Reduced, Mode::Off];
const ZOOMS: [f32; 5] = [0.75, 1.0, 1.25, 1.5, 2.0];

/// The demo's tiles: icon, name, and whether they wait on a slow system.
const TILES: [(&str, &str, bool); 4] = [
    ("wifi", "Wi-Fi", true),
    ("bluetooth", "Bluetooth", true),
    ("bell", "Do not disturb", false),
    ("moon", "Night light", false),
];

pub struct Demo {
    layer: bool,
    theme: Theme,
    tree: Tree<Msg>,
    /// The description is to be built anew before the next layout.
    stale: bool,
    page: Page,
    /// Presses on the buttons page, and of which style the last one was.
    presses: u32,
    last: Option<ButtonStyle>,
    quick: bool,
    slow: bool,
    /// The slow switch was asked to change and waits for its answer.
    slow_pending: bool,
    volume: f32,
    brightness: f32,
    fine: f32,
    tiles: [bool; 4],
    tiles_pending: [bool; 4],
    /// Whose details were asked for last.
    details: Option<usize>,
    selected: Option<usize>,
    opened: Option<usize>,
    empty: bool,
    name: String,
    secret: String,
    sent: Option<String>,
    /// What the theme is made of, as chosen on the settings page.
    accent: usize,
    zoom: usize,
    preset: Option<Preset>,
    panel: crate::panel::Panel<Msg>,
    /// Times the panel was opened, for the memory test.
    panels: u32,
    /// A clock ticks on the panel page.
    ticking: bool,
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
            page: Page::Buttons,
            presses: 0,
            last: None,
            quick: true,
            slow: false,
            slow_pending: false,
            volume: 40.0,
            brightness: 70.0,
            fine: 0.5,
            tiles: [true, false, false, true],
            tiles_pending: [false; 4],
            details: None,
            selected: None,
            opened: None,
            empty: false,
            name: String::new(),
            secret: String::new(),
            sent: None,
            accent: 0,
            zoom: 1,
            preset: Some(Preset::Expressive),
            panel: crate::panel::Panel::default(),
            panels: 0,
            ticking: false,
        }
    }

    /// What the elements said, done.
    fn apply(&mut self, ui: &mut Ui<Self>, message: Msg) {
        match message {
            Msg::Page(page) => {
                // Its button stays on its page: the panel goes with it.
                if page != self.page && self.panel.visible() {
                    self.panel.hide();
                }
                self.page = page;
                if page == Page::Panel && !self.ticking {
                    self.ticking = true;
                    let tick = Timer::from_duration(Duration::from_millis(1000 - (now_ms() % 1000)));
                    let _ = ui.handle().insert_source(tick, |_, _, app| {
                        if app.shell.page != Page::Panel {
                            app.shell.ticking = false;
                            return TimeoutAction::Drop;
                        }
                        app.shell.stale = true;
                        app.ui.dirty_all();
                        TimeoutAction::ToDuration(Duration::from_millis(1000 - (now_ms() % 1000)))
                    });
                }
            }
            Msg::Pressed(style) => {
                self.presses += 1;
                self.last = Some(style);
            }
            Msg::Quick => self.quick = !self.quick,
            Msg::Volume(value) => self.volume = value,
            Msg::Brightness(value) => self.brightness = value,
            Msg::Fine(value) => self.fine = value,
            Msg::More(at) => self.details = Some(at),
            Msg::Select(row) => self.selected = Some(row),
            Msg::Open(row) => self.opened = Some(row),
            Msg::Empty => self.empty = !self.empty,
            Msg::Name(text) => self.name = text,
            Msg::Secret(text) => self.secret = text,
            Msg::Sent => self.sent = Some(self.name.clone()),
            Msg::Dark(at) => self.retheme(|palette, _, _| palette.dark = at == 0),
            Msg::Variant(at) => self.retheme(|palette, _, _| palette.variant = Variant::ALL[at]),
            Msg::Accent(at) => {
                self.accent = at;
                let seed = yokoso_ui::Color::hex(ACCENTS[at].1).unwrap_or(yokoso_ui::Color::WHITE);
                self.retheme(|palette, _, _| palette.seed = seed);
            }
            Msg::Density(at) => self.retheme(|_, density, _| *density = DENSITIES[at]),
            Msg::Preset(at) => {
                self.preset = Some(Preset::ALL[at]);
                self.retheme(|_, _, motion| motion.global = Preset::ALL[at].params());
            }
            Msg::Mode(at) => self.retheme(|_, _, motion| motion.mode = MODES[at]),
            Msg::Speed(speed) => self.retheme(|_, _, motion| motion.speed = speed),
            Msg::Panel => self.toggle_panel(),
            Msg::Zoom(at) => {
                self.zoom = at;
                ui.set_zoom(ZOOMS[at]);
            }
            Msg::Tile(at) if TILES[at].2 => {
                self.tiles_pending[at] = true;
                let answer = Timer::from_duration(Duration::from_millis(1200));
                let _ = ui.handle().insert_source(answer, move |_, _, app| {
                    app.shell.tiles[at] = !app.shell.tiles[at];
                    app.shell.tiles_pending[at] = false;
                    app.shell.stale = true;
                    app.ui.dirty_all();
                    TimeoutAction::Drop
                });
            }
            Msg::Tile(at) => self.tiles[at] = !self.tiles[at],
            Msg::Slow => {
                // As a slow system would: the answer comes a while later.
                self.slow_pending = true;
                let answer = Timer::from_duration(Duration::from_millis(1200));
                let _ = ui.handle().insert_source(answer, |_, _, app| {
                    app.shell.slow = !app.shell.slow;
                    app.shell.slow_pending = false;
                    app.shell.stale = true;
                    app.ui.dirty_all();
                    TimeoutAction::Drop
                });
            }
        }
        self.stale = true;
    }

    /// The panel opens from its button, or closes back into it, from
    /// wherever it is.
    fn toggle_panel(&mut self) {
        let Some(button) = self.tree.rect_of("panel-button") else {
            return;
        };
        let to = Rect::new(button.x, button.bottom() + 8.0, 360.0, 260.0);
        self.panel.toggle(button, to);
        if self.panel.open {
            self.panels += 1;
        }
    }

    /// The panel page: a button the panel grows from.
    fn panel_page(&self) -> El<Msg> {
        let sizes = &self.theme.sizes;
        El::column(
            sizes.gap,
            vec![
                El::text("Panel", Type::Title).weight(600.0),
                El::text(
                    "It grows out of its button; its content shows once it has grown. Click again mid-way: it turns where it is.",
                    Type::Body,
                )
                .ink(Ink::OnSurfaceVariant)
                .width(Size::Fixed(420.0)),
                El::text(clock(), Type::Headline).weight(500.0).tabular().rolling(),
                El::button(
                    if self.panel.open { "Close" } else { "Quick settings" },
                    ButtonStyle::Tonal,
                    Some(Msg::Panel),
                )
                .key("panel-button"),
            ],
        )
        .align(Align::Start)
    }

    /// What the panel holds.
    fn panel_content(&self) -> El<Msg> {
        let sizes = &self.theme.sizes;
        let tile = |at: usize| {
            let (icon, title, _) = TILES[at];
            El::tile(TileEl {
                icon,
                title: title.into(),
                subtitle: if self.tiles[at] { "On" } else { "Off" }.into(),
                on: self.tiles[at],
                pending: false,
                on_press: Some(Msg::Tile(at)),
                on_more: None,
            })
            .width(Size::Fill(1.0))
        };
        El::column(
            sizes.gap,
            vec![
                El::text("Quick settings", Type::Title).weight(600.0),
                El::row(sizes.gap, vec![tile(2), tile(3)]),
                El::slider(self.volume, 0.0, 100.0, 1.0, Some(Msg::Volume))
                    .show_value()
                    .width(Size::Fill(1.0)),
            ],
        )
        .padding(Edges::all(sizes.pad))
        .fill()
    }

    /// The theme made anew with `change` to its palette, density or motion.
    fn retheme(&mut self, change: impl FnOnce(&mut Palette, &mut Density, &mut Motion)) {
        let (mut palette, mut density, mut motion) =
            (self.theme.palette, self.theme.density, self.theme.motion.clone());
        change(&mut palette, &mut density, &mut motion);
        self.theme = Theme::new(palette, density, motion);
    }

    /// The theme and motion page: everything the theme is made of, changed
    /// on the fly.
    fn settings(&self) -> El<Msg> {
        let sizes = &self.theme.sizes;
        let names = |all: &[&str]| all.iter().map(|name| name.to_string()).collect::<Vec<_>>();
        let line = |name: &str, control: El<Msg>| {
            El::row(
                sizes.gap,
                vec![
                    El::text(name, Type::Body).width(Size::Fixed(130.0)),
                    control.width(Size::Fixed(240.0)),
                ],
            )
        };
        let theme = &self.theme;
        let motion = &theme.motion;
        El::column(
            sizes.gap,
            vec![
                El::text("Theme and motion", Type::Title).weight(600.0),
                line(
                    "Theme",
                    El::menu(
                        names(&["Dark", "Light"]),
                        Some(if theme.palette.dark { 0 } else { 1 }),
                        "",
                        Some(Msg::Dark),
                    ),
                ),
                line(
                    "Palette",
                    El::menu(
                        Variant::ALL.iter().map(|variant| variant.name().to_string()).collect(),
                        Variant::ALL
                            .iter()
                            .position(|variant| *variant == theme.palette.variant),
                        "",
                        Some(Msg::Variant),
                    ),
                ),
                line(
                    "Accent",
                    El::menu(
                        ACCENTS.iter().map(|(name, _)| name.to_string()).collect(),
                        Some(self.accent),
                        "",
                        Some(Msg::Accent),
                    ),
                ),
                line(
                    "Density",
                    El::menu(
                        names(&["Compact", "Normal", "Comfortable"]),
                        DENSITIES.iter().position(|density| *density == theme.density),
                        "",
                        Some(Msg::Density),
                    ),
                ),
                line(
                    "Spring",
                    El::menu(
                        Preset::ALL.iter().map(|preset| preset.name().to_string()).collect(),
                        self.preset
                            .and_then(|preset| Preset::ALL.iter().position(|p| *p == preset)),
                        "",
                        Some(Msg::Preset),
                    ),
                ),
                line(
                    "Motion",
                    El::menu(
                        names(&["Normal", "Reduced", "Off"]),
                        MODES.iter().position(|mode| *mode == motion.mode),
                        "",
                        Some(Msg::Mode),
                    ),
                ),
                line(
                    "Speed",
                    El::slider(motion.speed, 0.5, 2.0, 0.05, Some(Msg::Speed)).show_value(),
                ),
                line(
                    "Zoom",
                    El::menu(
                        ZOOMS.iter().map(|zoom| format!("{zoom}\u{d7}")).collect(),
                        Some(self.zoom),
                        "",
                        Some(Msg::Zoom),
                    ),
                ),
            ],
        )
        .align(Align::Start)
    }

    /// The fields page: plain, a password, and disabled.
    fn fields(&self) -> El<Msg> {
        let sizes = &self.theme.sizes;
        let line = |name: &str, field: El<Msg>| {
            El::column(
                4.0,
                vec![
                    El::text(name, Type::Label).ink(Ink::OnSurfaceVariant),
                    field.width(Size::Fixed(380.0)),
                ],
            )
        };
        let said = match &self.sent {
            Some(name) => format!("Sent: {name}"),
            None => "Return sends the first; Ctrl+arrows go by words".into(),
        };
        El::column(
            sizes.gap * 1.5,
            vec![
                El::text("Fields", Type::Title).weight(600.0),
                line(
                    "Name",
                    El::field(&self.name, "Type here", Some(Msg::Name)).on_submit(Msg::Sent),
                ),
                line(
                    "Password",
                    El::field(&self.secret, "Dots, and nothing to copy", Some(Msg::Secret)).password(),
                ),
                line("Disabled", El::field("Takes nothing", "", None::<fn(String) -> Msg>)),
                El::text(said, Type::Body).ink(Ink::OnSurfaceVariant),
            ],
        )
        .align(Align::Start)
    }

    /// The lists page: ten thousand rows, or none.
    fn lists(&self) -> El<Msg> {
        let sizes = &self.theme.sizes;
        let len = if self.empty { 0 } else { 10_000 };
        let list = El::list(len, sizes.control + 8.0, |at| {
            El::row(
                12.0,
                vec![
                    El::icon(if at % 3 == 0 { "bell" } else { "check" }),
                    El::text(format!("Row {}", at + 1), Type::Body).width(Size::Fill(1.0)),
                    El::text(format!("{:05}", (at * 7919) % 100_000), Type::Small)
                        .ink(Ink::OnSurfaceVariant)
                        .tabular(),
                ],
            )
            .padding(Edges::xy(12.0, 0.0))
        })
        .choosing(self.selected, Msg::Select, Msg::Open)
        .empty("Nothing here")
        .fill();
        let said = match (self.selected, self.opened) {
            (_, Some(row)) => format!("Opened row {}", row + 1),
            (Some(row), None) => format!("Chosen row {}; Return opens it", row + 1),
            (None, None) => "Scroll, click a row, or Tab here and use the arrows".into(),
        };
        El::column(
            sizes.gap,
            vec![
                El::row(
                    sizes.gap,
                    vec![
                        El::text("Lists", Type::Title).weight(600.0),
                        El::button(
                            if self.empty { "Fill it" } else { "Empty it" },
                            ButtonStyle::Outlined,
                            Some(Msg::Empty),
                        ),
                    ],
                ),
                El::text(said, Type::Body).ink(Ink::OnSurfaceVariant),
                El::card(CardStyle::Filled, None, 0.0, vec![list])
                    .padding(Edges::all(4.0))
                    .fill(),
            ],
        )
        .align(Align::Start)
        .height(Size::Fill(1.0))
    }

    /// The cards and tiles page: a card of each style, and a grid of tiles.
    fn cards(&self) -> El<Msg> {
        let sizes = &self.theme.sizes;
        let card = |style, name: &str| {
            El::card(
                style,
                Some(name),
                6.0,
                vec![El::text("Background, corners, a title.", Type::Small).ink(Ink::OnSurfaceVariant)],
            )
            .padding(Edges::all(sizes.pad))
            .width(Size::Fill(1.0))
        };
        let tile = |at: usize| {
            let (icon, title, slow) = TILES[at];
            let subtitle = match (self.tiles_pending[at], self.tiles[at]) {
                (true, true) => "Turning off",
                (true, false) => "Turning on",
                (false, true) => "On",
                (false, false) => "Off",
            };
            El::tile(TileEl {
                icon,
                title: title.into(),
                subtitle: subtitle.into(),
                on: self.tiles[at],
                pending: self.tiles_pending[at],
                on_press: Some(Msg::Tile(at)),
                on_more: slow.then_some(Msg::More(at)),
            })
            .width(Size::Fill(1.0))
        };
        let said = match self.details {
            Some(at) => format!("Details of {} asked for", TILES[at].1),
            None => "Click the arrow of Wi-Fi or Bluetooth, or Shift+Return on it".into(),
        };
        El::column(
            sizes.gap,
            vec![
                El::text("Cards", Type::Title).weight(600.0),
                El::row(
                    sizes.gap,
                    vec![
                        card(CardStyle::Filled, "Filled"),
                        card(CardStyle::Elevated, "Elevated"),
                        card(CardStyle::Outlined, "Outlined"),
                    ],
                )
                .align(Align::Stretch),
                El::text("Tiles", Type::Title).weight(600.0),
                El::row(sizes.gap, vec![tile(0), tile(1)]),
                El::row(sizes.gap, vec![tile(2), tile(3)]),
                El::row(
                    sizes.gap,
                    vec![
                        El::tile(TileEl {
                            icon: "power",
                            title: "Disabled".into(),
                            subtitle: "Takes nothing".into(),
                            on: false,
                            pending: false,
                            on_press: None,
                            on_more: None,
                        })
                        .width(Size::Fill(1.0)),
                        El::spacer().width(Size::Fill(1.0)).height(Size::Fixed(1.0)),
                    ],
                ),
                El::text(said, Type::Body).ink(Ink::OnSurfaceVariant),
            ],
        )
        .width(Size::Fixed(560.0))
    }

    /// The sliders page: by whole numbers, by fives, fine, and disabled.
    fn sliders(&self) -> El<Msg> {
        let gap = self.theme.sizes.gap;
        let line = |name: &str, slider: El<Msg>| {
            El::column(
                4.0,
                vec![
                    El::text(name, Type::Label).ink(Ink::OnSurfaceVariant),
                    slider.width(Size::Fixed(420.0)),
                ],
            )
        };
        El::column(
            gap * 1.5,
            vec![
                El::text("Sliders", Type::Title).weight(600.0),
                line(
                    "Volume, by one",
                    El::slider(self.volume, 0.0, 100.0, 1.0, Some(Msg::Volume)).show_value(),
                ),
                line(
                    "Brightness, by five",
                    El::slider(self.brightness, 0.0, 100.0, 5.0, Some(Msg::Brightness)).show_value(),
                ),
                line(
                    "Fine, 0 to 1",
                    El::slider(self.fine, 0.0, 1.0, 0.01, Some(Msg::Fine)).show_value(),
                ),
                line("Disabled", El::slider(30.0, 0.0, 100.0, 1.0, None::<fn(f32) -> Msg>)),
            ],
        )
        .align(Align::Start)
    }

    /// The toggles page: quick, slow and disabled.
    fn toggles(&self) -> El<Msg> {
        let gap = self.theme.sizes.gap;
        let line = |name: &str, about: &str, toggle: El<Msg>| {
            El::row(
                gap,
                vec![
                    El::column(
                        2.0,
                        vec![
                            El::text(name, Type::Body),
                            El::text(about, Type::Small).ink(Ink::OnSurfaceVariant),
                        ],
                    )
                    .width(Size::Fill(1.0)),
                    toggle,
                ],
            )
            .width(Size::Fixed(420.0))
        };
        El::column(
            gap * 1.5,
            vec![
                El::text("Toggles", Type::Title).weight(600.0),
                line(
                    "Quick",
                    "Changes at once",
                    El::toggle(self.quick, false, Some(Msg::Quick)),
                ),
                line(
                    "Slow",
                    "Waits on the system, pulsing meanwhile",
                    El::toggle(self.slow, self.slow_pending, Some(Msg::Slow)),
                ),
                line("Disabled", "Takes nothing", El::toggle(true, false, None)),
            ],
        )
        .align(Align::Start)
    }

    /// The buttons page: each style, enabled and not.
    fn buttons(&self) -> El<Msg> {
        let gap = self.theme.sizes.gap;
        let styles = [
            (ButtonStyle::Filled, "Filled", "The one main action"),
            (ButtonStyle::Tonal, "Tonal", "An action of weight, not the main one"),
            (ButtonStyle::Outlined, "Outlined", "One of a few alike"),
            (ButtonStyle::Text, "Text", "The least of them"),
        ];
        let row = |enabled: bool| {
            El::row(
                gap,
                styles
                    .iter()
                    .map(|(style, name, about)| {
                        let button = El::button(*name, *style, enabled.then_some(Msg::Pressed(*style)));
                        if enabled { button.tooltip(*about) } else { button }
                    })
                    .collect(),
            )
        };
        let said = match self.last {
            Some(style) => format!("Pressed {} times, last: {style:?}", self.presses),
            None => "Not pressed yet".into(),
        };
        El::column(
            gap,
            vec![
                El::text("Buttons", Type::Title).weight(600.0),
                row(true),
                row(false),
                El::text(said, Type::Body).ink(Ink::OnSurfaceVariant).tabular(),
            ],
        )
        .align(Align::Start)
    }

    /// The description of everything shown, from the state: the pages on
    /// the left, the one chosen on the right.
    fn view(&self) -> El<Msg> {
        let sizes = &self.theme.sizes;
        let nav = El::column(
            4.0,
            Page::ALL
                .iter()
                .map(|page| {
                    let style = if *page == self.page {
                        ButtonStyle::Tonal
                    } else {
                        ButtonStyle::Text
                    };
                    El::button(page.name(), style, Some(Msg::Page(*page))).width(Size::Fill(1.0))
                })
                .collect(),
        )
        .width(Size::Fixed(180.0));
        let page = match self.page {
            Page::Buttons => self.buttons(),
            Page::Toggles => self.toggles(),
            Page::Sliders => self.sliders(),
            Page::Cards => self.cards(),
            Page::Lists => self.lists(),
            Page::Fields => self.fields(),
            Page::Settings => self.settings(),
            Page::Panel => self.panel_page(),
        };
        El::column(
            sizes.gap,
            vec![
                El::text("yokoso elements", Type::Headline).weight(600.0),
                El::text("Every element of the shell on a page of its own.", Type::Body).ink(Ink::OnSurfaceVariant),
                El::row(sizes.pad * 2.0, vec![nav, page.width(Size::Fill(1.0))])
                    .align(Align::Start)
                    .fill(),
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
        // The panel takes what lands on it while it is open.
        let on_panel = self.panel.open
            && match input {
                Input::PointerMove { x, y } | Input::Press { x, y, .. } | Input::Release { x, y, .. } => {
                    self.panel.content_rect().contains(*x, *y)
                }
                _ => false,
            };
        let response = if on_panel {
            self.panel.tree.input(input)
        } else {
            self.tree.input(input)
        };
        if response.escaped && self.panel.open {
            self.toggle_panel();
            self.stale = true;
        } else if response.escaped && self.layer {
            // Nothing left to unwind: a layer has no close button, Escape is it.
            ui.quit();
        }
        for message in response.messages {
            self.apply(ui, message);
        }
        if response.redraw || self.stale {
            ui.dirty(key);
        }
        if let Some(wake) = response.wake {
            ui.wake_after(key, wake);
        }
        if let Some(text) = response.copy {
            ui.copy(text);
        }
        if response.paste {
            ui.paste(key);
        }
    }

    fn wake(&self, _: Place) -> Option<std::time::Duration> {
        self.tree.wake()
    }

    fn update(&mut self, _: Place, time: &FrameTime) -> bool {
        let panel = self.panel.update(time.dt as f32, time.now, &self.theme);
        self.tree.update(time.dt as f32, time.now, &self.theme) | panel
    }

    fn layout(&mut self, _: Place, view: &View, text: &mut Text) {
        if self.stale {
            self.tree.set(self.view());
            self.panel.set(self.panel_content());
            self.stale = false;
        }
        if self.tree.needs_layout(view) {
            self.tree.layout(view, text, &self.theme);
        }
        self.panel.layout(view, text, &self.theme);
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
        self.panel.render(frame, &self.theme);
    }
}

impl crate::ipc::Answer for Demo {
    fn answer(&mut self, ui: &mut Ui<Self>, words: &[String]) -> Result<Value, String> {
        match words.first().map(String::as_str) {
            Some("ping") => Ok(json!("pong")),
            Some("panel") if words.get(1).map(String::as_str) == Some("at") => {
                let at: f32 = words.get(2).and_then(|at| at.parse().ok()).ok_or("at what, 0 to 1?")?;
                let button = self.tree.rect_of("panel-button").ok_or("not on the panel page")?;
                let to = Rect::new(button.x, button.bottom() + 8.0, 360.0, 260.0);
                self.panel.freeze(button, to, at.clamp(0.0, 1.0));
                self.stale = true;
                ui.dirty_all();
                Ok(json!(null))
            }
            Some("panel") => {
                self.apply(ui, Msg::Panel);
                ui.dirty_all();
                Ok(json!(self.panel.open))
            }
            Some("page") => {
                let name = words.get(1).ok_or("which page?")?;
                let page = Page::ALL
                    .into_iter()
                    .find(|page| page.name().to_lowercase().starts_with(&name.to_lowercase()))
                    .ok_or_else(|| format!("no page {name}"))?;
                self.apply(ui, Msg::Page(page));
                ui.dirty_all();
                Ok(json!(null))
            }
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
                "presses": self.presses,
                "tooltips": self.tree.tooltips_shown(),
                "page": self.page.name(),
                "quick": self.quick,
                "slow": self.slow,
                "slow_pending": self.slow_pending,
                "volume": self.volume,
                "brightness": self.brightness,
                "fine": self.fine,
                "tiles": self.tiles,
                "tiles_pending": self.tiles_pending,
                "details": self.details,
                "selected": self.selected,
                "scroll": self.tree.nodes().iter().find(|node| node.role == yokoso_ui::el::tree::Role::List)
                    .and_then(|node| self.tree.state(node.id)).map(|state| state.scroll.value),
                "opened": self.opened,
                "name": self.name,
                "secret_length": self.secret.chars().count(),
                "sent": self.sent,
                "dark": self.theme.palette.dark,
                "accent": ACCENTS[self.accent].0,
                "density": format!("{:?}", self.theme.density),
                "zoom": ui.zoom(),
                "mode": format!("{:?}", self.theme.motion.mode),
                "speed": self.theme.motion.speed,
                "preset": self.preset.map(Preset::name),
                "panel_open": self.panel.open,
                "panel_visible": self.panel.visible(),
                "panels": self.panels,
                "panel_shape": rect(self.panel.shape()),
                "panel_content": rect(self.panel.content_rect()),
            })),
            Some(other) => Err(format!("no such command: {other}")),
            None => Err("no command".into()),
        }
    }
}
