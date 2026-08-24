use core::fmt;

use crate::{
    bytecode::{WIRE_HANDLER_SIZE, WIRE_HEADER_SIZE, WIRE_INSTRUCTION_SIZE, WIRE_MAGIC, WIRE_VERSION},
    Edge, Handler, InputDevice, Instruction, Opcode, Program, SourceFilter, Trigger,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    Truncated,
    InvalidMagic,
    UnsupportedVersion(u16),
    InvalidDevice(u8),
    InvalidEdge(u8),
    InvalidSource(u8),
    InvalidOpcode(u8),
    SizeOverflow,
    TrailingBytes(usize),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => f.write_str("truncated Spellwire bytecode"),
            Self::InvalidMagic => f.write_str("invalid Spellwire bytecode magic"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported Spellwire bytecode version {version}")
            }
            Self::InvalidDevice(value) => write!(f, "invalid input device {value}"),
            Self::InvalidEdge(value) => write!(f, "invalid input edge {value}"),
            Self::InvalidSource(value) => write!(f, "invalid input source filter {value}"),
            Self::InvalidOpcode(value) => write!(f, "invalid opcode {value}"),
            Self::SizeOverflow => f.write_str("Spellwire bytecode size overflow"),
            Self::TrailingBytes(count) => write!(f, "Spellwire bytecode has {count} trailing bytes"),
        }
    }
}

impl std::error::Error for DecodeError {}

impl Program {
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut reader = Reader::new(bytes);
        if reader.take(4)? != WIRE_MAGIC {
            return Err(DecodeError::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != WIRE_VERSION {
            return Err(DecodeError::UnsupportedVersion(version));
        }
        let _flags = reader.u16()?;
        let state_count = usize::from(reader.u16()?);
        let handler_count = usize::from(reader.u16()?);
        let local_count = reader.u16()?;
        let stack_limit = reader.u16()?;
        let instruction_count = reader.u32()? as usize;
        let instruction_budget = reader.u32()?;

        debug_assert_eq!(reader.offset(), WIRE_HEADER_SIZE);

        let state_bytes = state_count.checked_mul(8).ok_or(DecodeError::SizeOverflow)?;
        let handler_bytes = handler_count
            .checked_mul(WIRE_HANDLER_SIZE)
            .ok_or(DecodeError::SizeOverflow)?;
        let instruction_bytes = instruction_count
            .checked_mul(WIRE_INSTRUCTION_SIZE)
            .ok_or(DecodeError::SizeOverflow)?;
        let expected_remaining = state_bytes
            .checked_add(handler_bytes)
            .and_then(|value| value.checked_add(instruction_bytes))
            .ok_or(DecodeError::SizeOverflow)?;
        if reader.remaining() < expected_remaining {
            return Err(DecodeError::Truncated);
        }

        let mut initial_state = Vec::with_capacity(state_count);
        for _ in 0..state_count {
            initial_state.push(reader.i64()?);
        }

        let mut handlers = Vec::with_capacity(handler_count);
        for _ in 0..handler_count {
            let raw_device = reader.u8()?;
            let raw_edge = reader.u8()?;
            let raw_source = reader.u8()?;
            let _reserved = reader.u8()?;
            let code = reader.u16()?;
            let _reserved = reader.u16()?;
            let entry = reader.u32()?;
            handlers.push(Handler {
                trigger: Trigger {
                    device: InputDevice::try_from(raw_device)
                        .map_err(|()| DecodeError::InvalidDevice(raw_device))?,
                    code,
                    edge: Edge::try_from(raw_edge)
                        .map_err(|()| DecodeError::InvalidEdge(raw_edge))?,
                    source: SourceFilter::try_from(raw_source)
                        .map_err(|()| DecodeError::InvalidSource(raw_source))?,
                },
                entry,
            });
        }

        let mut code = Vec::with_capacity(instruction_count);
        for _ in 0..instruction_count {
            let raw_opcode = reader.u8()?;
            let flags = reader.u8()?;
            let a = reader.u16()?;
            let b = reader.u32()?;
            let immediate = reader.i64()?;
            code.push(Instruction {
                opcode: Opcode::try_from(raw_opcode)
                    .map_err(|()| DecodeError::InvalidOpcode(raw_opcode))?,
                flags,
                a,
                b,
                immediate,
            });
        }

        if reader.remaining() != 0 {
            return Err(DecodeError::TrailingBytes(reader.remaining()));
        }

        Ok(Self {
            initial_state: initial_state.into_boxed_slice(),
            handlers: handlers.into_boxed_slice(),
            code: code.into_boxed_slice(),
            local_count,
            stack_limit,
            instruction_budget,
        })
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    const fn offset(&self) -> usize {
        self.cursor
    }

    const fn remaining(&self) -> usize {
        self.bytes.len() - self.cursor
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], DecodeError> {
        let end = self.cursor.checked_add(count).ok_or(DecodeError::SizeOverflow)?;
        let slice = self.bytes.get(self.cursor..end).ok_or(DecodeError::Truncated)?;
        self.cursor = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, DecodeError> {
        let bytes: [u8; 2] = self.take(2)?.try_into().map_err(|_| DecodeError::Truncated)?;
        Ok(u16::from_le_bytes(bytes))
    }

    fn u32(&mut self) -> Result<u32, DecodeError> {
        let bytes: [u8; 4] = self.take(4)?.try_into().map_err(|_| DecodeError::Truncated)?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn i64(&mut self) -> Result<i64, DecodeError> {
        let bytes: [u8; 8] = self.take(8)?.try_into().map_err(|_| DecodeError::Truncated)?;
        Ok(i64::from_le_bytes(bytes))
    }
}

#[cfg(test)]
mod tests {
    use crate::{bytecode::WIRE_MAGIC, Opcode};

    use super::*;

    #[test]
    fn decodes_minimal_program() {
        let mut bytes = Vec::new();
        bytes.extend(WIRE_MAGIC);
        bytes.extend(WIRE_VERSION.to_le_bytes());
        bytes.extend(0_u16.to_le_bytes());
        bytes.extend(1_u16.to_le_bytes()); // states
        bytes.extend(1_u16.to_le_bytes()); // handlers
        bytes.extend(0_u16.to_le_bytes()); // locals
        bytes.extend(8_u16.to_le_bytes()); // stack
        bytes.extend(1_u32.to_le_bytes()); // instructions
        bytes.extend(100_u32.to_le_bytes()); // budget
        bytes.extend(7_i64.to_le_bytes());
        bytes.extend([0, 0, 0, 0]);
        bytes.extend(0x14_u16.to_le_bytes());
        bytes.extend(0_u16.to_le_bytes());
        bytes.extend(0_u32.to_le_bytes());
        bytes.push(Opcode::Halt as u8);
        bytes.push(0);
        bytes.extend(0_u16.to_le_bytes());
        bytes.extend(0_u32.to_le_bytes());
        bytes.extend(0_i64.to_le_bytes());

        let program = Program::decode(&bytes).unwrap();
        assert_eq!(program.initial_state.as_ref(), &[7]);
        assert_eq!(program.code[0].opcode, Opcode::Halt);
    }
}
