mod bytecode;
mod model;
mod program;
mod vm;
mod wire;

pub use bytecode::{
    Instruction, Opcode, FLAG_STACK_OPERANDS, FLAG_WIDE_DELAY, MIN_WIRE_VERSION, WIRE_HANDLER_SIZE,
    WIRE_HEADER_SIZE, WIRE_INSTRUCTION_SIZE, WIRE_MAGIC, WIRE_VERSION,
};
pub use model::{
    key, modifier_for_key, Edge, EffectEvent, InputDevice, InputEvent, InputSource, MouseButton,
    OutputEvent, SourceFilter, Trigger, MAX_EFFECT_VALUES, MODIFIER_ALT, MODIFIER_CONTROL,
    MODIFIER_MASK, MODIFIER_META, MODIFIER_SHIFT, NO_STATE_GATE, TRIGGER_CONSUME,
    TRIGGER_EXACT_MODIFIERS, TRIGGER_FLAGS, TRIGGER_GATE_INVERTED, TRIGGER_IGNORE_REPEAT,
};
pub use program::{Handler, HandlerTable, MatchingHandlers, Program, ProgramError};
pub use vm::{
    validate_program, ContinuationScheduler, DispatchError, DispatchReport, Injector, InputState,
    PollReport, Runtime, RuntimeConfig, SchedulerFull, VmError, VmScratch,
    DEFAULT_MAX_CONTINUATIONS, MAX_LOCALS, MAX_OUTPUT_BATCH, MAX_STACK,
};
pub use wire::DecodeError;
