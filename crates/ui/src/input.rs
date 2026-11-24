//! Input as a surface gets it, in its logical pixels.

pub use smithay_client_toolkit::seat::keyboard::Keysym;

#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    PointerEnter {
        x: f32,
        y: f32,
    },
    PointerLeave,
    PointerMove {
        x: f32,
        y: f32,
    },
    /// A button, by its evdev code (`BTN_LEFT` is 0x110).
    Press {
        button: u32,
        x: f32,
        y: f32,
    },
    Release {
        button: u32,
        x: f32,
        y: f32,
    },
    Scroll(Scroll),
    KeyboardEnter,
    KeyboardLeave,
    Key(Key),
}

/// One pointer frame's worth of scrolling.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scroll {
    /// Logical pixels, right and down.
    pub x: f32,
    pub y: f32,
    /// Wheel steps, from `value120`: whole notches are 1, a high resolution
    /// wheel gives parts of one. 0 for fingers.
    pub steps_x: f32,
    pub steps_y: f32,
    pub source: ScrollSource,
    /// The fingers left the touchpad: from here on, kinetic scrolling is the
    /// client's to do.
    pub stop: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollSource {
    #[default]
    Wheel,
    Finger,
    Continuous,
    WheelTilt,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    pub keysym: Keysym,
    /// What it types, after the layout and compose (dead keys): None for keys
    /// that type nothing, and for a dead key itself.
    pub text: Option<String>,
    pub state: KeyState,
    pub mods: Mods,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyState {
    Pressed,
    /// Held: another press, at the compositor's repeat rate and delay.
    Repeated,
    Released,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub logo: bool,
    pub caps_lock: bool,
    pub num_lock: bool,
}
