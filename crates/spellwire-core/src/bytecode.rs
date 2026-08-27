use core::fmt;

pub const WIRE_MAGIC: [u8; 4] = *b"SPWR";
pub const WIRE_VERSION: u16 = 5;
pub const MIN_WIRE_VERSION: u16 = 3;
pub const WIRE_HEADER_SIZE: usize = 24;
pub const WIRE_HANDLER_SIZE: usize = 16;
pub const WIRE_INSTRUCTION_SIZE: usize = 16;

/// Output/query opcodes read their operands from the VM stack when this bit is set.
pub const FLAG_STACK_OPERANDS: u8 = 1 << 7;
/// `DelayUs` reads a signed 64-bit immediate, or scales its stack operand by that immediate.
pub const FLAG_WIDE_DELAY: u8 = 1 << 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Opcode {
    Halt = 0,
    PushConst = 1,
    LoadState = 2,
    StoreState = 3,
    LoadLocal = 4,
    StoreLocal = 5,
    Pop = 6,
    Dup = 7,
    Add = 8,
    Sub = 9,
    Mul = 10,
    Div = 11,
    Mod = 12,
    Neg = 13,
    Eq = 14,
    Ne = 15,
    Lt = 16,
    Le = 17,
    Gt = 18,
    Ge = 19,
    Not = 20,
    BitAnd = 21,
    BitOr = 22,
    BitXor = 23,
    Shl = 24,
    Shr = 25,
    Jump = 26,
    JumpIfFalse = 27,
    LoadInputCode = 28,
    LoadInputEdge = 29,
    LoadInputSource = 30,
    LoadHeld = 31,
    KeyDown = 32,
    KeyUp = 33,
    MouseDown = 34,
    MouseUp = 35,
    MouseMove = 36,
    MouseWheel = 37,
    DelayUs = 38,
    StoreStateImm = 39,
    AddStateImm = 40,
    XorStateImm = 41,
    ToggleState = 42,
    EmitEffect = 43,
}

impl TryFrom<u8> for Opcode {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Halt),
            1 => Ok(Self::PushConst),
            2 => Ok(Self::LoadState),
            3 => Ok(Self::StoreState),
            4 => Ok(Self::LoadLocal),
            5 => Ok(Self::StoreLocal),
            6 => Ok(Self::Pop),
            7 => Ok(Self::Dup),
            8 => Ok(Self::Add),
            9 => Ok(Self::Sub),
            10 => Ok(Self::Mul),
            11 => Ok(Self::Div),
            12 => Ok(Self::Mod),
            13 => Ok(Self::Neg),
            14 => Ok(Self::Eq),
            15 => Ok(Self::Ne),
            16 => Ok(Self::Lt),
            17 => Ok(Self::Le),
            18 => Ok(Self::Gt),
            19 => Ok(Self::Ge),
            20 => Ok(Self::Not),
            21 => Ok(Self::BitAnd),
            22 => Ok(Self::BitOr),
            23 => Ok(Self::BitXor),
            24 => Ok(Self::Shl),
            25 => Ok(Self::Shr),
            26 => Ok(Self::Jump),
            27 => Ok(Self::JumpIfFalse),
            28 => Ok(Self::LoadInputCode),
            29 => Ok(Self::LoadInputEdge),
            30 => Ok(Self::LoadInputSource),
            31 => Ok(Self::LoadHeld),
            32 => Ok(Self::KeyDown),
            33 => Ok(Self::KeyUp),
            34 => Ok(Self::MouseDown),
            35 => Ok(Self::MouseUp),
            36 => Ok(Self::MouseMove),
            37 => Ok(Self::MouseWheel),
            38 => Ok(Self::DelayUs),
            39 => Ok(Self::StoreStateImm),
            40 => Ok(Self::AddStateImm),
            41 => Ok(Self::XorStateImm),
            42 => Ok(Self::ToggleState),
            43 => Ok(Self::EmitEffect),
            _ => Err(()),
        }
    }
}

impl fmt::Display for Opcode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Fixed-width bytecode instruction. The wire representation is always 16 bytes:
/// opcode:u8, flags:u8, a:u16, b:u32, immediate:i64.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction {
    pub opcode: Opcode,
    pub flags: u8,
    pub a: u16,
    pub b: u32,
    pub immediate: i64,
}

impl Instruction {
    #[must_use]
    pub const fn new(opcode: Opcode) -> Self {
        Self { opcode, flags: 0, a: 0, b: 0, immediate: 0 }
    }

    #[must_use]
    pub const fn with_a(mut self, value: u16) -> Self {
        self.a = value;
        self
    }

    #[must_use]
    pub const fn with_b(mut self, value: u32) -> Self {
        self.b = value;
        self
    }

    #[must_use]
    pub const fn with_immediate(mut self, value: i64) -> Self {
        self.immediate = value;
        self
    }
}
