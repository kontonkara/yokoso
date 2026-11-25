//! Elements: what the shell describes a surface with. A description is
//! light and built anew when the shell's state changes; what an element
//! keeps between rebuilds lives in the tree, by its id.

pub mod kinds;
pub mod layout;
pub mod tree;

use crate::geometry::Color;

/// How much room an element takes along one axis.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Size {
    /// As much as its content needs.
    #[default]
    Hug,
    /// A share of what is left, by weight.
    Fill(f32),
    Fixed(f32),
}

/// Where children sit across a row's or a column's main axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    /// As wide (in a column) or as tall (in a row) as their parent.
    Stretch,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edges {
    pub const fn all(by: f32) -> Self {
        Self {
            top: by,
            right: by,
            bottom: by,
            left: by,
        }
    }

    pub const fn xy(x: f32, y: f32) -> Self {
        Self {
            top: y,
            right: x,
            bottom: y,
            left: x,
        }
    }

    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

/// Which text style, from the theme's type tokens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Type {
    #[default]
    Body,
    Label,
    Small,
    Title,
    Headline,
}

/// What color text takes, from the theme's roles.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Ink {
    #[default]
    OnSurface,
    OnSurfaceVariant,
    Primary,
    Error,
    Own(Color),
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextEl {
    pub text: String,
    pub ty: Type,
    pub ink: Ink,
    pub weight: f32,
    /// One line, cut with an ellipsis where it doesn't fit.
    pub ellipsis: bool,
    pub tabular: bool,
}

/// How a card stands out from what is under it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CardStyle {
    #[default]
    Filled,
    /// Raised on a shadow.
    Elevated,
    Outlined,
}

/// How a button looks, from most to least weight.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonStyle {
    /// The one main action.
    Filled,
    #[default]
    Tonal,
    Outlined,
    Text,
}

pub struct ListEl<M> {
    pub len: usize,
    pub row_height: f32,
    /// Builds the row at an index.
    pub row: std::rc::Rc<dyn Fn(usize) -> El<M>>,
    pub selected: Option<usize>,
    /// A row chosen: by a click, or by the arrows.
    pub on_select: Option<std::rc::Rc<dyn Fn(usize) -> M>>,
    /// The chosen row used: Return.
    pub on_activate: Option<std::rc::Rc<dyn Fn(usize) -> M>>,
    /// What it says when it has no rows.
    pub empty: String,
    /// Set by the tree before layout: the first row among its children, and
    /// how far it is scrolled, pixels.
    pub first: usize,
    pub scroll: f32,
}

pub struct TileEl<M> {
    pub icon: &'static str,
    pub title: String,
    /// Its state in words: "On", a network's name.
    pub subtitle: String,
    pub on: bool,
    /// Asked to change, not yet confirmed.
    pub pending: bool,
    /// Its main action, on a click or Return; disabled without one.
    pub on_press: Option<M>,
    /// Into its details, by the arrow at its end or Shift+Return.
    pub on_more: Option<M>,
}

pub struct SliderEl<M> {
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    /// Its value written beside it.
    pub show_value: bool,
    pub on_change: Option<std::rc::Rc<dyn Fn(f32) -> M>>,
}

impl<M> SliderEl<M> {
    /// `value` kept in range, on a step.
    pub fn fit(&self, value: f32) -> f32 {
        let value = value.clamp(self.min, self.max);
        if self.step > 0.0 {
            (self.min + ((value - self.min) / self.step).round() * self.step).clamp(self.min, self.max)
        } else {
            value
        }
    }

    /// Where `value` is along it, 0 to 1.
    pub fn fraction(&self, value: f32) -> f32 {
        if self.max > self.min {
            ((value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

/// What an element is.
pub enum Kind<M> {
    Row {
        gap: f32,
        align: Align,
    },
    Column {
        gap: f32,
        align: Align,
    },
    /// Children over each other, the last on top.
    Stack,
    /// A column on a background of its own.
    Card {
        style: CardStyle,
        gap: f32,
    },
    /// An icon of the shell's own set, by name.
    Icon {
        name: &'static str,
        ink: Ink,
    },
    /// Its one child, with a few words that show when the pointer rests on
    /// it a while.
    Tooltip {
        text: String,
    },
    /// Rows of one height, as many as there are, of which only those in view
    /// are ever built.
    List(ListEl<M>),
    /// Room and nothing else.
    Spacer,
    Text(TextEl),
    /// Disabled without a message to send.
    Button {
        label: String,
        style: ButtonStyle,
        on_press: Option<M>,
    },
    /// A switch. Pending: asked to change, not yet confirmed, as a system
    /// setting is until the system answers. Disabled without a message.
    Toggle {
        on: bool,
        pending: bool,
        on_toggle: Option<M>,
    },
    /// A value between `min` and `max`, in `step`s. Disabled without a
    /// message to send.
    Slider(SliderEl<M>),
    /// A control center's tile: a setting with its state, and a way into
    /// its details.
    Tile(TileEl<M>),
    #[doc(hidden)]
    _Message(std::marker::PhantomData<M>),
}

/// An element as the shell describes it.
pub struct El<M> {
    pub key: Option<u64>,
    pub kind: Kind<M>,
    pub width: Size,
    pub height: Size,
    pub padding: Edges,
    pub children: Vec<El<M>>,
}

impl<M> El<M> {
    pub fn new(kind: Kind<M>) -> Self {
        Self {
            key: None,
            kind,
            width: Size::Hug,
            height: Size::Hug,
            padding: Edges::default(),
            children: Vec::new(),
        }
    }

    pub fn row(gap: f32, children: Vec<El<M>>) -> Self {
        Self::new(Kind::Row {
            gap,
            align: Align::Center,
        })
        .with(children)
    }

    pub fn column(gap: f32, children: Vec<El<M>>) -> Self {
        Self::new(Kind::Column {
            gap,
            align: Align::Stretch,
        })
        .with(children)
    }

    pub fn stack(children: Vec<El<M>>) -> Self {
        Self::new(Kind::Stack).with(children)
    }

    pub fn spacer() -> Self {
        Self::new(Kind::Spacer).width(Size::Fill(1.0)).height(Size::Fill(1.0))
    }

    pub fn text(text: impl Into<String>, ty: Type) -> Self {
        Self::new(Kind::Text(TextEl {
            text: text.into(),
            ty,
            ink: Ink::default(),
            weight: 400.0,
            ellipsis: false,
            tabular: false,
        }))
    }

    pub fn button(label: impl Into<String>, style: ButtonStyle, on_press: Option<M>) -> Self {
        Self::new(Kind::Button {
            label: label.into(),
            style,
            on_press,
        })
    }

    pub fn toggle(on: bool, pending: bool, on_toggle: Option<M>) -> Self {
        Self::new(Kind::Toggle { on, pending, on_toggle })
    }

    pub fn slider(value: f32, min: f32, max: f32, step: f32, on_change: Option<impl Fn(f32) -> M + 'static>) -> Self {
        Self::new(Kind::Slider(SliderEl {
            value,
            min,
            max,
            step,
            show_value: false,
            on_change: on_change.map(|change| std::rc::Rc::new(change) as std::rc::Rc<dyn Fn(f32) -> M>),
        }))
    }

    /// For a slider: its value written beside it.
    pub fn show_value(mut self) -> Self {
        if let Kind::Slider(slider) = &mut self.kind {
            slider.show_value = true;
        }
        self
    }

    /// A card of `children` in a column, under a title where there is one.
    pub fn card(style: CardStyle, title: Option<&str>, gap: f32, children: Vec<El<M>>) -> Self {
        let mut all = Vec::with_capacity(children.len() + 1);
        if let Some(title) = title {
            all.push(Self::text(title, Type::Title).weight(600.0));
        }
        all.extend(children);
        Self::new(Kind::Card { style, gap }).with(all)
    }

    pub fn icon(name: &'static str) -> Self {
        Self::new(Kind::Icon {
            name,
            ink: Ink::OnSurfaceVariant,
        })
    }

    /// A list of `len` rows `row_height` high, `row` building each one.
    pub fn list(len: usize, row_height: f32, row: impl Fn(usize) -> El<M> + 'static) -> Self {
        Self::new(Kind::List(ListEl {
            len,
            row_height,
            row: std::rc::Rc::new(row),
            selected: None,
            on_select: None,
            on_activate: None,
            empty: String::new(),
            first: 0,
            scroll: 0.0,
        }))
    }

    /// For a list: which row is chosen, and what it sends as rows are
    /// chosen and used.
    pub fn choosing(
        mut self,
        selected: Option<usize>,
        on_select: impl Fn(usize) -> M + 'static,
        on_activate: impl Fn(usize) -> M + 'static,
    ) -> Self {
        if let Kind::List(list) = &mut self.kind {
            list.selected = selected;
            list.on_select = Some(std::rc::Rc::new(on_select));
            list.on_activate = Some(std::rc::Rc::new(on_activate));
        }
        self
    }

    /// For a list: what it says with no rows.
    pub fn empty(mut self, text: impl Into<String>) -> Self {
        if let Kind::List(list) = &mut self.kind {
            list.empty = text.into();
        }
        self
    }

    pub fn tile(tile: TileEl<M>) -> Self {
        Self::new(Kind::Tile(tile))
    }

    /// Its key: what names it among its siblings, for ones that come and go.
    pub fn key(mut self, key: impl std::hash::Hash) -> Self {
        use std::hash::Hasher;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        self.key = Some(hasher.finish());
        self
    }

    pub fn width(mut self, width: Size) -> Self {
        self.width = width;
        self
    }

    pub fn height(mut self, height: Size) -> Self {
        self.height = height;
        self
    }

    pub fn fill(self) -> Self {
        self.width(Size::Fill(1.0)).height(Size::Fill(1.0))
    }

    pub fn padding(mut self, padding: Edges) -> Self {
        self.padding = padding;
        self
    }

    pub fn with(mut self, children: Vec<El<M>>) -> Self {
        self.children = children;
        self
    }

    /// This element with `text` shown when the pointer rests on it a while.
    pub fn tooltip(self, text: impl Into<String>) -> Self {
        let (width, height) = (self.width, self.height);
        let child = self
            .width(match width {
                Size::Hug => Size::Hug,
                _ => Size::Fill(1.0),
            })
            .height(match height {
                Size::Hug => Size::Hug,
                _ => Size::Fill(1.0),
            });
        Self::new(Kind::Tooltip { text: text.into() })
            .width(width)
            .height(height)
            .with(vec![child])
    }

    /// For a row or a column: where its children sit across it.
    pub fn align(mut self, to: Align) -> Self {
        if let Kind::Row { align, .. } | Kind::Column { align, .. } = &mut self.kind {
            *align = to;
        }
        self
    }

    /// For text and icons: their color.
    pub fn ink(mut self, ink: Ink) -> Self {
        match &mut self.kind {
            Kind::Text(text) => text.ink = ink,
            Kind::Icon { ink: own, .. } => *own = ink,
            _ => {}
        }
        self
    }

    /// For text: its weight, 400 regular.
    pub fn weight(mut self, weight: f32) -> Self {
        if let Kind::Text(text) = &mut self.kind {
            text.weight = weight;
        }
        self
    }

    /// For text: one line with an ellipsis where it doesn't fit.
    pub fn ellipsis(mut self) -> Self {
        if let Kind::Text(text) = &mut self.kind {
            text.ellipsis = true;
        }
        self
    }

    /// For text: digits of one width.
    pub fn tabular(mut self) -> Self {
        if let Kind::Text(text) = &mut self.kind {
            text.tabular = true;
        }
        self
    }
}
