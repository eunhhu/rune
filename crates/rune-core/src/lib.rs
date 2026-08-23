//! Realtime, allocation-free macro execution primitives used by Rune.
//!
//! `rune-core` deliberately has no third-party dependencies. The control plane may allocate while
//! decoding and compiling programs, but dispatching an input event does not allocate or invoke
//! JavaScript.

mod executor;
mod model;
mod program;
mod wire;

pub use executor::{
    DispatchError, DispatchReport, Engine, ExecutionConfig, ExecutionScratch, Injector,
    MAX_OUTPUT_BATCH,
};
pub use model::{
    key, Action, Edge, InputDevice, InputEvent, InputSource, MouseButton, OutputEvent,
    SourceFilter, Trigger,
};
pub use program::{Program, ProgramSet, ProgramSetError, MAX_KEY_CODE, MAX_MOUSE_BUTTON};
pub use wire::{decode_program_set, DecodeError, WIRE_MAGIC, WIRE_VERSION};
