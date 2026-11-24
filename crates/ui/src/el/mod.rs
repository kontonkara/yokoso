//! Elements: what the shell describes a surface with. A description is
//! light and built anew when the shell's state changes; what an element
//! keeps between rebuilds lives in the tree, by its id.

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
    /// Room and nothing else.
    Spacer,
    Text(TextEl),
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

    /// For a row or a column: where its children sit across it.
    pub fn align(mut self, to: Align) -> Self {
        if let Kind::Row { align, .. } | Kind::Column { align, .. } = &mut self.kind {
            *align = to;
        }
        self
    }

    /// For text: its color.
    pub fn ink(mut self, ink: Ink) -> Self {
        if let Kind::Text(text) = &mut self.kind {
            text.ink = ink;
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
