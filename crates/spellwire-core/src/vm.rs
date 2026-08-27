use core::{fmt, hint::spin_loop};
use std::{
    thread,
    time::{Duration, Instant},
};

use crate::{
    key, Edge, HandlerTable, InputDevice, InputEvent, InputSource, MouseButton, Opcode,
    OutputEvent, Program, ProgramError, MODIFIER_ALT, MODIFIER_CONTROL, MODIFIER_META,
    MODIFIER_SHIFT, TRIGGER_CONSUME,
};

pub const MAX_STACK: usize = 256;
pub const MAX_LOCALS: usize = 256;
pub const MAX_OUTPUT_BATCH: usize = 64;
pub const DEFAULT_MAX_CONTINUATIONS: usize = 64;

pub trait Injector {
    type Error;

    /// Submit a contiguous, zero-delay output batch to the platform backend.
    ///
    /// # Errors
    ///
    /// Returns the backend-specific error when the batch cannot be submitted.
    fn send(&mut self, events: &[OutputEvent]) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeConfig {
    pub spin_threshold: Duration,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self { spin_threshold: Duration::from_micros(100) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmError {
    StackOverflow,
    StackUnderflow,
    DivisionByZero,
    InvalidProgramCounter(u32),
    InvalidKeyCode(i64),
    InvalidMouseButton(i64),
    InvalidCoordinate(i64),
    InvalidDelay(i64),
    InstructionBudgetExceeded(u32),
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StackOverflow => f.write_str("VM stack overflow"),
            Self::StackUnderflow => f.write_str("VM stack underflow"),
            Self::DivisionByZero => f.write_str("division by zero"),
            Self::InvalidProgramCounter(pc) => write!(f, "invalid program counter {pc}"),
            Self::InvalidKeyCode(code) => write!(f, "invalid key code {code}"),
            Self::InvalidMouseButton(button) => write!(f, "invalid mouse button {button}"),
            Self::InvalidCoordinate(value) => {
                write!(f, "mouse coordinate delta {value} does not fit i32")
            }
            Self::InvalidDelay(value) => write!(f, "delay {value} does not fit u32 microseconds"),
            Self::InstructionBudgetExceeded(budget) => {
                write!(f, "instruction budget {budget} exceeded")
            }
        }
    }
}

impl std::error::Error for VmError {}

#[derive(Debug)]
pub enum DispatchError<E> {
    Vm { handler_id: u16, source: VmError },
    Inject { handler_id: u16, source: E },
}

impl<E: fmt::Display> fmt::Display for DispatchError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Vm { handler_id, source } => {
                write!(f, "handler {handler_id} failed in VM: {source}")
            }
            Self::Inject { handler_id, source } => {
                write!(f, "handler {handler_id} failed to inject: {source}")
            }
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for DispatchError<E> {}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DispatchReport {
    pub handlers: u16,
    pub instructions: u32,
    pub output_events: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PollReport {
    pub completed_handlers: u16,
    pub instructions: u32,
    pub output_events: u32,
    pub pending_handlers: u16,
    pub next_deadline: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerFull {
    pub capacity: usize,
    pub pending: usize,
    pub requested: usize,
}

impl fmt::Display for SchedulerFull {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "continuation scheduler is full (capacity {}, pending {}, requested {})",
            self.capacity, self.pending, self.requested
        )
    }
}

impl std::error::Error for SchedulerFull {}

#[derive(Debug, Clone)]
pub struct InputState {
    keyboard: [[u64; 4]; 2],
    mouse: [u16; 2],
}

impl InputState {
    #[must_use]
    pub const fn new() -> Self {
        Self { keyboard: [[0; 4]; 2], mouse: [0; 2] }
    }

    pub fn apply(&mut self, event: InputEvent) {
        let down = event.edge == Edge::Down;
        let source = event.source as usize;
        match event.device {
            InputDevice::Keyboard if event.code < 256 => {
                let word = usize::from(event.code / 64);
                let bit = u32::from(event.code % 64);
                if down {
                    self.keyboard[source][word] |= 1_u64 << bit;
                } else {
                    self.keyboard[source][word] &= !(1_u64 << bit);
                }
            }
            InputDevice::MouseButton if event.code < 16 => {
                let mask = 1_u16 << event.code;
                if down {
                    self.mouse[source] |= mask;
                } else {
                    self.mouse[source] &= !mask;
                }
            }
            _ => {}
        }
    }

    #[must_use]
    pub fn held(&self, device: InputDevice, code: u16) -> bool {
        self.held_for_source(device, code, InputSource::Physical)
            || self.held_for_source(device, code, InputSource::Synthetic)
    }

    #[must_use]
    pub fn held_for_source(&self, device: InputDevice, code: u16, source: InputSource) -> bool {
        let source = source as usize;
        match device {
            InputDevice::Keyboard if code < 256 => {
                let word = usize::from(code / 64);
                let bit = u32::from(code % 64);
                self.keyboard[source][word] & (1_u64 << bit) != 0
            }
            InputDevice::MouseButton if code < 16 => self.mouse[source] & (1_u16 << code) != 0,
            _ => false,
        }
    }

    #[must_use]
    pub fn modifiers(&self, source: InputSource) -> u8 {
        let mut modifiers = 0;
        if self.held_for_source(InputDevice::Keyboard, key::LEFT_CONTROL, source)
            || self.held_for_source(InputDevice::Keyboard, key::RIGHT_CONTROL, source)
        {
            modifiers |= MODIFIER_CONTROL;
        }
        if self.held_for_source(InputDevice::Keyboard, key::LEFT_SHIFT, source)
            || self.held_for_source(InputDevice::Keyboard, key::RIGHT_SHIFT, source)
        {
            modifiers |= MODIFIER_SHIFT;
        }
        if self.held_for_source(InputDevice::Keyboard, key::LEFT_ALT, source)
            || self.held_for_source(InputDevice::Keyboard, key::RIGHT_ALT, source)
        {
            modifiers |= MODIFIER_ALT;
        }
        if self.held_for_source(InputDevice::Keyboard, key::LEFT_META, source)
            || self.held_for_source(InputDevice::Keyboard, key::RIGHT_META, source)
        {
            modifiers |= MODIFIER_META;
        }
        modifiers
    }

    /// Returns logical modifiers for trigger matching, excluding the current modifier key itself.
    /// The opposite left/right key still satisfies the logical modifier group.
    #[must_use]
    #[inline]
    pub fn modifiers_for_event(&self, event: InputEvent) -> u8 {
        let modifiers = self.modifiers(event.source);
        let (group, opposite) = match event.code {
            key::LEFT_CONTROL => (MODIFIER_CONTROL, key::RIGHT_CONTROL),
            key::RIGHT_CONTROL => (MODIFIER_CONTROL, key::LEFT_CONTROL),
            key::LEFT_SHIFT => (MODIFIER_SHIFT, key::RIGHT_SHIFT),
            key::RIGHT_SHIFT => (MODIFIER_SHIFT, key::LEFT_SHIFT),
            key::LEFT_ALT => (MODIFIER_ALT, key::RIGHT_ALT),
            key::RIGHT_ALT => (MODIFIER_ALT, key::LEFT_ALT),
            key::LEFT_META => (MODIFIER_META, key::RIGHT_META),
            key::RIGHT_META => (MODIFIER_META, key::LEFT_META),
            _ => return modifiers,
        };
        if self.held_for_source(InputDevice::Keyboard, opposite, event.source) {
            modifiers
        } else {
            modifiers & !group
        }
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct VmScratch {
    stack: [i64; MAX_STACK],
    stack_len: usize,
    locals: [i64; MAX_LOCALS],
    output: [OutputEvent; MAX_OUTPUT_BATCH],
    output_len: usize,
}

struct ExecutionCursor {
    handler_id: u16,
    event: InputEvent,
    pc: u32,
    instructions: u32,
    output_events: u32,
    deadline: Instant,
}

impl ExecutionCursor {
    fn new(handler_id: u16, event: InputEvent, entry: u32, now: Instant) -> Self {
        Self { handler_id, event, pc: entry, instructions: 0, output_events: 0, deadline: now }
    }
}

struct Continuation {
    cursor: ExecutionCursor,
    scratch: VmScratch,
}

/// Fixed-capacity queue for handlers suspended at `sleepUs()` deadlines.
///
/// The host owns one scheduler per runtime and calls [`Runtime::poll_ready`] whenever input
/// arrives or the next deadline expires. This keeps the observation thread free of VM sleeps.
pub struct ContinuationScheduler {
    pending: Vec<Continuation>,
    capacity: usize,
}

impl ContinuationScheduler {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self { pending: Vec::with_capacity(capacity), capacity }
    }

    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        self.pending.iter().map(|continuation| continuation.cursor.deadline).min()
    }

    pub fn clear(&mut self) {
        self.pending.clear();
    }
}

impl Default for ContinuationScheduler {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_CONTINUATIONS)
    }
}

impl VmScratch {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            stack: [0; MAX_STACK],
            stack_len: 0,
            locals: [0; MAX_LOCALS],
            output: [OutputEvent::Empty; MAX_OUTPUT_BATCH],
            output_len: 0,
        }
    }

    fn reset(&mut self, local_count: usize) {
        self.stack_len = 0;
        self.output_len = 0;
        self.locals[..local_count].fill(0);
    }

    fn push(&mut self, value: i64, stack_limit: usize) -> Result<(), VmError> {
        if self.stack_len >= stack_limit {
            return Err(VmError::StackOverflow);
        }
        self.stack[self.stack_len] = value;
        self.stack_len += 1;
        Ok(())
    }

    fn pop(&mut self) -> Result<i64, VmError> {
        if self.stack_len == 0 {
            return Err(VmError::StackUnderflow);
        }
        self.stack_len -= 1;
        Ok(self.stack[self.stack_len])
    }

    fn peek(&self) -> Result<i64, VmError> {
        self.stack_len.checked_sub(1).map(|index| self.stack[index]).ok_or(VmError::StackUnderflow)
    }

    fn queue<I: Injector>(&mut self, event: OutputEvent, injector: &mut I) -> Result<(), I::Error> {
        if self.output_len == self.output.len() {
            self.flush(injector)?;
        }
        self.output[self.output_len] = event;
        self.output_len += 1;
        Ok(())
    }

    fn flush<I: Injector>(&mut self, injector: &mut I) -> Result<(), I::Error> {
        if self.output_len != 0 {
            let len = core::mem::replace(&mut self.output_len, 0);
            injector.send(&self.output[..len])?;
        }
        Ok(())
    }
}

impl Default for VmScratch {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct Runtime {
    program: Program,
    handlers: HandlerTable,
    state: Box<[i64]>,
    input_state: InputState,
    release_arms: Box<[u8]>,
    has_consuming_releases: bool,
    config: RuntimeConfig,
}

impl Runtime {
    /// Validates a program and creates its trigger table and persistent state.
    ///
    /// # Errors
    ///
    /// Returns [`ProgramError`] when bytecode metadata, triggers, entries, jumps, or resource
    /// limits are invalid.
    pub fn new(program: Program, config: RuntimeConfig) -> Result<Self, ProgramError> {
        validate_program(&program)?;
        let handlers = HandlerTable::build(&program.handlers)?;
        let state = program.initial_state.clone();
        let release_arms = vec![0; program.handlers.len()].into_boxed_slice();
        let has_consuming_releases = program.handlers.iter().any(|handler| {
            handler.trigger.edge == Edge::Up && handler.trigger.flags & TRIGGER_CONSUME != 0
        });
        Ok(Self {
            program,
            handlers,
            state,
            input_state: InputState::new(),
            release_arms,
            has_consuming_releases,
            config,
        })
    }

    #[must_use]
    pub fn state(&self) -> &[i64] {
        &self.state
    }

    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }

    pub fn set_state(&mut self, slot: usize, value: i64) -> bool {
        let Some(target) = self.state.get_mut(slot) else {
            return false;
        };
        *target = value;
        true
    }

    #[must_use]
    pub fn get_state(&self, slot: usize) -> Option<i64> {
        self.state.get(slot).copied()
    }

    /// Clears held-input and release-latch bookkeeping after an observer queue overflow.
    /// Persistent program state is intentionally preserved.
    pub fn reset_input_tracking(&mut self) {
        self.input_state = InputState::new();
        self.release_arms.fill(0);
    }

    #[inline]
    fn arm_consuming_releases(&mut self, event: InputEvent, modifiers: u8, repeated: bool) {
        if !self.has_consuming_releases || event.edge != Edge::Down || repeated {
            return;
        }
        let release = InputEvent { edge: Edge::Up, ..event };
        let source_bit = 1_u8 << event.source as u8;
        for handler_id in self.handlers.matching(release) {
            let index = usize::from(handler_id);
            let trigger = self.program.handlers[index].trigger;
            if trigger.flags & TRIGGER_CONSUME != 0
                && trigger.matches_context(modifiers, false)
                && trigger.matches_gate(&self.state)
            {
                self.release_arms[index] |= source_bit;
            }
        }
    }

    #[inline]
    fn handler_matches(
        &self,
        handler_id: u16,
        event: InputEvent,
        modifiers: u8,
        repeated: bool,
    ) -> bool {
        let index = usize::from(handler_id);
        let trigger = self.program.handlers[index].trigger;
        if event.edge == Edge::Up && trigger.flags & TRIGGER_CONSUME != 0 {
            self.release_arms[index] & (1_u8 << event.source as u8) != 0
        } else {
            trigger.matches_context(modifiers, repeated) && trigger.matches_gate(&self.state)
        }
    }

    #[inline]
    fn clear_consuming_release_arms(&mut self, event: InputEvent) {
        if !self.has_consuming_releases || event.edge != Edge::Up {
            return;
        }
        let source_mask = !(1_u8 << event.source as u8);
        for handler_id in self.handlers.matching(event) {
            let index = usize::from(handler_id);
            if self.program.handlers[index].trigger.flags & TRIGGER_CONSUME != 0 {
                self.release_arms[index] &= source_mask;
            }
        }
    }

    /// Applies one input event and executes every matching handler.
    ///
    /// # Errors
    ///
    /// Returns [`DispatchError::Vm`] for bytecode execution failures or
    /// [`DispatchError::Inject`] when the platform injector rejects an output batch.
    pub fn dispatch<I: Injector>(
        &mut self,
        event: InputEvent,
        injector: &mut I,
        scratch: &mut VmScratch,
    ) -> Result<DispatchReport, DispatchError<I::Error>> {
        let modifiers = self.input_state.modifiers_for_event(event);
        let repeated = event.edge == Edge::Down
            && self.input_state.held_for_source(event.device, event.code, event.source);
        self.arm_consuming_releases(event, modifiers, repeated);
        self.input_state.apply(event);

        let mut report = DispatchReport::default();
        let Runtime {
            program,
            handlers,
            state,
            input_state,
            release_arms,
            has_consuming_releases: _,
            config,
        } = self;
        for handler_id in handlers.matching(event) {
            let index = usize::from(handler_id);
            let trigger = program.handlers[index].trigger;
            let matches = if event.edge == Edge::Up && trigger.flags & TRIGGER_CONSUME != 0 {
                release_arms[index] & (1_u8 << event.source as u8) != 0
            } else {
                trigger.matches_context(modifiers, repeated) && trigger.matches_gate(state)
            };
            if !matches {
                continue;
            }
            let entry = program.handlers[index].entry;
            scratch.reset(usize::from(program.local_count));
            let mut cursor = ExecutionCursor::new(handler_id, event, entry, Instant::now());
            let execution = loop {
                match execute_until_yield(
                    program,
                    input_state,
                    state,
                    injector,
                    scratch,
                    &mut cursor,
                ) {
                    Ok(ExecutionStep::Complete(report)) => break Ok(report),
                    Ok(ExecutionStep::Yield(deadline)) => {
                        wait_until(deadline, config.spin_threshold);
                    }
                    Err(error) => break Err(error),
                }
            };
            let execution = match execution {
                Ok(value) => value,
                Err(ExecutionFailure::Vm(source)) => {
                    self.clear_consuming_release_arms(event);
                    return Err(DispatchError::Vm { handler_id, source });
                }
                Err(ExecutionFailure::Inject(source)) => {
                    self.clear_consuming_release_arms(event);
                    return Err(DispatchError::Inject { handler_id, source });
                }
            };
            report.handlers = report.handlers.saturating_add(1);
            report.instructions = report.instructions.saturating_add(execution.instructions);
            report.output_events = report.output_events.saturating_add(execution.output_events);
        }
        self.clear_consuming_release_arms(event);
        Ok(report)
    }

    /// Records an input event and queues all matching handlers without waiting for `sleepUs()`.
    ///
    /// Queue insertion is all-or-nothing. Input held-state is still updated if the queue is full,
    /// because the physical transition already happened before the host observed it.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerFull`] if every matching handler cannot fit.
    pub fn enqueue(
        &mut self,
        event: InputEvent,
        scheduler: &mut ContinuationScheduler,
        now: Instant,
    ) -> Result<u16, SchedulerFull> {
        let modifiers = self.input_state.modifiers_for_event(event);
        let repeated = event.edge == Edge::Down
            && self.input_state.held_for_source(event.device, event.code, event.source);
        self.arm_consuming_releases(event, modifiers, repeated);
        self.input_state.apply(event);
        let handler_count = self
            .handlers
            .matching(event)
            .filter(|handler_id| self.handler_matches(*handler_id, event, modifiers, repeated))
            .count();
        let available = scheduler.capacity.saturating_sub(scheduler.pending.len());
        if handler_count > available {
            self.clear_consuming_release_arms(event);
            return Err(SchedulerFull {
                capacity: scheduler.capacity,
                pending: scheduler.pending.len(),
                requested: handler_count,
            });
        }

        for handler_id in self
            .handlers
            .matching(event)
            .filter(|handler_id| self.handler_matches(*handler_id, event, modifiers, repeated))
        {
            let entry = self.program.handlers[usize::from(handler_id)].entry;
            let mut scratch = VmScratch::new();
            scratch.reset(usize::from(self.program.local_count));
            scheduler.pending.push(Continuation {
                cursor: ExecutionCursor::new(handler_id, event, entry, now),
                scratch,
            });
        }
        self.clear_consuming_release_arms(event);
        Ok(u16::try_from(handler_count).unwrap_or(u16::MAX))
    }

    /// Runs every continuation whose absolute deadline is at or before `now`.
    ///
    /// Zero-duration delays continue in the same poll. A yielded handler remains queued; a
    /// completed or failed handler is removed. The returned next deadline lets a host block its
    /// event loop without polling.
    ///
    /// # Errors
    ///
    /// Returns the first VM or injector error. Other pending continuations remain scheduled.
    pub fn poll_ready<I: Injector>(
        &mut self,
        scheduler: &mut ContinuationScheduler,
        now: Instant,
        injector: &mut I,
    ) -> Result<PollReport, DispatchError<I::Error>> {
        let mut report = PollReport::default();
        let mut index = 0;
        while index < scheduler.pending.len() {
            if scheduler.pending[index].cursor.deadline > now {
                index += 1;
                continue;
            }

            let continuation = &mut scheduler.pending[index];
            let step = execute_until_yield(
                &self.program,
                &self.input_state,
                &mut self.state,
                injector,
                &mut continuation.scratch,
                &mut continuation.cursor,
            );
            match step {
                Ok(ExecutionStep::Complete(execution)) => {
                    report.completed_handlers = report.completed_handlers.saturating_add(1);
                    report.instructions =
                        report.instructions.saturating_add(execution.instructions);
                    report.output_events =
                        report.output_events.saturating_add(execution.output_events);
                    scheduler.pending.remove(index);
                }
                Ok(ExecutionStep::Yield(deadline)) if deadline <= now => {}
                Ok(ExecutionStep::Yield(_)) => index += 1,
                Err(ExecutionFailure::Vm(source)) => {
                    let handler_id = continuation.cursor.handler_id;
                    scheduler.pending.remove(index);
                    return Err(DispatchError::Vm { handler_id, source });
                }
                Err(ExecutionFailure::Inject(source)) => {
                    let handler_id = continuation.cursor.handler_id;
                    scheduler.pending.remove(index);
                    return Err(DispatchError::Inject { handler_id, source });
                }
            }
        }
        report.pending_handlers = u16::try_from(scheduler.pending.len()).unwrap_or(u16::MAX);
        report.next_deadline = scheduler.next_deadline();
        Ok(report)
    }
}

struct ExecutionReport {
    instructions: u32,
    output_events: u32,
}

enum ExecutionStep {
    Complete(ExecutionReport),
    Yield(Instant),
}

enum ExecutionFailure<E> {
    Vm(VmError),
    Inject(E),
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)] // Keeping the opcode dispatch in one match makes VM semantics auditable.
fn execute_until_yield<I: Injector>(
    program: &Program,
    input_state: &InputState,
    state: &mut [i64],
    injector: &mut I,
    scratch: &mut VmScratch,
    cursor: &mut ExecutionCursor,
) -> Result<ExecutionStep, ExecutionFailure<I::Error>> {
    let stack_limit = usize::from(program.stack_limit);

    loop {
        if cursor.instructions >= program.instruction_budget {
            return Err(ExecutionFailure::Vm(VmError::InstructionBudgetExceeded(
                program.instruction_budget,
            )));
        }
        let instruction = *program
            .code
            .get(cursor.pc as usize)
            .ok_or(ExecutionFailure::Vm(VmError::InvalidProgramCounter(cursor.pc)))?;
        cursor.instructions += 1;
        cursor.pc = cursor.pc.saturating_add(1);

        macro_rules! pop {
            () => {
                scratch.pop().map_err(ExecutionFailure::Vm)?
            };
        }
        macro_rules! push {
            ($value:expr) => {
                scratch.push($value, stack_limit).map_err(ExecutionFailure::Vm)?
            };
        }
        macro_rules! binary {
            ($body:expr) => {{
                let right = pop!();
                let left = pop!();
                push!($body(left, right));
            }};
        }

        match instruction.opcode {
            Opcode::Halt => {
                scratch.flush(injector).map_err(ExecutionFailure::Inject)?;
                return Ok(ExecutionStep::Complete(ExecutionReport {
                    instructions: cursor.instructions,
                    output_events: cursor.output_events,
                }));
            }
            Opcode::PushConst => push!(instruction.immediate),
            Opcode::LoadState => push!(state[usize::from(instruction.a)]),
            Opcode::StoreState => state[usize::from(instruction.a)] = pop!(),
            Opcode::StoreStateImm => {
                state[usize::from(instruction.a)] = instruction.immediate;
            }
            Opcode::AddStateImm => {
                let target = &mut state[usize::from(instruction.a)];
                *target = target.wrapping_add(instruction.immediate);
            }
            Opcode::XorStateImm => {
                state[usize::from(instruction.a)] ^= instruction.immediate;
            }
            Opcode::ToggleState => {
                let target = &mut state[usize::from(instruction.a)];
                *target = i64::from(*target == 0);
            }
            Opcode::LoadLocal => push!(scratch.locals[usize::from(instruction.a)]),
            Opcode::StoreLocal => scratch.locals[usize::from(instruction.a)] = pop!(),
            Opcode::Pop => {
                let _ = pop!();
            }
            Opcode::Dup => push!(scratch.peek().map_err(ExecutionFailure::Vm)?),
            Opcode::Add => binary!(i64::wrapping_add),
            Opcode::Sub => binary!(i64::wrapping_sub),
            Opcode::Mul => binary!(i64::wrapping_mul),
            Opcode::Div => {
                let right = pop!();
                let left = pop!();
                if right == 0 {
                    return Err(ExecutionFailure::Vm(VmError::DivisionByZero));
                }
                push!(left.wrapping_div(right));
            }
            Opcode::Mod => {
                let right = pop!();
                let left = pop!();
                if right == 0 {
                    return Err(ExecutionFailure::Vm(VmError::DivisionByZero));
                }
                push!(left.wrapping_rem(right));
            }
            Opcode::Neg => {
                let value = pop!();
                push!(value.wrapping_neg());
            }
            Opcode::Eq => binary!(|left, right| i64::from(left == right)),
            Opcode::Ne => binary!(|left, right| i64::from(left != right)),
            Opcode::Lt => binary!(|left, right| i64::from(left < right)),
            Opcode::Le => binary!(|left, right| i64::from(left <= right)),
            Opcode::Gt => binary!(|left, right| i64::from(left > right)),
            Opcode::Ge => binary!(|left, right| i64::from(left >= right)),
            Opcode::Not => {
                let value = pop!();
                push!(i64::from(value == 0));
            }
            Opcode::BitAnd => binary!(|left, right| left & right),
            Opcode::BitOr => binary!(|left, right| left | right),
            Opcode::BitXor => binary!(|left, right| left ^ right),
            Opcode::Shl => {
                binary!(|left: i64, right: i64| left.wrapping_shl(shift_amount(right)));
            }
            Opcode::Shr => {
                binary!(|left: i64, right: i64| left.wrapping_shr(shift_amount(right)));
            }
            Opcode::Jump => cursor.pc = instruction.b,
            Opcode::JumpIfFalse => {
                if pop!() == 0 {
                    cursor.pc = instruction.b;
                }
            }
            Opcode::LoadInputCode => push!(i64::from(cursor.event.code)),
            Opcode::LoadInputEdge => push!(cursor.event.edge as i64),
            Opcode::LoadInputSource => push!(cursor.event.source as i64),
            Opcode::LoadHeld => {
                let device_bits = instruction.flags & !crate::FLAG_STACK_OPERANDS;
                let device = InputDevice::try_from(device_bits).map_err(|()| {
                    ExecutionFailure::Vm(VmError::InvalidProgramCounter(cursor.pc - 1))
                })?;
                let code = if instruction.flags & crate::FLAG_STACK_OPERANDS != 0 {
                    let raw_code = pop!();
                    u16::try_from(raw_code)
                        .map_err(|_| ExecutionFailure::Vm(VmError::InvalidKeyCode(raw_code)))?
                } else {
                    instruction.a
                };
                push!(i64::from(input_state.held(device, code)));
            }
            Opcode::KeyDown | Opcode::KeyUp => {
                let raw_code = if instruction.flags & crate::FLAG_STACK_OPERANDS != 0 {
                    pop!()
                } else {
                    i64::from(instruction.a)
                };
                let code = u16::try_from(raw_code)
                    .map_err(|_| ExecutionFailure::Vm(VmError::InvalidKeyCode(raw_code)))?;
                let down = instruction.opcode == Opcode::KeyDown;
                scratch
                    .queue(OutputEvent::Key { code, down }, injector)
                    .map_err(ExecutionFailure::Inject)?;
                cursor.output_events = cursor.output_events.saturating_add(1);
            }
            Opcode::MouseDown | Opcode::MouseUp => {
                let raw_button = if instruction.flags & crate::FLAG_STACK_OPERANDS != 0 {
                    pop!()
                } else {
                    i64::from(instruction.a)
                };
                let button = u8::try_from(raw_button)
                    .ok()
                    .and_then(|value| MouseButton::try_from(value).ok())
                    .ok_or(ExecutionFailure::Vm(VmError::InvalidMouseButton(raw_button)))?;
                let down = instruction.opcode == Opcode::MouseDown;
                scratch
                    .queue(OutputEvent::MouseButton { button, down }, injector)
                    .map_err(ExecutionFailure::Inject)?;
                cursor.output_events = cursor.output_events.saturating_add(1);
            }
            Opcode::MouseMove | Opcode::MouseWheel => {
                let (x, y) = if instruction.flags & crate::FLAG_STACK_OPERANDS != 0 {
                    let raw_y = pop!();
                    let raw_x = pop!();
                    let x = i32::try_from(raw_x)
                        .map_err(|_| ExecutionFailure::Vm(VmError::InvalidCoordinate(raw_x)))?;
                    let y = i32::try_from(raw_y)
                        .map_err(|_| ExecutionFailure::Vm(VmError::InvalidCoordinate(raw_y)))?;
                    (x, y)
                } else {
                    unpack_pair(instruction.immediate)
                };
                let output = if instruction.opcode == Opcode::MouseMove {
                    OutputEvent::MouseMove { dx: x, dy: y }
                } else {
                    OutputEvent::MouseWheel { x, y }
                };
                scratch.queue(output, injector).map_err(ExecutionFailure::Inject)?;
                cursor.output_events = cursor.output_events.saturating_add(1);
            }
            Opcode::DelayUs => {
                let stack_operand = instruction.flags & crate::FLAG_STACK_OPERANDS != 0;
                let wide = instruction.flags & crate::FLAG_WIDE_DELAY != 0;
                let raw_delay = if stack_operand {
                    pop!()
                } else if wide {
                    instruction.immediate
                } else {
                    i64::from(instruction.b)
                };
                let scaled_delay = if stack_operand && wide {
                    raw_delay
                        .checked_mul(instruction.immediate)
                        .ok_or(ExecutionFailure::Vm(VmError::InvalidDelay(raw_delay)))?
                } else {
                    raw_delay
                };
                let delay = if wide {
                    u64::try_from(scaled_delay)
                        .map_err(|_| ExecutionFailure::Vm(VmError::InvalidDelay(scaled_delay)))?
                } else {
                    u64::from(
                        u32::try_from(scaled_delay).map_err(|_| {
                            ExecutionFailure::Vm(VmError::InvalidDelay(scaled_delay))
                        })?,
                    )
                };
                let deadline = cursor
                    .deadline
                    .checked_add(Duration::from_micros(delay))
                    .ok_or(ExecutionFailure::Vm(VmError::InvalidDelay(scaled_delay)))?;
                scratch.flush(injector).map_err(ExecutionFailure::Inject)?;
                cursor.deadline = deadline;
                return Ok(ExecutionStep::Yield(cursor.deadline));
            }
        }
    }
}

fn unpack_pair(value: i64) -> (i32, i32) {
    let [x0, x1, x2, x3, y0, y1, y2, y3] = value.to_le_bytes();
    (i32::from_le_bytes([x0, x1, x2, x3]), i32::from_le_bytes([y0, y1, y2, y3]))
}

fn shift_amount(value: i64) -> u32 {
    u32::try_from(value & 63).unwrap_or_default()
}

fn wait_until(deadline: Instant, spin_threshold: Duration) {
    loop {
        let now = Instant::now();
        let Some(remaining) = deadline.checked_duration_since(now) else {
            return;
        };
        if remaining > spin_threshold {
            thread::sleep(remaining.saturating_sub(spin_threshold));
        } else {
            spin_loop();
        }
    }
}

/// Checks bytecode references and fixed runtime resource limits before dispatch.
///
/// # Errors
///
/// Returns [`ProgramError`] for empty programs, invalid entries/jumps/slots, or resource limits
/// outside the fixed VM capacities.
pub fn validate_program(program: &Program) -> Result<(), ProgramError> {
    if program.handlers.is_empty() {
        return Err(ProgramError::NoHandlers);
    }
    if program.code.is_empty() {
        return Err(ProgramError::NoCode);
    }
    if usize::from(program.stack_limit) > MAX_STACK || program.stack_limit == 0 {
        return Err(ProgramError::StackLimitTooLarge(program.stack_limit));
    }
    if usize::from(program.local_count) > MAX_LOCALS {
        return Err(ProgramError::LocalCountTooLarge(program.local_count));
    }
    if program.instruction_budget == 0 {
        return Err(ProgramError::ZeroInstructionBudget);
    }

    for (index, handler) in program.handlers.iter().enumerate() {
        if handler.trigger.gate != crate::NO_STATE_GATE
            && usize::from(handler.trigger.gate) >= program.initial_state.len()
        {
            return Err(ProgramError::InvalidTrigger(handler.trigger));
        }
        if handler.entry as usize >= program.code.len() {
            return Err(ProgramError::InvalidEntry { handler: index, entry: handler.entry });
        }
    }

    for (index, instruction) in program.code.iter().enumerate() {
        match instruction.opcode {
            Opcode::Jump | Opcode::JumpIfFalse if instruction.b as usize >= program.code.len() => {
                return Err(ProgramError::InvalidJump {
                    instruction: index,
                    target: instruction.b,
                });
            }
            Opcode::LoadState
            | Opcode::StoreState
            | Opcode::StoreStateImm
            | Opcode::AddStateImm
            | Opcode::XorStateImm
            | Opcode::ToggleState
                if usize::from(instruction.a) >= program.initial_state.len() =>
            {
                return Err(ProgramError::InvalidStateSlot {
                    instruction: index,
                    slot: instruction.a,
                });
            }
            Opcode::LoadLocal | Opcode::StoreLocal if instruction.a >= program.local_count => {
                return Err(ProgramError::InvalidLocalSlot {
                    instruction: index,
                    slot: instruction.a,
                });
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use core::convert::Infallible;

    use crate::{
        key, Handler, InputSource, Instruction, Opcode, Program, SourceFilter, Trigger,
        FLAG_STACK_OPERANDS, FLAG_WIDE_DELAY,
    };

    use super::*;

    #[derive(Default)]
    struct RecordingInjector(Vec<OutputEvent>);

    impl Injector for RecordingInjector {
        type Error = Infallible;

        fn send(&mut self, events: &[OutputEvent]) -> Result<(), Self::Error> {
            self.0.extend_from_slice(events);
            Ok(())
        }
    }

    fn dispatch_key(
        runtime: &mut Runtime,
        injector: &mut RecordingInjector,
        scratch: &mut VmScratch,
        code: u16,
        edge: Edge,
        source: InputSource,
    ) {
        runtime
            .dispatch(
                InputEvent { device: InputDevice::Keyboard, code, edge, source },
                injector,
                scratch,
            )
            .unwrap();
    }

    #[test]
    fn state_and_branch_persist_between_dispatches() {
        // count += 1; if (count >= 2) tap E;
        let program = Program {
            initial_state: vec![0].into_boxed_slice(),
            handlers: vec![Handler {
                trigger: Trigger {
                    device: InputDevice::Keyboard,
                    code: key::Q,
                    edge: Edge::Down,
                    source: SourceFilter::Physical,
                    flags: 0,
                    modifiers: 0,
                    gate: crate::NO_STATE_GATE,
                },
                entry: 0,
            }]
            .into_boxed_slice(),
            code: vec![
                Instruction::new(Opcode::LoadState).with_a(0),
                Instruction::new(Opcode::PushConst).with_immediate(1),
                Instruction::new(Opcode::Add),
                Instruction::new(Opcode::Dup),
                Instruction::new(Opcode::StoreState).with_a(0),
                Instruction::new(Opcode::PushConst).with_immediate(2),
                Instruction::new(Opcode::Ge),
                Instruction::new(Opcode::JumpIfFalse).with_b(10),
                Instruction::new(Opcode::KeyDown).with_a(key::E),
                Instruction::new(Opcode::KeyUp).with_a(key::E),
                Instruction::new(Opcode::Halt),
            ]
            .into_boxed_slice(),
            local_count: 0,
            stack_limit: 16,
            instruction_budget: 1_000,
        };
        let mut runtime = Runtime::new(program, RuntimeConfig::default()).unwrap();
        let event = InputEvent {
            device: InputDevice::Keyboard,
            code: key::Q,
            edge: Edge::Down,
            source: InputSource::Physical,
        };
        let mut injector = RecordingInjector::default();
        let mut scratch = VmScratch::new();

        runtime.dispatch(event, &mut injector, &mut scratch).unwrap();
        assert!(injector.0.is_empty());
        runtime.dispatch(event, &mut injector, &mut scratch).unwrap();
        assert_eq!(runtime.get_state(0), Some(2));
        assert_eq!(injector.0.len(), 2);
    }

    #[test]
    fn direct_state_opcodes_update_slots_without_vm_stack_traffic() {
        let program = Program {
            initial_state: vec![0, 1, 0].into_boxed_slice(),
            handlers: vec![Handler {
                trigger: Trigger {
                    device: InputDevice::Keyboard,
                    code: key::Q,
                    edge: Edge::Down,
                    source: SourceFilter::Physical,
                    flags: 0,
                    modifiers: 0,
                    gate: crate::NO_STATE_GATE,
                },
                entry: 0,
            }]
            .into_boxed_slice(),
            code: vec![
                Instruction::new(Opcode::AddStateImm).with_a(0).with_immediate(3),
                Instruction::new(Opcode::ToggleState).with_a(1),
                Instruction::new(Opcode::XorStateImm).with_a(2).with_immediate(5),
                Instruction::new(Opcode::StoreStateImm).with_a(0).with_immediate(9),
                Instruction::new(Opcode::Halt),
            ]
            .into_boxed_slice(),
            local_count: 0,
            stack_limit: 1,
            instruction_budget: 100,
        };
        let mut runtime = Runtime::new(program, RuntimeConfig::default()).unwrap();
        let mut injector = RecordingInjector::default();
        let mut scratch = VmScratch::new();

        dispatch_key(
            &mut runtime,
            &mut injector,
            &mut scratch,
            key::Q,
            Edge::Down,
            InputSource::Physical,
        );
        assert_eq!(runtime.state(), &[9, 0, 5]);
    }

    #[test]
    fn invalid_dynamic_delay_does_not_flush_pending_output() {
        for invalid_delay in [-1, i64::from(u32::MAX) + 1] {
            let mut delay = Instruction::new(Opcode::DelayUs);
            delay.flags = FLAG_STACK_OPERANDS;
            let program = Program {
                initial_state: Box::new([]),
                handlers: vec![Handler {
                    trigger: Trigger {
                        device: InputDevice::Keyboard,
                        code: key::Q,
                        edge: Edge::Down,
                        source: SourceFilter::Physical,
                        flags: 0,
                        modifiers: 0,
                        gate: crate::NO_STATE_GATE,
                    },
                    entry: 0,
                }]
                .into_boxed_slice(),
                code: vec![
                    Instruction::new(Opcode::KeyDown).with_a(key::E),
                    Instruction::new(Opcode::PushConst).with_immediate(invalid_delay),
                    delay,
                    Instruction::new(Opcode::KeyUp).with_a(key::E),
                    Instruction::new(Opcode::Halt),
                ]
                .into_boxed_slice(),
                local_count: 0,
                stack_limit: 8,
                instruction_budget: 100,
            };
            let mut runtime = Runtime::new(program, RuntimeConfig::default()).unwrap();
            let event = InputEvent {
                device: InputDevice::Keyboard,
                code: key::Q,
                edge: Edge::Down,
                source: InputSource::Physical,
            };
            let mut injector = RecordingInjector::default();
            let mut scratch = VmScratch::new();

            let error = runtime.dispatch(event, &mut injector, &mut scratch).unwrap_err();
            assert!(matches!(
                error,
                DispatchError::Vm { source: VmError::InvalidDelay(value), .. }
                    if value == invalid_delay
            ));
            assert!(injector.0.is_empty());
        }
    }

    #[test]
    fn continuation_scheduler_yields_without_blocking_and_resumes_at_deadline() {
        let program = Program {
            initial_state: Box::new([]),
            handlers: vec![Handler {
                trigger: Trigger {
                    device: InputDevice::Keyboard,
                    code: key::Q,
                    edge: Edge::Down,
                    source: SourceFilter::Physical,
                    flags: 0,
                    modifiers: 0,
                    gate: crate::NO_STATE_GATE,
                },
                entry: 0,
            }]
            .into_boxed_slice(),
            code: vec![
                Instruction::new(Opcode::KeyDown).with_a(key::E),
                Instruction::new(Opcode::DelayUs).with_b(10_000),
                Instruction::new(Opcode::KeyUp).with_a(key::E),
                Instruction::new(Opcode::Halt),
            ]
            .into_boxed_slice(),
            local_count: 0,
            stack_limit: 8,
            instruction_budget: 100,
        };
        let mut runtime = Runtime::new(program, RuntimeConfig::default()).unwrap();
        let mut scheduler = ContinuationScheduler::default();
        let mut injector = RecordingInjector::default();
        let now = Instant::now();
        let event = InputEvent {
            device: InputDevice::Keyboard,
            code: key::Q,
            edge: Edge::Down,
            source: InputSource::Physical,
        };

        assert_eq!(runtime.enqueue(event, &mut scheduler, now).unwrap(), 1);
        let first = runtime.poll_ready(&mut scheduler, now, &mut injector).unwrap();
        assert_eq!(first.completed_handlers, 0);
        assert_eq!(first.pending_handlers, 1);
        assert_eq!(first.next_deadline, Some(now + Duration::from_millis(10)));
        assert_eq!(injector.0, vec![OutputEvent::Key { code: key::E, down: true }]);

        let early = runtime
            .poll_ready(&mut scheduler, now + Duration::from_micros(9_999), &mut injector)
            .unwrap();
        assert_eq!(early.completed_handlers, 0);
        assert_eq!(injector.0.len(), 1);

        let complete = runtime
            .poll_ready(&mut scheduler, now + Duration::from_millis(10), &mut injector)
            .unwrap();
        assert_eq!(complete.completed_handlers, 1);
        assert_eq!(complete.pending_handlers, 0);
        assert_eq!(complete.instructions, 4);
        assert_eq!(injector.0[1], OutputEvent::Key { code: key::E, down: false });
    }

    #[test]
    fn wide_scaled_delay_supports_hour_units_in_one_opcode() {
        let mut delay = Instruction::new(Opcode::DelayUs).with_immediate(3_600_000_000);
        delay.flags = FLAG_STACK_OPERANDS | FLAG_WIDE_DELAY;
        let program = Program {
            initial_state: vec![2].into_boxed_slice(),
            handlers: vec![Handler {
                trigger: Trigger {
                    device: InputDevice::Keyboard,
                    code: key::Q,
                    edge: Edge::Down,
                    source: SourceFilter::Physical,
                    flags: 0,
                    modifiers: 0,
                    gate: crate::NO_STATE_GATE,
                },
                entry: 0,
            }]
            .into_boxed_slice(),
            code: vec![
                Instruction::new(Opcode::LoadState).with_a(0),
                delay,
                Instruction::new(Opcode::Halt),
            ]
            .into_boxed_slice(),
            local_count: 0,
            stack_limit: 1,
            instruction_budget: 100,
        };
        let mut runtime = Runtime::new(program, RuntimeConfig::default()).unwrap();
        let mut scheduler = ContinuationScheduler::default();
        let mut injector = RecordingInjector::default();
        let now = Instant::now();
        let event = InputEvent {
            device: InputDevice::Keyboard,
            code: key::Q,
            edge: Edge::Down,
            source: InputSource::Physical,
        };

        runtime.enqueue(event, &mut scheduler, now).unwrap();
        let report = runtime.poll_ready(&mut scheduler, now, &mut injector).unwrap();
        assert_eq!(report.next_deadline, Some(now + Duration::from_secs(7_200)));
    }

    #[test]
    fn full_scheduler_rejects_all_new_handlers_but_tracks_input_state() {
        let program = Program {
            initial_state: Box::new([]),
            handlers: vec![Handler {
                trigger: Trigger {
                    device: InputDevice::Keyboard,
                    code: key::Q,
                    edge: Edge::Down,
                    source: SourceFilter::Physical,
                    flags: 0,
                    modifiers: 0,
                    gate: crate::NO_STATE_GATE,
                },
                entry: 0,
            }]
            .into_boxed_slice(),
            code: vec![Instruction::new(Opcode::Halt)].into_boxed_slice(),
            local_count: 0,
            stack_limit: 8,
            instruction_budget: 100,
        };
        let mut runtime = Runtime::new(program, RuntimeConfig::default()).unwrap();
        let mut scheduler = ContinuationScheduler::new(0);
        let event = InputEvent {
            device: InputDevice::Keyboard,
            code: key::Q,
            edge: Edge::Down,
            source: InputSource::Physical,
        };

        let error = runtime.enqueue(event, &mut scheduler, Instant::now()).unwrap_err();
        assert_eq!(error, SchedulerFull { capacity: 0, pending: 0, requested: 1 });
        assert!(runtime.input_state.held(InputDevice::Keyboard, key::Q));
        assert_eq!(scheduler.pending(), 0);
    }

    #[test]
    fn modifier_trigger_context_excludes_itself_but_keeps_the_opposite_side() {
        let mut state = InputState::new();
        let left = InputEvent {
            device: InputDevice::Keyboard,
            code: key::LEFT_CONTROL,
            edge: Edge::Down,
            source: InputSource::Physical,
        };
        assert_eq!(state.modifiers_for_event(left), 0);
        state.apply(left);
        assert_eq!(state.modifiers_for_event(left), 0);
        assert_eq!(state.modifiers_for_event(InputEvent { edge: Edge::Up, ..left }), 0);

        let right = InputEvent { code: key::RIGHT_CONTROL, ..left };
        state.apply(right);
        assert_eq!(state.modifiers_for_event(left), crate::MODIFIER_CONTROL);
    }

    #[test]
    fn consuming_release_trigger_latches_down_context_and_gate() {
        let program = Program {
            initial_state: vec![1].into_boxed_slice(),
            handlers: vec![Handler {
                trigger: Trigger {
                    device: InputDevice::Keyboard,
                    code: key::K,
                    edge: Edge::Up,
                    source: SourceFilter::Physical,
                    flags: crate::TRIGGER_CONSUME | crate::TRIGGER_EXACT_MODIFIERS,
                    modifiers: crate::MODIFIER_CONTROL,
                    gate: 0,
                },
                entry: 0,
            }]
            .into_boxed_slice(),
            code: vec![
                Instruction::new(Opcode::KeyDown).with_a(key::E),
                Instruction::new(Opcode::Halt),
            ]
            .into_boxed_slice(),
            local_count: 0,
            stack_limit: 8,
            instruction_budget: 100,
        };
        let mut runtime = Runtime::new(program, RuntimeConfig::default()).unwrap();
        let mut injector = RecordingInjector::default();
        let mut scratch = VmScratch::new();

        dispatch_key(
            &mut runtime,
            &mut injector,
            &mut scratch,
            key::LEFT_CONTROL,
            Edge::Down,
            InputSource::Physical,
        );
        dispatch_key(
            &mut runtime,
            &mut injector,
            &mut scratch,
            key::K,
            Edge::Down,
            InputSource::Physical,
        );
        assert!(runtime.set_state(0, 0));
        dispatch_key(
            &mut runtime,
            &mut injector,
            &mut scratch,
            key::LEFT_CONTROL,
            Edge::Up,
            InputSource::Physical,
        );
        dispatch_key(
            &mut runtime,
            &mut injector,
            &mut scratch,
            key::K,
            Edge::Up,
            InputSource::Physical,
        );
        assert_eq!(injector.0, vec![OutputEvent::Key { code: key::E, down: true }]);

        dispatch_key(
            &mut runtime,
            &mut injector,
            &mut scratch,
            key::K,
            Edge::Down,
            InputSource::Physical,
        );
        assert!(runtime.set_state(0, 1));
        dispatch_key(
            &mut runtime,
            &mut injector,
            &mut scratch,
            key::K,
            Edge::Up,
            InputSource::Physical,
        );
        assert_eq!(injector.0.len(), 1);
    }

    #[test]
    fn modifier_and_repeat_policy_filters_before_vm_execution() {
        let program = Program {
            initial_state: Box::new([]),
            handlers: vec![Handler {
                trigger: Trigger {
                    device: InputDevice::Keyboard,
                    code: key::K,
                    edge: Edge::Down,
                    source: SourceFilter::Physical,
                    flags: crate::TRIGGER_EXACT_MODIFIERS | crate::TRIGGER_IGNORE_REPEAT,
                    modifiers: crate::MODIFIER_CONTROL,
                    gate: crate::NO_STATE_GATE,
                },
                entry: 0,
            }]
            .into_boxed_slice(),
            code: vec![
                Instruction::new(Opcode::KeyDown).with_a(key::E),
                Instruction::new(Opcode::Halt),
            ]
            .into_boxed_slice(),
            local_count: 0,
            stack_limit: 8,
            instruction_budget: 100,
        };
        let mut runtime = Runtime::new(program, RuntimeConfig::default()).unwrap();
        let mut injector = RecordingInjector::default();
        let mut scratch = VmScratch::new();
        dispatch_key(
            &mut runtime,
            &mut injector,
            &mut scratch,
            key::LEFT_CONTROL,
            Edge::Down,
            InputSource::Physical,
        );
        dispatch_key(
            &mut runtime,
            &mut injector,
            &mut scratch,
            key::K,
            Edge::Down,
            InputSource::Physical,
        );
        dispatch_key(
            &mut runtime,
            &mut injector,
            &mut scratch,
            key::K,
            Edge::Down,
            InputSource::Physical,
        );
        dispatch_key(
            &mut runtime,
            &mut injector,
            &mut scratch,
            key::K,
            Edge::Up,
            InputSource::Physical,
        );
        assert_eq!(injector.0, vec![OutputEvent::Key { code: key::E, down: true }]);

        dispatch_key(
            &mut runtime,
            &mut injector,
            &mut scratch,
            key::LEFT_SHIFT,
            Edge::Down,
            InputSource::Physical,
        );
        dispatch_key(
            &mut runtime,
            &mut injector,
            &mut scratch,
            key::K,
            Edge::Down,
            InputSource::Physical,
        );
        assert_eq!(injector.0.len(), 1);
    }

    #[test]
    fn native_state_gate_filters_before_vm_execution() {
        let program = Program {
            initial_state: vec![0].into_boxed_slice(),
            handlers: vec![Handler {
                trigger: Trigger {
                    device: InputDevice::Keyboard,
                    code: key::K,
                    edge: Edge::Down,
                    source: SourceFilter::Physical,
                    flags: 0,
                    modifiers: 0,
                    gate: 0,
                },
                entry: 0,
            }]
            .into_boxed_slice(),
            code: vec![
                Instruction::new(Opcode::KeyDown).with_a(key::E),
                Instruction::new(Opcode::Halt),
            ]
            .into_boxed_slice(),
            local_count: 0,
            stack_limit: 8,
            instruction_budget: 100,
        };
        let mut runtime = Runtime::new(program, RuntimeConfig::default()).unwrap();
        let mut injector = RecordingInjector::default();
        let mut scratch = VmScratch::new();

        dispatch_key(
            &mut runtime,
            &mut injector,
            &mut scratch,
            key::K,
            Edge::Down,
            InputSource::Physical,
        );
        assert!(injector.0.is_empty());

        assert!(runtime.set_state(0, 1));
        dispatch_key(
            &mut runtime,
            &mut injector,
            &mut scratch,
            key::K,
            Edge::Down,
            InputSource::Physical,
        );
        assert_eq!(injector.0, vec![OutputEvent::Key { code: key::E, down: true }]);
    }
}
