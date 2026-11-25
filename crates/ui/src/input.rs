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
    /// Text from the clipboard, as asked for.
    Paste(String),
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
    /// The physical key, its evdev code: shortcuts go by where a key is, so
    /// Ctrl+C is Ctrl+C in any layout.
    pub code: u32,
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

/// The keypad's keys that move, as the keys they stand for: with Num Lock
/// off, its 7 is Home and its 1 End, and they work as those do.
pub fn plain(keysym: Keysym) -> Keysym {
    match keysym {
        Keysym::KP_Home => Keysym::Home,
        Keysym::KP_End => Keysym::End,
        Keysym::KP_Up => Keysym::Up,
        Keysym::KP_Down => Keysym::Down,
        Keysym::KP_Left => Keysym::Left,
        Keysym::KP_Right => Keysym::Right,
        Keysym::KP_Page_Up => Keysym::Page_Up,
        Keysym::KP_Page_Down => Keysym::Page_Down,
        Keysym::KP_Insert => Keysym::Insert,
        Keysym::KP_Delete => Keysym::Delete,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keypad_moves_as_the_keys_it_stands_for() {
        assert_eq!(plain(Keysym::KP_Home), Keysym::Home);
        assert_eq!(plain(Keysym::KP_End), Keysym::End);
        assert_eq!(plain(Keysym::KP_Page_Down), Keysym::Page_Down);
        // Its digits, with Num Lock on, and every other key, stay as they are.
        assert_eq!(plain(Keysym::KP_7), Keysym::KP_7);
        assert_eq!(plain(Keysym::a), Keysym::a);
    }
}
