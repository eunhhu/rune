use core::fmt;

/// Spellwire key identifiers use USB HID keyboard usage IDs, not platform virtual-key codes.
pub mod key {
    pub const A: u16 = 0x04;
    pub const B: u16 = 0x05;
    pub const C: u16 = 0x06;
    pub const D: u16 = 0x07;
    pub const E: u16 = 0x08;
    pub const F: u16 = 0x09;
    pub const G: u16 = 0x0a;
    pub const H: u16 = 0x0b;
    pub const I: u16 = 0x0c;
    pub const J: u16 = 0x0d;
    pub const K: u16 = 0x0e;
    pub const L: u16 = 0x0f;
    pub const M: u16 = 0x10;
    pub const N: u16 = 0x11;
    pub const O: u16 = 0x12;
    pub const P: u16 = 0x13;
    pub const Q: u16 = 0x14;
    pub const R: u16 = 0x15;
    pub const S: u16 = 0x16;
    pub const T: u16 = 0x17;
    pub const U: u16 = 0x18;
    pub const V: u16 = 0x19;
    pub const W: u16 = 0x1a;
    pub const X: u16 = 0x1b;
    pub const Y: u16 = 0x1c;
    pub const Z: u16 = 0x1d;

    pub const DIGIT_1: u16 = 0x1e;
    pub const DIGIT_2: u16 = 0x1f;
    pub const DIGIT_3: u16 = 0x20;
    pub const DIGIT_4: u16 = 0x21;
    pub const DIGIT_5: u16 = 0x22;
    pub const DIGIT_6: u16 = 0x23;
    pub const DIGIT_7: u16 = 0x24;
    pub const DIGIT_8: u16 = 0x25;
    pub const DIGIT_9: u16 = 0x26;
    pub const DIGIT_0: u16 = 0x27;

    pub const ENTER: u16 = 0x28;
    pub const ESCAPE: u16 = 0x29;
    pub const BACKSPACE: u16 = 0x2a;
    pub const TAB: u16 = 0x2b;
    pub const SPACE: u16 = 0x2c;
    pub const MINUS: u16 = 0x2d;
    pub const EQUAL: u16 = 0x2e;
    pub const LEFT_BRACKET: u16 = 0x2f;
    pub const RIGHT_BRACKET: u16 = 0x30;
    pub const BACKSLASH: u16 = 0x31;
    pub const SEMICOLON: u16 = 0x33;
    pub const QUOTE: u16 = 0x34;
    pub const GRAVE: u16 = 0x35;
    pub const COMMA: u16 = 0x36;
    pub const PERIOD: u16 = 0x37;
    pub const SLASH: u16 = 0x38;
    pub const CAPS_LOCK: u16 = 0x39;

    pub const F1: u16 = 0x3a;
    pub const F2: u16 = 0x3b;
    pub const F3: u16 = 0x3c;
    pub const F4: u16 = 0x3d;
    pub const F5: u16 = 0x3e;
    pub const F6: u16 = 0x3f;
    pub const F7: u16 = 0x40;
    pub const F8: u16 = 0x41;
    pub const F9: u16 = 0x42;
    pub const F10: u16 = 0x43;
    pub const F11: u16 = 0x44;
    pub const F12: u16 = 0x45;
    pub const PRINT_SCREEN: u16 = 0x46;
    pub const SCROLL_LOCK: u16 = 0x47;
    pub const PAUSE: u16 = 0x48;
    pub const INSERT: u16 = 0x49;
    pub const HOME: u16 = 0x4a;
    pub const PAGE_UP: u16 = 0x4b;
    pub const DELETE: u16 = 0x4c;
    pub const END: u16 = 0x4d;
    pub const PAGE_DOWN: u16 = 0x4e;
    pub const ARROW_RIGHT: u16 = 0x4f;
    pub const ARROW_LEFT: u16 = 0x50;
    pub const ARROW_DOWN: u16 = 0x51;
    pub const ARROW_UP: u16 = 0x52;

    pub const NUM_LOCK: u16 = 0x53;
    pub const NUMPAD_DIVIDE: u16 = 0x54;
    pub const NUMPAD_MULTIPLY: u16 = 0x55;
    pub const NUMPAD_SUBTRACT: u16 = 0x56;
    pub const NUMPAD_ADD: u16 = 0x57;
    pub const NUMPAD_ENTER: u16 = 0x58;
    pub const NUMPAD_1: u16 = 0x59;
    pub const NUMPAD_2: u16 = 0x5a;
    pub const NUMPAD_3: u16 = 0x5b;
    pub const NUMPAD_4: u16 = 0x5c;
    pub const NUMPAD_5: u16 = 0x5d;
    pub const NUMPAD_6: u16 = 0x5e;
    pub const NUMPAD_7: u16 = 0x5f;
    pub const NUMPAD_8: u16 = 0x60;
    pub const NUMPAD_9: u16 = 0x61;
    pub const NUMPAD_0: u16 = 0x62;
    pub const NUMPAD_DECIMAL: u16 = 0x63;
    pub const APPLICATION: u16 = 0x65;

    pub const LEFT_CONTROL: u16 = 0xe0;
    pub const LEFT_SHIFT: u16 = 0xe1;
    pub const LEFT_ALT: u16 = 0xe2;
    pub const LEFT_META: u16 = 0xe3;
    pub const RIGHT_CONTROL: u16 = 0xe4;
    pub const RIGHT_SHIFT: u16 = 0xe5;
    pub const RIGHT_ALT: u16 = 0xe6;
    pub const RIGHT_META: u16 = 0xe7;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InputDevice {
    Keyboard = 0,
    MouseButton = 1,
}

impl TryFrom<u8> for InputDevice {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Keyboard),
            1 => Ok(Self::MouseButton),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Edge {
    Down = 0,
    Up = 1,
}

impl TryFrom<u8> for Edge {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Down),
            1 => Ok(Self::Up),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InputSource {
    Physical = 0,
    Synthetic = 1,
}

impl TryFrom<u8> for InputSource {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Physical),
            1 => Ok(Self::Synthetic),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SourceFilter {
    Physical = 0,
    Synthetic = 1,
    Any = 2,
}

pub const MODIFIER_CONTROL: u8 = 1 << 0;
pub const MODIFIER_SHIFT: u8 = 1 << 1;
pub const MODIFIER_ALT: u8 = 1 << 2;
pub const MODIFIER_META: u8 = 1 << 3;
pub const MODIFIER_MASK: u8 = MODIFIER_CONTROL | MODIFIER_SHIFT | MODIFIER_ALT | MODIFIER_META;

pub const TRIGGER_CONSUME: u8 = 1 << 0;
pub const TRIGGER_EXACT_MODIFIERS: u8 = 1 << 1;
pub const TRIGGER_IGNORE_REPEAT: u8 = 1 << 2;
pub const TRIGGER_GATE_INVERTED: u8 = 1 << 3;
pub const TRIGGER_FLAGS: u8 =
    TRIGGER_CONSUME | TRIGGER_EXACT_MODIFIERS | TRIGGER_IGNORE_REPEAT | TRIGGER_GATE_INVERTED;
pub const NO_STATE_GATE: u16 = u16::MAX;

impl TryFrom<u8> for SourceFilter {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Physical),
            1 => Ok(Self::Synthetic),
            2 => Ok(Self::Any),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trigger {
    pub device: InputDevice,
    pub code: u16,
    pub edge: Edge,
    pub source: SourceFilter,
    pub flags: u8,
    pub modifiers: u8,
    pub gate: u16,
}

impl Trigger {
    #[must_use]
    pub const fn matches_context(self, modifiers: u8, repeated: bool) -> bool {
        if repeated && self.flags & TRIGGER_IGNORE_REPEAT != 0 {
            return false;
        }
        let modifiers = modifiers & MODIFIER_MASK;
        if self.flags & TRIGGER_EXACT_MODIFIERS != 0 {
            modifiers == self.modifiers
        } else {
            modifiers & self.modifiers == self.modifiers
        }
    }

    #[must_use]
    pub fn matches_gate(self, state: &[i64]) -> bool {
        if self.gate == NO_STATE_GATE {
            return true;
        }
        let active = state.get(usize::from(self.gate)).is_some_and(|value| *value != 0);
        active != (self.flags & TRIGGER_GATE_INVERTED != 0)
    }
}

#[must_use]
pub const fn modifier_for_key(code: u16) -> u8 {
    match code {
        key::LEFT_CONTROL | key::RIGHT_CONTROL => MODIFIER_CONTROL,
        key::LEFT_SHIFT | key::RIGHT_SHIFT => MODIFIER_SHIFT,
        key::LEFT_ALT | key::RIGHT_ALT => MODIFIER_ALT,
        key::LEFT_META | key::RIGHT_META => MODIFIER_META,
        _ => 0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputEvent {
    pub device: InputDevice,
    pub code: u16,
    pub edge: Edge,
    pub source: InputSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MouseButton {
    Left = 0,
    Right = 1,
    Middle = 2,
    Back = 3,
    Forward = 4,
}

impl TryFrom<u8> for MouseButton {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Left),
            1 => Ok(Self::Right),
            2 => Ok(Self::Middle),
            3 => Ok(Self::Back),
            4 => Ok(Self::Forward),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputEvent {
    Empty,
    Key { code: u16, down: bool },
    MouseButton { button: MouseButton, down: bool },
    MouseMove { dx: i32, dy: i32 },
    MouseWheel { x: i32, y: i32 },
}

impl fmt::Display for InputDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Keyboard => f.write_str("keyboard"),
            Self::MouseButton => f.write_str("mouse button"),
        }
    }
}
