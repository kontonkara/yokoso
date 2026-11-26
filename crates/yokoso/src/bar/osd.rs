//! The OSD: a pill at the bottom middle of the main screen, saying what just
//! changed: the volume, the microphone, the output, the brightness, the
//! layout. It rises in, stays a moment (each change again keeps it), and
//! fades; the pointer goes through it to what is beneath.

use yokoso_ui::{
    app::{Frame, View},
    el::{Align, Edges, El, Ink, Size, Type, tree::Tree},
    motion::{Part, Spring},
    text::Text,
    theme::Theme,
};

use super::Msg;

pub const WIDTH: f32 = 300.0;
pub const HEIGHT: f32 = 52.0;
/// Room around the pill for its shadow, and how far it rises coming in.
pub const PAD: f32 = 12.0;
const RISE: f32 = 12.0;
const TRACK: f32 = 170.0;
const ICON: f32 = 20.0;

/// What it says.
#[derive(Clone, Debug, PartialEq)]
pub enum Shows {
    /// 1 is full; louder past it.
    Volume {
        level: f32,
        muted: bool,
    },
    Mic {
        level: f32,
        muted: bool,
    },
    /// The output sound goes to now, by what it is called.
    Output {
        name: String,
    },
    Brightness {
        level: f32,
    },
    Layout {
        name: String,
    },
}

impl Shows {
    /// Its name in `osd.hidden`.
    pub fn group(&self) -> &'static str {
        match self {
            Self::Volume { .. } => "volume",
            Self::Mic { .. } => "mic",
            Self::Output { .. } => "output",
            Self::Brightness { .. } => "brightness",
            Self::Layout { .. } => "layout",
        }
    }
}

pub struct Osd {
    pub shows: Option<Shows>,
    tree: Tree<Msg>,
    stale: bool,
    shown: Spring,
}

impl Default for Osd {
    fn default() -> Self {
        Self {
            shows: None,
            tree: Tree::default(),
            stale: false,
            shown: Spring::new(0.0),
        }
    }
}

impl Osd {
    pub fn show(&mut self, shows: Shows) {
        self.stale = self.shows.as_ref() != Some(&shows) || self.stale;
        self.shows = Some(shows);
        self.shown.to(1.0);
    }

    pub fn hide(&mut self) {
        self.shown.to(0.0);
    }

    /// Faded away, and still.
    pub fn gone(&self) -> bool {
        self.shown.target == 0.0 && !self.shown.is_moving()
    }

    pub fn moving(&self) -> bool {
        self.shown.is_moving()
    }

    pub fn update(&mut self, dt: f32, now: f64, theme: &Theme) -> bool {
        let feel = theme.motion.feel(Part::Content);
        let moving = self.shown.step(dt, &feel);
        self.tree.update(dt, now, theme) || moving
    }

    pub fn wake(&self) -> Option<std::time::Duration> {
        if self.moving() {
            Some(std::time::Duration::ZERO)
        } else {
            self.tree.wake()
        }
    }

    pub fn layout(&mut self, view: &View, text: &mut Text, theme: &Theme) {
        if self.stale
            && let Some(shows) = &self.shows
        {
            self.tree.set(El::column(0.0, vec![view_of(shows, theme)]));
            self.stale = false;
        }
        let inside = View {
            width: WIDTH,
            height: HEIGHT,
            scale: view.scale,
        };
        if self.tree.needs_layout(&inside) {
            self.tree.layout(&inside, text, theme);
        }
    }

    pub fn render(&self, frame: &mut Frame, theme: &Theme) {
        let shown = self.shown.value.clamp(0.0, 1.0);
        if shown <= 0.0 {
            return;
        }
        let colors = &theme.colors;
        let rise = (1.0 - shown) * RISE;
        let pill = yokoso_ui::Rect::new(PAD, PAD + rise, WIDTH, HEIGHT);
        frame.alpha(shown);
        frame.shadow(pill, HEIGHT / 2.0, 12.0, colors.shadow.alpha(0.35));
        frame.fill(pill, HEIGHT / 2.0, colors.surface_container_high);
        let base = frame.offset();
        frame.set_offset(base.0 + pill.x, base.1 + pill.y);
        self.tree.paint(frame, theme);
        frame.set_offset(base.0, base.1);
        frame.alpha(1.0);
    }
}

/// A level as a bar: a track and its fill, the whole of it past full.
fn level_bar(level: f32, ink: Ink, track: Ink) -> El<Msg> {
    let filled = (level.clamp(0.0, 1.0) * TRACK).max(3.0);
    El::stack(vec![
        El::block(track, 3.0).width(Size::Fixed(TRACK)).height(Size::Fixed(6.0)),
        El::block(ink, 3.0)
            .width(Size::Fixed(filled))
            .height(Size::Fixed(6.0))
            .glide(),
    ])
    .width(Size::Fixed(TRACK))
}

fn view_of(shows: &Shows, theme: &Theme) -> El<Msg> {
    let faint = Ink::Own(theme.colors.on_surface.alpha(0.5));
    let track = Ink::Own(theme.colors.on_surface.alpha(0.12));
    let percent = |level: f32| {
        El::text(format!("{:.0}%", level * 100.0), Type::Body)
            .tabular()
            .weight(600.0)
    };
    let icon = |name: &'static str, ink: Ink| El::icon(name).ink(ink).width(Size::Fixed(ICON));
    // A level: the icon at the left edge, the bar, the percent at the right.
    let level = |name: &'static str, at: f32, ink: Ink| {
        vec![icon(name, ink), level_bar(at, ink, track), El::spacer(), percent(at)]
    };
    // Words: in the middle, cut where they don't fit.
    let words = |el: El<Msg>| {
        El::column(0.0, vec![el.ellipsis()])
            .align(Align::Center)
            .width(Size::Fill(1.0))
    };
    let parts = match shows {
        Shows::Volume { level: at, muted } => {
            let name = match at {
                _ if *muted || *at <= 0.0 => "volume-x",
                l if *l < 0.34 => "volume-0",
                l if *l < 0.67 => "volume-1",
                _ => "volume",
            };
            level(name, *at, if *muted { faint } else { Ink::Primary })
        }
        Shows::Mic { level: at, muted } => level(
            if *muted { "mic-off" } else { "mic" },
            *at,
            if *muted { faint } else { Ink::Primary },
        ),
        Shows::Brightness { level: at } => level("sun", *at, Ink::Primary),
        // The icon's room kept at the right too: the name in the middle.
        Shows::Output { name } => vec![
            icon("volume", Ink::Primary),
            words(El::text(name.clone(), Type::Body).weight(600.0)),
            El::spacer().width(Size::Fixed(ICON)),
        ],
        Shows::Layout { name } => vec![words(El::text(name.clone(), Type::Title).weight(600.0))],
    };
    El::row(12.0, parts)
        .align(Align::Center)
        .padding(Edges {
            left: 18.0,
            right: 18.0,
            ..Edges::default()
        })
        .width(Size::Fixed(WIDTH))
        .height(Size::Fixed(HEIGHT))
}

/// What the OSD says of a change in the sound: the output moving to another
/// device, its volume or muting; the microphone's. Nothing as PipeWire is
/// first heard, or heard again after it was gone.
pub fn sound_change(
    old: &crate::pw::Sound,
    new: &crate::pw::Sound,
    old_mic: &crate::pw::Sound,
    new_mic: &crate::pw::Sound,
) -> Option<Shows> {
    use crate::pw::Sound;
    if let (Sound::Out(old), Sound::Out(new)) = (old, new) {
        if old.name != new.name {
            let name = if new.description.is_empty() {
                &new.name
            } else {
                &new.description
            };
            return Some(Shows::Output { name: name.clone() });
        }
        if old.volume != new.volume || old.muted != new.muted {
            return Some(Shows::Volume {
                level: new.volume,
                muted: new.muted,
            });
        }
    }
    if let (Sound::Out(old), Sound::Out(new)) = (old_mic, new_mic)
        && old.name == new.name
        && (old.volume != new.volume || old.muted != new.muted)
    {
        return Some(Shows::Mic {
            level: new.volume,
            muted: new.muted,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pw::{Output, Sound};

    fn out(name: &str, volume: f32, muted: bool) -> Sound {
        Sound::Out(Output {
            name: name.into(),
            description: format!("{name} out"),
            volume,
            muted,
        })
    }

    #[test]
    fn a_change_in_the_sound_says_what_changed_and_nothing_at_first() {
        let none = Sound::default();
        let mic = out("mic", 1.0, false);
        assert_eq!(
            sound_change(&none, &out("a", 0.5, false), &none, &mic),
            None,
            "first heard"
        );
        assert_eq!(
            sound_change(&out("a", 0.5, false), &out("a", 0.55, false), &mic, &mic),
            Some(Shows::Volume {
                level: 0.55,
                muted: false
            })
        );
        assert_eq!(
            sound_change(&out("a", 0.5, false), &out("phones", 0.3, false), &mic, &mic),
            Some(Shows::Output {
                name: "phones out".into()
            })
        );
        assert_eq!(
            sound_change(
                &out("a", 0.5, false),
                &out("a", 0.5, false),
                &mic,
                &out("mic", 1.0, true)
            ),
            Some(Shows::Mic {
                level: 1.0,
                muted: true
            })
        );
        assert_eq!(
            sound_change(&out("a", 0.5, false), &out("a", 0.5, false), &mic, &mic),
            None
        );
    }
}
