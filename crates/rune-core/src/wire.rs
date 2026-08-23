use core::fmt;

use crate::{
    Action, Edge, InputDevice, MouseButton, Program, ProgramSet, ProgramSetError, SourceFilter,
    Trigger, MAX_KEY_CODE, MAX_MOUSE_BUTTON,
};

pub const WIRE_MAGIC: [u8; 4] = *b"RUNE";
pub const WIRE_VERSION: u16 = 1;
const MAX_PROGRAMS: usize = 16_384;
const MAX_ACTIONS_PER_PROGRAM: usize = 4_096;
const MAX_NAME_BYTES: usize = 1_024;
const MAX_DELAY_US: u32 = 60_000_000;

const OP_KEY_DOWN: u8 = 1;
const OP_KEY_UP: u8 = 2;
const OP_MOUSE_DOWN: u8 = 3;
const OP_MOUSE_UP: u8 = 4;
const OP_MOUSE_MOVE: u8 = 5;
const OP_MOUSE_WHEEL: u8 = 6;
const OP_DELAY_US: u8 = 7;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    UnexpectedEof,
    InvalidMagic,
    UnsupportedVersion(u16),
    TooManyPrograms(usize),
    NameTooLong(usize),
    InvalidUtf8,
    TooManyActions { program: usize, count: usize },
    InvalidDevice(u8),
    InvalidEdge(u8),
    InvalidSource(u8),
    InvalidTriggerCode { device: InputDevice, code: u16 },
    InvalidOpcode(u8),
    InvalidKeyCode(u16),
    InvalidMouseButton(u8),
    DelayTooLarge(u32),
    TrailingBytes(usize),
    ProgramSet(ProgramSetError),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof => f.write_str("unexpected end of Rune program data"),
            Self::InvalidMagic => f.write_str("invalid Rune program magic"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported Rune wire version {version}")
            }
            Self::TooManyPrograms(count) => write!(f, "program count {count} exceeds {MAX_PROGRAMS}"),
            Self::NameTooLong(length) => {
                write!(f, "program name length {length} exceeds {MAX_NAME_BYTES}")
            }
            Self::InvalidUtf8 => f.write_str("program name is not valid UTF-8"),
            Self::TooManyActions { program, count } => write!(
                f,
                "program {program} has {count} actions; maximum is {MAX_ACTIONS_PER_PROGRAM}"
            ),
            Self::InvalidDevice(value) => write!(f, "invalid input device value {value}"),
            Self::InvalidEdge(value) => write!(f, "invalid input edge value {value}"),
            Self::InvalidSource(value) => write!(f, "invalid input source filter value {value}"),
            Self::InvalidTriggerCode { device, code } => {
                write!(f, "invalid {device} trigger code {code}")
            }
            Self::InvalidOpcode(opcode) => write!(f, "invalid Rune action opcode {opcode}"),
            Self::InvalidKeyCode(code) => write!(f, "invalid Rune key code {code}"),
            Self::InvalidMouseButton(button) => write!(f, "invalid mouse button {button}"),
            Self::DelayTooLarge(delay) => {
                write!(f, "delay {delay} us exceeds the {MAX_DELAY_US} us limit")
            }
            Self::TrailingBytes(count) => write!(f, "Rune program contains {count} trailing bytes"),
            Self::ProgramSet(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for DecodeError {}

impl From<ProgramSetError> for DecodeError {
    fn from(value: ProgramSetError) -> Self {
        Self::ProgramSet(value)
    }
}

pub fn decode_program_set(bytes: &[u8]) -> Result<ProgramSet, DecodeError> {
    let mut reader = Reader::new(bytes);
    if reader.take_array::<4>()? != WIRE_MAGIC {
        return Err(DecodeError::InvalidMagic);
    }
    let version = reader.u16()?;
    if version != WIRE_VERSION {
        return Err(DecodeError::UnsupportedVersion(version));
    }
    let _flags = reader.u16()?;
    let program_count = reader.u32()? as usize;
    if program_count > MAX_PROGRAMS {
        return Err(DecodeError::TooManyPrograms(program_count));
    }

    let mut programs = Vec::with_capacity(program_count);
    for program_index in 0..program_count {
        let name_len = usize::from(reader.u16()?);
        if name_len > MAX_NAME_BYTES {
            return Err(DecodeError::NameTooLong(name_len));
        }
        let device_raw = reader.u8()?;
        let edge_raw = reader.u8()?;
        let source_raw = reader.u8()?;
        let _reserved = reader.u8()?;
        let code = reader.u16()?;
        let action_count = usize::from(reader.u16()?);
        if action_count > MAX_ACTIONS_PER_PROGRAM {
            return Err(DecodeError::TooManyActions {
                program: program_index,
                count: action_count,
            });
        }

        let device = InputDevice::try_from(device_raw)
            .map_err(|()| DecodeError::InvalidDevice(device_raw))?;
        let edge = Edge::try_from(edge_raw).map_err(|()| DecodeError::InvalidEdge(edge_raw))?;
        let source = SourceFilter::try_from(source_raw)
            .map_err(|()| DecodeError::InvalidSource(source_raw))?;
        validate_trigger_code(device, code)?;

        let name = std::str::from_utf8(reader.take(name_len)?)
            .map_err(|_| DecodeError::InvalidUtf8)?
            .into();

        let mut actions = Vec::with_capacity(action_count);
        for _ in 0..action_count {
            actions.push(decode_action(&mut reader)?);
        }

        programs.push(Program {
            name,
            trigger: Trigger {
                device,
                code,
                edge,
                source,
            },
            actions: actions.into_boxed_slice(),
        });
    }

    if !reader.is_empty() {
        return Err(DecodeError::TrailingBytes(reader.remaining()));
    }

    ProgramSet::new(programs).map_err(Into::into)
}

fn validate_trigger_code(device: InputDevice, code: u16) -> Result<(), DecodeError> {
    let valid = match device {
        InputDevice::Keyboard => usize::from(code) < MAX_KEY_CODE,
        InputDevice::MouseButton => usize::from(code) < MAX_MOUSE_BUTTON,
    };
    if valid {
        Ok(())
    } else {
        Err(DecodeError::InvalidTriggerCode { device, code })
    }
}

fn decode_action(reader: &mut Reader<'_>) -> Result<Action, DecodeError> {
    let opcode = reader.u8()?;
    match opcode {
        OP_KEY_DOWN | OP_KEY_UP => {
            let code = reader.u16()?;
            if usize::from(code) >= MAX_KEY_CODE {
                return Err(DecodeError::InvalidKeyCode(code));
            }
            Ok(if opcode == OP_KEY_DOWN {
                Action::KeyDown(code)
            } else {
                Action::KeyUp(code)
            })
        }
        OP_MOUSE_DOWN | OP_MOUSE_UP => {
            let raw = reader.u8()?;
            let button = MouseButton::try_from(raw)
                .map_err(|()| DecodeError::InvalidMouseButton(raw))?;
            Ok(if opcode == OP_MOUSE_DOWN {
                Action::MouseDown(button)
            } else {
                Action::MouseUp(button)
            })
        }
        OP_MOUSE_MOVE => Ok(Action::MouseMove {
            dx: reader.i32()?,
            dy: reader.i32()?,
        }),
        OP_MOUSE_WHEEL => Ok(Action::MouseWheel {
            x: reader.i32()?,
            y: reader.i32()?,
        }),
        OP_DELAY_US => {
            let delay = reader.u32()?;
            if delay > MAX_DELAY_US {
                return Err(DecodeError::DelayTooLarge(delay));
            }
            Ok(Action::DelayUs(delay))
        }
        _ => Err(DecodeError::InvalidOpcode(opcode)),
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], DecodeError> {
        let end = self
            .offset
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(DecodeError::UnexpectedEof)?;
        let result = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(result)
    }

    fn take_array<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| DecodeError::UnexpectedEof)
    }

    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.take_array()?))
    }

    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.take_array()?))
    }

    fn i32(&mut self) -> Result<i32, DecodeError> {
        Ok(i32::from_le_bytes(self.take_array()?))
    }

    const fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }

    const fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
}

#[cfg(test)]
mod tests {
    use crate::{Action, Edge, InputDevice, SourceFilter};

    use super::{decode_program_set, WIRE_MAGIC, WIRE_VERSION};

    #[test]
    fn decodes_a_program() {
        let mut bytes = Vec::new();
        bytes.extend(WIRE_MAGIC);
        bytes.extend(WIRE_VERSION.to_le_bytes());
        bytes.extend(0_u16.to_le_bytes());
        bytes.extend(1_u32.to_le_bytes());
        bytes.extend(5_u16.to_le_bytes());
        bytes.push(InputDevice::Keyboard as u8);
        bytes.push(Edge::Down as u8);
        bytes.push(SourceFilter::Physical as u8);
        bytes.push(0);
        bytes.extend(0x14_u16.to_le_bytes());
        bytes.extend(3_u16.to_le_bytes());
        bytes.extend(b"lunge");
        bytes.push(1);
        bytes.extend(0x08_u16.to_le_bytes());
        bytes.push(7);
        bytes.extend(80_u32.to_le_bytes());
        bytes.push(2);
        bytes.extend(0x08_u16.to_le_bytes());

        let set = decode_program_set(&bytes).unwrap();
        assert_eq!(set.len(), 1);
        assert_eq!(set.programs()[0].actions.as_ref(), [
            Action::KeyDown(0x08),
            Action::DelayUs(80),
            Action::KeyUp(0x08),
        ]);
    }
}
