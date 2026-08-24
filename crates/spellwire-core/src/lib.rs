mod bytecode;
mod model;
mod program;
mod vm;
mod wire;

pub use bytecode::{
    Instruction, Opcode, FLAG_STACK_OPERANDS, WIRE_HANDLER_SIZE, WIRE_HEADER_SIZE,
    WIRE_INSTRUCTION_SIZE, WIRE_MAGIC, WIRE_VERSION,
};
pub use model::{
    key, Edge, InputDevice, InputEvent, InputSource, MouseButton, OutputEvent, SourceFilter,
    Trigger,
};
pub use program::{Handler, HandlerTable, MatchingHandlers, Program, ProgramError};
pub use vm::{
    validate_program, DispatchError, DispatchReport, Injector, InputState, Runtime, RuntimeConfig,
    VmError, VmScratch, MAX_LOCALS, MAX_OUTPUT_BATCH, MAX_STACK,
};
pub use wire::DecodeError;
