from __future__ import annotations

import json
import re
import shutil
import textwrap
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(textwrap.dedent(content).lstrip("\n"), encoding="utf-8")


def append_once(path: str, marker: str, content: str) -> None:
    target = ROOT / path
    current = target.read_text(encoding="utf-8")
    if marker not in current:
        target.write_text(current.rstrip() + "\n\n" + textwrap.dedent(content).lstrip("\n"), encoding="utf-8")


def add_cargo_dependencies(path: str, dependencies: dict[str, str]) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    if "[dependencies]" not in text:
        text = text.rstrip() + "\n\n[dependencies]\n"
    for name, declaration in dependencies.items():
        if not re.search(rf"(?m)^{re.escape(name)}\s*=", text):
            section = text.index("[dependencies]") + len("[dependencies]")
            end = text.find("\n[", section)
            if end == -1:
                end = len(text)
            text = text[:end].rstrip() + f"\n{name} = {declaration}\n" + text[end:]
    target.write_text(text, encoding="utf-8")


write(
    "crates/rune-core/src/rt_vm.rs",
    r'''
use core::fmt;
use std::{
    hint::spin_loop,
    sync::atomic::{AtomicI64, AtomicU8, Ordering},
    thread,
    time::{Duration, Instant},
};

use crate::{
    Edge, Injector, InputDevice, InputEvent, InputSource, MouseButton, OutputEvent, SourceFilter,
    Trigger,
};

pub const RT_MAX_STACK: usize = 256;
pub const RT_MAX_LOCALS: usize = 256;
pub const RT_MAX_CALL_DEPTH: usize = 32;
pub const RT_MAX_OUTPUT_BATCH: usize = 64;
pub const RT_DEFAULT_INSTRUCTION_BUDGET: u32 = 100_000;

const MAX_KEY_CODE: usize = 256;
const MAX_MOUSE_BUTTON: usize = 8;
const BASE_TRIGGER_SLOTS: usize = MAX_KEY_CODE * 2 + MAX_MOUSE_BUTTON * 2;
const SOURCE_TABLES: usize = 3;
const TRIGGER_SLOTS: usize = BASE_TRIGGER_SLOTS * SOURCE_TABLES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtInstruction {
    PushI64(i64),
    LoadState(u16),
    StoreState(u16),
    LoadLocal(u16),
    StoreLocal(u16),
    LoadEventCode,
    LoadEventEdge,
    LoadEventSource,
    HeldKey(u16),
    HeldMouse(u8),
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Neg,
    LogicalNot,
    BitNot,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Jump(u32),
    JumpIfFalse(u32),
    Call { function: u16, argc: u8 },
    ReturnValue,
    Drop,
    Dup,
    KeyDown(u16),
    KeyUp(u16),
    MouseDown(MouseButton),
    MouseUp(MouseButton),
    MouseMove,
    MouseWheel,
    DelayUs,
    Halt,
}

#[derive(Debug, Clone)]
pub struct RtFunction {
    pub name: Box<str>,
    pub entry: u32,
    pub params: u16,
    pub locals: u16,
}

#[derive(Debug, Clone)]
pub struct RtHandler {
    pub name: Box<str>,
    pub trigger: Trigger,
    pub entry: u32,
    pub locals: u16,
}

#[derive(Debug, Clone)]
pub struct RtModule {
    pub(crate) state_initial: Box<[i64]>,
    pub(crate) functions: Box<[RtFunction]>,
    pub(crate) handlers: Box<[RtHandler]>,
    pub(crate) code: Box<[RtInstruction]>,
}

impl RtModule {
    #[must_use]
    pub fn new(
        state_initial: Vec<i64>,
        functions: Vec<RtFunction>,
        handlers: Vec<RtHandler>,
        code: Vec<RtInstruction>,
    ) -> Self {
        Self {
            state_initial: state_initial.into_boxed_slice(),
            functions: functions.into_boxed_slice(),
            handlers: handlers.into_boxed_slice(),
            code: code.into_boxed_slice(),
        }
    }

    #[must_use]
    pub fn state_count(&self) -> usize {
        self.state_initial.len()
    }

    #[must_use]
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RtExecutionConfig {
    pub spin_threshold: Duration,
    pub instruction_budget: u32,
}

impl Default for RtExecutionConfig {
    fn default() -> Self {
        Self {
            spin_threshold: Duration::from_micros(100),
            instruction_budget: RT_DEFAULT_INSTRUCTION_BUDGET,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RtBuildError {
    EmptyCode,
    TooManyHandlers(usize),
    InvalidTrigger { name: Box<str>, trigger: Trigger },
    InvalidEntry { name: Box<str>, entry: u32 },
    TooManyLocals { name: Box<str>, locals: u16 },
    InvalidFunctionParameters { name: Box<str>, params: u16, locals: u16 },
}

impl fmt::Display for RtBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCode => f.write_str("realtime module has no bytecode"),
            Self::TooManyHandlers(count) => write!(f, "handler count {count} exceeds u16"),
            Self::InvalidTrigger { name, trigger } => {
                write!(f, "handler {name:?} has invalid trigger {trigger:?}")
            }
            Self::InvalidEntry { name, entry } => {
                write!(f, "entry {entry} for {name:?} is outside bytecode")
            }
            Self::TooManyLocals { name, locals } => {
                write!(f, "{name:?} requests {locals} locals; maximum is {RT_MAX_LOCALS}")
            }
            Self::InvalidFunctionParameters { name, params, locals } => write!(
                f,
                "function {name:?} has {params} parameters but only {locals} local slots"
            ),
        }
    }
}

impl std::error::Error for RtBuildError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtFault {
    InstructionBudget,
    InstructionPointer(usize),
    StackOverflow,
    StackUnderflow,
    LocalOverflow,
    InvalidLocal(u16),
    InvalidState(u16),
    InvalidFunction(u16),
    InvalidCallArity { function: u16, expected: u16, actual: u8 },
    CallDepth,
    DivisionByZero,
    ArithmeticOverflow,
    NegativeDelay(i64),
    InvalidJump(u32),
}

impl fmt::Display for RtFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InstructionBudget => f.write_str("realtime instruction budget exhausted"),
            Self::InstructionPointer(ip) => write!(f, "instruction pointer {ip} is invalid"),
            Self::StackOverflow => f.write_str("realtime value stack overflow"),
            Self::StackUnderflow => f.write_str("realtime value stack underflow"),
            Self::LocalOverflow => f.write_str("realtime local stack overflow"),
            Self::InvalidLocal(slot) => write!(f, "local slot {slot} is invalid"),
            Self::InvalidState(slot) => write!(f, "state slot {slot} is invalid"),
            Self::InvalidFunction(function) => write!(f, "function {function} is invalid"),
            Self::InvalidCallArity {
                function,
                expected,
                actual,
            } => write!(
                f,
                "function {function} expected {expected} arguments, received {actual}"
            ),
            Self::CallDepth => f.write_str("realtime call depth exceeded"),
            Self::DivisionByZero => f.write_str("division by zero"),
            Self::ArithmeticOverflow => f.write_str("integer arithmetic overflow"),
            Self::NegativeDelay(value) => write!(f, "negative delay {value} is invalid"),
            Self::InvalidJump(target) => write!(f, "jump target {target} is invalid"),
        }
    }
}

impl std::error::Error for RtFault {}

#[derive(Debug)]
pub enum RtExecutionError<E> {
    Fault(RtFault),
    Injection(E),
}

impl<E: fmt::Display> fmt::Display for RtExecutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fault(fault) => fault.fmt(f),
            Self::Injection(source) => write!(f, "native input injection failed: {source}"),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for RtExecutionError<E> {}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RtDispatchReport {
    pub handlers: u16,
    pub output_events: u32,
    pub instructions: u32,
}

#[derive(Clone, Copy)]
struct Bucket {
    start: u32,
    len: u16,
}

impl Bucket {
    const EMPTY: Self = Self { start: 0, len: 0 };
}

#[derive(Clone, Copy)]
struct CallFrame {
    return_ip: usize,
    previous_base: usize,
    previous_limit: usize,
    previous_local_len: usize,
    stack_base: usize,
}

impl CallFrame {
    const EMPTY: Self = Self {
        return_ip: 0,
        previous_base: 0,
        previous_limit: 0,
        previous_local_len: 0,
        stack_base: 0,
    };
}

pub struct RtScratch {
    stack: [i64; RT_MAX_STACK],
    stack_len: usize,
    locals: [i64; RT_MAX_LOCALS],
    local_len: usize,
    frames: [CallFrame; RT_MAX_CALL_DEPTH],
    frame_len: usize,
    output: [OutputEvent; RT_MAX_OUTPUT_BATCH],
    output_len: usize,
}

impl RtScratch {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            stack: [0; RT_MAX_STACK],
            stack_len: 0,
            locals: [0; RT_MAX_LOCALS],
            local_len: 0,
            frames: [CallFrame::EMPTY; RT_MAX_CALL_DEPTH],
            frame_len: 0,
            output: [OutputEvent::Empty; RT_MAX_OUTPUT_BATCH],
            output_len: 0,
        }
    }

    fn reset(&mut self, locals: usize) -> Result<(), RtFault> {
        if locals > self.locals.len() {
            return Err(RtFault::LocalOverflow);
        }
        self.stack_len = 0;
        self.frame_len = 0;
        self.output_len = 0;
        self.local_len = locals;
        self.locals[..locals].fill(0);
        Ok(())
    }

    fn push(&mut self, value: i64) -> Result<(), RtFault> {
        if self.stack_len == self.stack.len() {
            return Err(RtFault::StackOverflow);
        }
        self.stack[self.stack_len] = value;
        self.stack_len += 1;
        Ok(())
    }

    fn pop(&mut self) -> Result<i64, RtFault> {
        if self.stack_len == 0 {
            return Err(RtFault::StackUnderflow);
        }
        self.stack_len -= 1;
        Ok(self.stack[self.stack_len])
    }

    fn peek(&self) -> Result<i64, RtFault> {
        self.stack_len
            .checked_sub(1)
            .map(|index| self.stack[index])
            .ok_or(RtFault::StackUnderflow)
    }

    fn push_frame(&mut self, frame: CallFrame) -> Result<(), RtFault> {
        if self.frame_len == self.frames.len() {
            return Err(RtFault::CallDepth);
        }
        self.frames[self.frame_len] = frame;
        self.frame_len += 1;
        Ok(())
    }

    fn pop_frame(&mut self) -> Option<CallFrame> {
        self.frame_len = self.frame_len.checked_sub(1)?;
        Some(self.frames[self.frame_len])
    }
}

impl Default for RtScratch {
    fn default() -> Self {
        Self::new()
    }
}

pub struct RtEngine {
    module: RtModule,
    states: Box<[AtomicI64]>,
    held_keys: Box<[AtomicU8]>,
    held_mouse: Box<[AtomicU8]>,
    buckets: Box<[Bucket]>,
    handler_ids: Box<[u16]>,
    config: RtExecutionConfig,
}

impl RtEngine {
    pub fn new(module: RtModule, config: RtExecutionConfig) -> Result<Self, RtBuildError> {
        if module.code.is_empty() {
            return Err(RtBuildError::EmptyCode);
        }
        if module.handlers.len() > usize::from(u16::MAX) {
            return Err(RtBuildError::TooManyHandlers(module.handlers.len()));
        }
        for function in module.functions.iter() {
            validate_entry(&function.name, function.entry, module.code.len())?;
            validate_locals(&function.name, function.locals)?;
            if function.params > function.locals {
                return Err(RtBuildError::InvalidFunctionParameters {
                    name: function.name.clone(),
                    params: function.params,
                    locals: function.locals,
                });
            }
        }

        let mut lists: Vec<Vec<u16>> = (0..TRIGGER_SLOTS).map(|_| Vec::new()).collect();
        for (id, handler) in module.handlers.iter().enumerate() {
            validate_entry(&handler.name, handler.entry, module.code.len())?;
            validate_locals(&handler.name, handler.locals)?;
            let Some(slot) = trigger_slot(handler.trigger) else {
                return Err(RtBuildError::InvalidTrigger {
                    name: handler.name.clone(),
                    trigger: handler.trigger,
                });
            };
            lists[slot].push(id as u16);
        }

        let total_ids = lists.iter().map(Vec::len).sum();
        let mut buckets = vec![Bucket::EMPTY; TRIGGER_SLOTS];
        let mut handler_ids = Vec::with_capacity(total_ids);
        for (slot, ids) in lists.into_iter().enumerate() {
            let start = handler_ids.len();
            handler_ids.extend(ids);
            buckets[slot] = Bucket {
                start: start as u32,
                len: (handler_ids.len() - start) as u16,
            };
        }

        let states = module
            .state_initial
            .iter()
            .copied()
            .map(AtomicI64::new)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let held_keys = (0..MAX_KEY_CODE)
            .map(|_| AtomicU8::new(0))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let held_mouse = (0..MAX_MOUSE_BUTTON)
            .map(|_| AtomicU8::new(0))
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Ok(Self {
            module,
            states,
            held_keys,
            held_mouse,
            buckets: buckets.into_boxed_slice(),
            handler_ids: handler_ids.into_boxed_slice(),
            config,
        })
    }

    #[must_use]
    pub fn state_count(&self) -> usize {
        self.states.len()
    }

    #[must_use]
    pub fn state_get(&self, slot: usize) -> Option<i64> {
        self.states.get(slot).map(|value| value.load(Ordering::Acquire))
    }

    pub fn state_set(&self, slot: usize, value: i64) -> bool {
        let Some(state) = self.states.get(slot) else {
            return false;
        };
        state.store(value, Ordering::Release);
        true
    }

    pub fn dispatch<I: Injector>(
        &self,
        event: InputEvent,
        injector: &mut I,
        scratch: &mut RtScratch,
    ) -> Result<RtDispatchReport, RtExecutionError<I::Error>> {
        self.update_held(event);
        let Some(base) = base_slot(event.device, event.code, event.edge) else {
            return Ok(RtDispatchReport::default());
        };
        let exact_source = match event.source {
            InputSource::Physical => SourceFilter::Physical,
            InputSource::Synthetic => SourceFilter::Synthetic,
        };

        let mut report = RtDispatchReport::default();
        for source in [exact_source, SourceFilter::Any] {
            let ids = self.ids_for_slot(source_slot(base, source));
            for &handler_id in ids {
                let handler = &self.module.handlers[usize::from(handler_id)];
                let handler_report = self.execute_handler(handler, event, injector, scratch)?;
                report.handlers = report.handlers.saturating_add(1);
                report.output_events = report
                    .output_events
                    .saturating_add(handler_report.output_events);
                report.instructions = report.instructions.saturating_add(handler_report.instructions);
            }
        }
        Ok(report)
    }

    fn ids_for_slot(&self, slot: usize) -> &[u16] {
        let bucket = self.buckets[slot];
        let start = bucket.start as usize;
        &self.handler_ids[start..start + usize::from(bucket.len)]
    }

    fn update_held(&self, event: InputEvent) {
        let value = u8::from(event.edge == Edge::Down);
        match event.device {
            InputDevice::Keyboard => {
                if let Some(slot) = self.held_keys.get(usize::from(event.code)) {
                    slot.store(value, Ordering::Release);
                }
            }
            InputDevice::MouseButton => {
                if let Some(slot) = self.held_mouse.get(usize::from(event.code)) {
                    slot.store(value, Ordering::Release);
                }
            }
        }
    }

    fn execute_handler<I: Injector>(
        &self,
        handler: &RtHandler,
        event: InputEvent,
        injector: &mut I,
        scratch: &mut RtScratch,
    ) -> Result<RtDispatchReport, RtExecutionError<I::Error>> {
        scratch
            .reset(usize::from(handler.locals))
            .map_err(RtExecutionError::Fault)?;
        let mut ip = handler.entry as usize;
        let mut local_base = 0_usize;
        let mut local_limit = usize::from(handler.locals);
        let mut deadline = Instant::now();
        let mut report = RtDispatchReport::default();

        loop {
            report.instructions = report.instructions.saturating_add(1);
            if report.instructions > self.config.instruction_budget {
                return Err(RtExecutionError::Fault(RtFault::InstructionBudget));
            }
            let instruction = *self
                .module
                .code
                .get(ip)
                .ok_or(RtExecutionError::Fault(RtFault::InstructionPointer(ip)))?;

            match instruction {
                RtInstruction::PushI64(value) => {
                    scratch.push(value).map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::LoadState(slot) => {
                    let value = self
                        .states
                        .get(usize::from(slot))
                        .ok_or(RtExecutionError::Fault(RtFault::InvalidState(slot)))?
                        .load(Ordering::Acquire);
                    scratch.push(value).map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::StoreState(slot) => {
                    let value = scratch.peek().map_err(RtExecutionError::Fault)?;
                    self.states
                        .get(usize::from(slot))
                        .ok_or(RtExecutionError::Fault(RtFault::InvalidState(slot)))?
                        .store(value, Ordering::Release);
                    ip += 1;
                }
                RtInstruction::LoadLocal(slot) => {
                    let index = local_index(local_base, local_limit, slot)
                        .map_err(RtExecutionError::Fault)?;
                    scratch
                        .push(scratch.locals[index])
                        .map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::StoreLocal(slot) => {
                    let index = local_index(local_base, local_limit, slot)
                        .map_err(RtExecutionError::Fault)?;
                    scratch.locals[index] = scratch.peek().map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::LoadEventCode => {
                    scratch
                        .push(i64::from(event.code))
                        .map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::LoadEventEdge => {
                    scratch
                        .push(event.edge as i64)
                        .map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::LoadEventSource => {
                    scratch
                        .push(event.source as i64)
                        .map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::HeldKey(code) => {
                    let held = self
                        .held_keys
                        .get(usize::from(code))
                        .map_or(0, |slot| slot.load(Ordering::Acquire));
                    scratch
                        .push(i64::from(held != 0))
                        .map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::HeldMouse(button) => {
                    let held = self
                        .held_mouse
                        .get(usize::from(button))
                        .map_or(0, |slot| slot.load(Ordering::Acquire));
                    scratch
                        .push(i64::from(held != 0))
                        .map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::Add => {
                    binary(scratch, i64::wrapping_add).map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::Sub => {
                    binary(scratch, i64::wrapping_sub).map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::Mul => {
                    binary(scratch, i64::wrapping_mul).map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::Div => {
                    let right = scratch.pop().map_err(RtExecutionError::Fault)?;
                    let left = scratch.pop().map_err(RtExecutionError::Fault)?;
                    if right == 0 {
                        return Err(RtExecutionError::Fault(RtFault::DivisionByZero));
                    }
                    let value = left
                        .checked_div(right)
                        .ok_or(RtExecutionError::Fault(RtFault::ArithmeticOverflow))?;
                    scratch.push(value).map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::Rem => {
                    let right = scratch.pop().map_err(RtExecutionError::Fault)?;
                    let left = scratch.pop().map_err(RtExecutionError::Fault)?;
                    if right == 0 {
                        return Err(RtExecutionError::Fault(RtFault::DivisionByZero));
                    }
                    let value = left
                        .checked_rem(right)
                        .ok_or(RtExecutionError::Fault(RtFault::ArithmeticOverflow))?;
                    scratch.push(value).map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::Neg => {
                    let value = scratch.pop().map_err(RtExecutionError::Fault)?;
                    scratch
                        .push(value.wrapping_neg())
                        .map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::LogicalNot => {
                    let value = scratch.pop().map_err(RtExecutionError::Fault)?;
                    scratch
                        .push(i64::from(value == 0))
                        .map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::BitNot => {
                    let value = scratch.pop().map_err(RtExecutionError::Fault)?;
                    scratch.push(!value).map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::BitAnd => {
                    binary(scratch, |left, right| left & right)
                        .map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::BitOr => {
                    binary(scratch, |left, right| left | right)
                        .map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::BitXor => {
                    binary(scratch, |left, right| left ^ right)
                        .map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::Shl => {
                    binary(scratch, |left, right| left.wrapping_shl((right & 63) as u32))
                        .map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::Shr => {
                    binary(scratch, |left, right| left.wrapping_shr((right & 63) as u32))
                        .map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::Eq => {
                    compare(scratch, |left, right| left == right)
                        .map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::Ne => {
                    compare(scratch, |left, right| left != right)
                        .map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::Lt => {
                    compare(scratch, |left, right| left < right)
                        .map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::Le => {
                    compare(scratch, |left, right| left <= right)
                        .map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::Gt => {
                    compare(scratch, |left, right| left > right)
                        .map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::Ge => {
                    compare(scratch, |left, right| left >= right)
                        .map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::Jump(target) => {
                    ip = jump_target(target, self.module.code.len())
                        .map_err(RtExecutionError::Fault)?;
                }
                RtInstruction::JumpIfFalse(target) => {
                    let condition = scratch.pop().map_err(RtExecutionError::Fault)?;
                    if condition == 0 {
                        ip = jump_target(target, self.module.code.len())
                            .map_err(RtExecutionError::Fault)?;
                    } else {
                        ip += 1;
                    }
                }
                RtInstruction::Call { function, argc } => {
                    let function_spec = self
                        .module
                        .functions
                        .get(usize::from(function))
                        .ok_or(RtExecutionError::Fault(RtFault::InvalidFunction(function)))?;
                    if u16::from(argc) != function_spec.params {
                        return Err(RtExecutionError::Fault(RtFault::InvalidCallArity {
                            function,
                            expected: function_spec.params,
                            actual: argc,
                        }));
                    }
                    let argc = usize::from(argc);
                    let stack_base = scratch
                        .stack_len
                        .checked_sub(argc)
                        .ok_or(RtExecutionError::Fault(RtFault::StackUnderflow))?;
                    let new_base = scratch.local_len;
                    let new_len = new_base + usize::from(function_spec.locals);
                    if new_len > scratch.locals.len() {
                        return Err(RtExecutionError::Fault(RtFault::LocalOverflow));
                    }
                    scratch.locals[new_base..new_len].fill(0);
                    for index in 0..argc {
                        scratch.locals[new_base + index] = scratch.stack[stack_base + index];
                    }
                    scratch.stack_len = stack_base;
                    scratch
                        .push_frame(CallFrame {
                            return_ip: ip + 1,
                            previous_base: local_base,
                            previous_limit: local_limit,
                            previous_local_len: scratch.local_len,
                            stack_base,
                        })
                        .map_err(RtExecutionError::Fault)?;
                    scratch.local_len = new_len;
                    local_base = new_base;
                    local_limit = usize::from(function_spec.locals);
                    ip = function_spec.entry as usize;
                }
                RtInstruction::ReturnValue => {
                    let value = scratch.pop().map_err(RtExecutionError::Fault)?;
                    let Some(frame) = scratch.pop_frame() else {
                        flush_output(scratch, injector)?;
                        return Ok(report);
                    };
                    scratch.stack_len = frame.stack_base;
                    scratch.push(value).map_err(RtExecutionError::Fault)?;
                    scratch.local_len = frame.previous_local_len;
                    local_base = frame.previous_base;
                    local_limit = frame.previous_limit;
                    ip = frame.return_ip;
                }
                RtInstruction::Drop => {
                    scratch.pop().map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::Dup => {
                    let value = scratch.peek().map_err(RtExecutionError::Fault)?;
                    scratch.push(value).map_err(RtExecutionError::Fault)?;
                    ip += 1;
                }
                RtInstruction::KeyDown(code) => {
                    push_output(
                        scratch,
                        injector,
                        OutputEvent::Key { code, down: true },
                    )?;
                    report.output_events = report.output_events.saturating_add(1);
                    ip += 1;
                }
                RtInstruction::KeyUp(code) => {
                    push_output(
                        scratch,
                        injector,
                        OutputEvent::Key { code, down: false },
                    )?;
                    report.output_events = report.output_events.saturating_add(1);
                    ip += 1;
                }
                RtInstruction::MouseDown(button) => {
                    push_output(
                        scratch,
                        injector,
                        OutputEvent::MouseButton { button, down: true },
                    )?;
                    report.output_events = report.output_events.saturating_add(1);
                    ip += 1;
                }
                RtInstruction::MouseUp(button) => {
                    push_output(
                        scratch,
                        injector,
                        OutputEvent::MouseButton {
                            button,
                            down: false,
                        },
                    )?;
                    report.output_events = report.output_events.saturating_add(1);
                    ip += 1;
                }
                RtInstruction::MouseMove => {
                    let dy = scratch.pop().map_err(RtExecutionError::Fault)?;
                    let dx = scratch.pop().map_err(RtExecutionError::Fault)?;
                    push_output(
                        scratch,
                        injector,
                        OutputEvent::MouseMove {
                            dx: narrow_i32(dx),
                            dy: narrow_i32(dy),
                        },
                    )?;
                    report.output_events = report.output_events.saturating_add(1);
                    ip += 1;
                }
                RtInstruction::MouseWheel => {
                    let y = scratch.pop().map_err(RtExecutionError::Fault)?;
                    let x = scratch.pop().map_err(RtExecutionError::Fault)?;
                    push_output(
                        scratch,
                        injector,
                        OutputEvent::MouseWheel {
                            x: narrow_i32(x),
                            y: narrow_i32(y),
                        },
                    )?;
                    report.output_events = report.output_events.saturating_add(1);
                    ip += 1;
                }
                RtInstruction::DelayUs => {
                    let micros = scratch.pop().map_err(RtExecutionError::Fault)?;
                    if micros < 0 {
                        return Err(RtExecutionError::Fault(RtFault::NegativeDelay(micros)));
                    }
                    flush_output(scratch, injector)?;
                    deadline = deadline
                        .checked_add(Duration::from_micros(micros as u64))
                        .unwrap_or_else(Instant::now);
                    wait_until(deadline, self.config.spin_threshold);
                    ip += 1;
                }
                RtInstruction::Halt => {
                    flush_output(scratch, injector)?;
                    return Ok(report);
                }
            }
        }
    }
}

fn validate_entry(name: &str, entry: u32, code_len: usize) -> Result<(), RtBuildError> {
    if entry as usize >= code_len {
        return Err(RtBuildError::InvalidEntry {
            name: name.into(),
            entry,
        });
    }
    Ok(())
}

fn validate_locals(name: &str, locals: u16) -> Result<(), RtBuildError> {
    if usize::from(locals) > RT_MAX_LOCALS {
        return Err(RtBuildError::TooManyLocals {
            name: name.into(),
            locals,
        });
    }
    Ok(())
}

fn local_index(base: usize, limit: usize, slot: u16) -> Result<usize, RtFault> {
    let slot_index = usize::from(slot);
    if slot_index >= limit {
        return Err(RtFault::InvalidLocal(slot));
    }
    Ok(base + slot_index)
}

fn binary(
    scratch: &mut RtScratch,
    operation: impl FnOnce(i64, i64) -> i64,
) -> Result<(), RtFault> {
    let right = scratch.pop()?;
    let left = scratch.pop()?;
    scratch.push(operation(left, right))
}

fn compare(
    scratch: &mut RtScratch,
    operation: impl FnOnce(i64, i64) -> bool,
) -> Result<(), RtFault> {
    let right = scratch.pop()?;
    let left = scratch.pop()?;
    scratch.push(i64::from(operation(left, right)))
}

fn jump_target(target: u32, code_len: usize) -> Result<usize, RtFault> {
    let target = target as usize;
    if target >= code_len {
        return Err(RtFault::InvalidJump(target as u32));
    }
    Ok(target)
}

fn narrow_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn push_output<I: Injector>(
    scratch: &mut RtScratch,
    injector: &mut I,
    event: OutputEvent,
) -> Result<(), RtExecutionError<I::Error>> {
    if scratch.output_len == scratch.output.len() {
        flush_output(scratch, injector)?;
    }
    scratch.output[scratch.output_len] = event;
    scratch.output_len += 1;
    Ok(())
}

fn flush_output<I: Injector>(
    scratch: &mut RtScratch,
    injector: &mut I,
) -> Result<(), RtExecutionError<I::Error>> {
    if scratch.output_len != 0 {
        let len = std::mem::replace(&mut scratch.output_len, 0);
        injector
            .send(&scratch.output[..len])
            .map_err(RtExecutionError::Injection)?;
    }
    Ok(())
}

fn wait_until(deadline: Instant, spin_threshold: Duration) {
    loop {
        let now = Instant::now();
        let Some(remaining) = deadline.checked_duration_since(now) else {
            break;
        };
        if remaining > spin_threshold {
            thread::sleep(remaining - spin_threshold);
        } else {
            spin_loop();
        }
    }
}

fn trigger_slot(trigger: Trigger) -> Option<usize> {
    let base = base_slot(trigger.device, trigger.code, trigger.edge)?;
    Some(source_slot(base, trigger.source))
}

fn source_slot(base: usize, source: SourceFilter) -> usize {
    base + BASE_TRIGGER_SLOTS * source as usize
}

fn base_slot(device: InputDevice, code: u16, edge: Edge) -> Option<usize> {
    let edge_offset = edge as usize;
    match device {
        InputDevice::Keyboard if usize::from(code) < MAX_KEY_CODE => {
            Some(edge_offset * MAX_KEY_CODE + usize::from(code))
        }
        InputDevice::MouseButton if usize::from(code) < MAX_MOUSE_BUTTON => {
            Some(MAX_KEY_CODE * 2 + edge_offset * MAX_MOUSE_BUTTON + usize::from(code))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use core::convert::Infallible;

    use super::*;

    #[derive(Default)]
    struct RecordingInjector {
        events: Vec<OutputEvent>,
    }

    impl Injector for RecordingInjector {
        type Error = Infallible;

        fn send(&mut self, events: &[OutputEvent]) -> Result<(), Self::Error> {
            self.events.extend_from_slice(events);
            Ok(())
        }
    }

    #[test]
    fn persistent_state_survives_between_events() {
        let module = RtModule::new(
            vec![0],
            vec![],
            vec![RtHandler {
                name: "counter".into(),
                trigger: Trigger {
                    device: InputDevice::Keyboard,
                    code: 0x14,
                    edge: Edge::Down,
                    source: SourceFilter::Physical,
                },
                entry: 0,
                locals: 0,
            }],
            vec![
                RtInstruction::LoadState(0),
                RtInstruction::PushI64(1),
                RtInstruction::Add,
                RtInstruction::StoreState(0),
                RtInstruction::Drop,
                RtInstruction::LoadState(0),
                RtInstruction::PushI64(3),
                RtInstruction::Ge,
                RtInstruction::JumpIfFalse(12),
                RtInstruction::KeyDown(0x08),
                RtInstruction::KeyUp(0x08),
                RtInstruction::Halt,
                RtInstruction::Halt,
            ],
        );
        let engine = RtEngine::new(module, RtExecutionConfig::default()).unwrap();
        let mut injector = RecordingInjector::default();
        let mut scratch = RtScratch::new();
        let event = InputEvent {
            device: InputDevice::Keyboard,
            code: 0x14,
            edge: Edge::Down,
            source: InputSource::Physical,
        };

        engine.dispatch(event, &mut injector, &mut scratch).unwrap();
        engine.dispatch(event, &mut injector, &mut scratch).unwrap();
        assert!(injector.events.is_empty());
        engine.dispatch(event, &mut injector, &mut scratch).unwrap();

        assert_eq!(engine.state_get(0), Some(3));
        assert_eq!(injector.events.len(), 2);
    }

    #[test]
    fn helper_calls_and_loops_use_fixed_scratch() {
        let module = RtModule::new(
            vec![],
            vec![RtFunction {
                name: "emit".into(),
                entry: 8,
                params: 1,
                locals: 1,
            }],
            vec![RtHandler {
                name: "loop".into(),
                trigger: Trigger {
                    device: InputDevice::Keyboard,
                    code: 0x14,
                    edge: Edge::Down,
                    source: SourceFilter::Physical,
                },
                entry: 0,
                locals: 0,
            }],
            vec![
                RtInstruction::PushI64(2),
                RtInstruction::Call {
                    function: 0,
                    argc: 1,
                },
                RtInstruction::Drop,
                RtInstruction::Halt,
                RtInstruction::Halt,
                RtInstruction::Halt,
                RtInstruction::Halt,
                RtInstruction::Halt,
                RtInstruction::LoadLocal(0),
                RtInstruction::JumpIfFalse(17),
                RtInstruction::KeyDown(0x08),
                RtInstruction::KeyUp(0x08),
                RtInstruction::LoadLocal(0),
                RtInstruction::PushI64(1),
                RtInstruction::Sub,
                RtInstruction::StoreLocal(0),
                RtInstruction::Drop,
                RtInstruction::Jump(8),
                RtInstruction::PushI64(0),
                RtInstruction::ReturnValue,
            ],
        );
        let engine = RtEngine::new(module, RtExecutionConfig::default()).unwrap();
        let mut injector = RecordingInjector::default();
        let mut scratch = RtScratch::new();
        engine
            .dispatch(
                InputEvent {
                    device: InputDevice::Keyboard,
                    code: 0x14,
                    edge: Edge::Down,
                    source: InputSource::Physical,
                },
                &mut injector,
                &mut scratch,
            )
            .unwrap();
        assert_eq!(injector.events.len(), 4);
    }
}
''',
)

write(
    "crates/rune-core/src/rt_wire.rs",
    r'''
use core::fmt;

use serde::Deserialize;

use crate::{
    Edge, InputDevice, MouseButton, RtFunction, RtHandler, RtInstruction, RtModule, SourceFilter,
    Trigger, RT_MAX_LOCALS,
};

pub const RT_WIRE_VERSION: u16 = 2;
const MAX_STATES: usize = 4_096;
const MAX_FUNCTIONS: usize = 4_096;
const MAX_HANDLERS: usize = 4_096;
const MAX_CODE: usize = 1_000_000;

#[derive(Debug)]
pub struct RtDecodeError(Box<str>);

impl RtDecodeError {
    fn new(message: impl Into<Box<str>>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for RtDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for RtDecodeError {}

#[derive(Deserialize)]
struct WireModule {
    version: u16,
    #[serde(default)]
    states: Vec<i64>,
    #[serde(default)]
    functions: Vec<WireFunction>,
    #[serde(default)]
    handlers: Vec<WireHandler>,
    code: Vec<WireInstruction>,
}

#[derive(Deserialize)]
struct WireFunction {
    name: String,
    entry: u32,
    params: u16,
    locals: u16,
}

#[derive(Deserialize)]
struct WireHandler {
    name: String,
    trigger: WireTrigger,
    entry: u32,
    locals: u16,
}

#[derive(Deserialize)]
struct WireTrigger {
    device: u8,
    code: u16,
    edge: u8,
    source: u8,
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum WireInstruction {
    PushI64 { value: i64 },
    LoadState { slot: u16 },
    StoreState { slot: u16 },
    LoadLocal { slot: u16 },
    StoreLocal { slot: u16 },
    LoadEventCode,
    LoadEventEdge,
    LoadEventSource,
    HeldKey { code: u16 },
    HeldMouse { button: u8 },
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Neg,
    LogicalNot,
    BitNot,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Jump { target: u32 },
    JumpIfFalse { target: u32 },
    Call { function: u16, argc: u8 },
    ReturnValue,
    Drop,
    Dup,
    KeyDown { code: u16 },
    KeyUp { code: u16 },
    MouseDown { button: u8 },
    MouseUp { button: u8 },
    MouseMove,
    MouseWheel,
    DelayUs,
    Halt,
}

pub fn decode_rt_module(bytes: &[u8]) -> Result<RtModule, RtDecodeError> {
    let wire: WireModule = serde_json::from_slice(bytes)
        .map_err(|error| RtDecodeError::new(format!("invalid realtime module JSON: {error}")))?;
    if wire.version != RT_WIRE_VERSION {
        return Err(RtDecodeError::new(format!(
            "unsupported realtime module version {}; expected {RT_WIRE_VERSION}",
            wire.version
        )));
    }
    if wire.states.len() > MAX_STATES {
        return Err(RtDecodeError::new("too many persistent state slots"));
    }
    if wire.functions.len() > MAX_FUNCTIONS {
        return Err(RtDecodeError::new("too many realtime functions"));
    }
    if wire.handlers.len() > MAX_HANDLERS {
        return Err(RtDecodeError::new("too many realtime handlers"));
    }
    if wire.code.is_empty() || wire.code.len() > MAX_CODE {
        return Err(RtDecodeError::new("invalid realtime bytecode length"));
    }

    let code_len = wire.code.len();
    let state_len = wire.states.len();
    let function_len = wire.functions.len();
    let mut code = Vec::with_capacity(code_len);
    for instruction in wire.code {
        let decoded = decode_instruction(instruction)?;
        validate_instruction(&decoded, code_len, state_len, function_len)?;
        code.push(decoded);
    }

    let mut functions = Vec::with_capacity(function_len);
    for function in wire.functions {
        validate_entry(function.entry, code_len, "function")?;
        validate_locals(function.locals, "function")?;
        if function.params > function.locals {
            return Err(RtDecodeError::new(format!(
                "function {:?} has more parameters than local slots",
                function.name
            )));
        }
        functions.push(RtFunction {
            name: function.name.into_boxed_str(),
            entry: function.entry,
            params: function.params,
            locals: function.locals,
        });
    }

    let mut handlers = Vec::with_capacity(wire.handlers.len());
    for handler in wire.handlers {
        validate_entry(handler.entry, code_len, "handler")?;
        validate_locals(handler.locals, "handler")?;
        handlers.push(RtHandler {
            name: handler.name.into_boxed_str(),
            trigger: decode_trigger(handler.trigger)?,
            entry: handler.entry,
            locals: handler.locals,
        });
    }

    Ok(RtModule::new(wire.states, functions, handlers, code))
}

fn decode_trigger(trigger: WireTrigger) -> Result<Trigger, RtDecodeError> {
    let device = InputDevice::try_from(trigger.device)
        .map_err(|()| RtDecodeError::new("invalid trigger device"))?;
    let edge = Edge::try_from(trigger.edge)
        .map_err(|()| RtDecodeError::new("invalid trigger edge"))?;
    let source = SourceFilter::try_from(trigger.source)
        .map_err(|()| RtDecodeError::new("invalid trigger source"))?;
    Ok(Trigger {
        device,
        code: trigger.code,
        edge,
        source,
    })
}

fn decode_instruction(instruction: WireInstruction) -> Result<RtInstruction, RtDecodeError> {
    Ok(match instruction {
        WireInstruction::PushI64 { value } => RtInstruction::PushI64(value),
        WireInstruction::LoadState { slot } => RtInstruction::LoadState(slot),
        WireInstruction::StoreState { slot } => RtInstruction::StoreState(slot),
        WireInstruction::LoadLocal { slot } => RtInstruction::LoadLocal(slot),
        WireInstruction::StoreLocal { slot } => RtInstruction::StoreLocal(slot),
        WireInstruction::LoadEventCode => RtInstruction::LoadEventCode,
        WireInstruction::LoadEventEdge => RtInstruction::LoadEventEdge,
        WireInstruction::LoadEventSource => RtInstruction::LoadEventSource,
        WireInstruction::HeldKey { code } => RtInstruction::HeldKey(code),
        WireInstruction::HeldMouse { button } => RtInstruction::HeldMouse(button),
        WireInstruction::Add => RtInstruction::Add,
        WireInstruction::Sub => RtInstruction::Sub,
        WireInstruction::Mul => RtInstruction::Mul,
        WireInstruction::Div => RtInstruction::Div,
        WireInstruction::Rem => RtInstruction::Rem,
        WireInstruction::Neg => RtInstruction::Neg,
        WireInstruction::LogicalNot => RtInstruction::LogicalNot,
        WireInstruction::BitNot => RtInstruction::BitNot,
        WireInstruction::BitAnd => RtInstruction::BitAnd,
        WireInstruction::BitOr => RtInstruction::BitOr,
        WireInstruction::BitXor => RtInstruction::BitXor,
        WireInstruction::Shl => RtInstruction::Shl,
        WireInstruction::Shr => RtInstruction::Shr,
        WireInstruction::Eq => RtInstruction::Eq,
        WireInstruction::Ne => RtInstruction::Ne,
        WireInstruction::Lt => RtInstruction::Lt,
        WireInstruction::Le => RtInstruction::Le,
        WireInstruction::Gt => RtInstruction::Gt,
        WireInstruction::Ge => RtInstruction::Ge,
        WireInstruction::Jump { target } => RtInstruction::Jump(target),
        WireInstruction::JumpIfFalse { target } => RtInstruction::JumpIfFalse(target),
        WireInstruction::Call { function, argc } => RtInstruction::Call { function, argc },
        WireInstruction::ReturnValue => RtInstruction::ReturnValue,
        WireInstruction::Drop => RtInstruction::Drop,
        WireInstruction::Dup => RtInstruction::Dup,
        WireInstruction::KeyDown { code } => RtInstruction::KeyDown(code),
        WireInstruction::KeyUp { code } => RtInstruction::KeyUp(code),
        WireInstruction::MouseDown { button } => RtInstruction::MouseDown(
            MouseButton::try_from(button)
                .map_err(|()| RtDecodeError::new("invalid mouse button"))?,
        ),
        WireInstruction::MouseUp { button } => RtInstruction::MouseUp(
            MouseButton::try_from(button)
                .map_err(|()| RtDecodeError::new("invalid mouse button"))?,
        ),
        WireInstruction::MouseMove => RtInstruction::MouseMove,
        WireInstruction::MouseWheel => RtInstruction::MouseWheel,
        WireInstruction::DelayUs => RtInstruction::DelayUs,
        WireInstruction::Halt => RtInstruction::Halt,
    })
}

fn validate_instruction(
    instruction: &RtInstruction,
    code_len: usize,
    state_len: usize,
    function_len: usize,
) -> Result<(), RtDecodeError> {
    match *instruction {
        RtInstruction::LoadState(slot) | RtInstruction::StoreState(slot)
            if usize::from(slot) >= state_len =>
        {
            Err(RtDecodeError::new(format!("state slot {slot} is invalid")))
        }
        RtInstruction::Jump(target) | RtInstruction::JumpIfFalse(target)
            if target as usize >= code_len =>
        {
            Err(RtDecodeError::new(format!("jump target {target} is invalid")))
        }
        RtInstruction::Call { function, .. } if usize::from(function) >= function_len => Err(
            RtDecodeError::new(format!("function id {function} is invalid")),
        ),
        RtInstruction::HeldKey(code)
        | RtInstruction::KeyDown(code)
        | RtInstruction::KeyUp(code)
            if usize::from(code) >= 256 =>
        {
            Err(RtDecodeError::new(format!("key code {code} is invalid")))
        }
        RtInstruction::HeldMouse(button) if usize::from(button) >= 8 => Err(
            RtDecodeError::new(format!("mouse button {button} is invalid")),
        ),
        _ => Ok(()),
    }
}

fn validate_entry(entry: u32, code_len: usize, kind: &str) -> Result<(), RtDecodeError> {
    if entry as usize >= code_len {
        return Err(RtDecodeError::new(format!(
            "{kind} entry {entry} is outside bytecode"
        )));
    }
    Ok(())
}

fn validate_locals(locals: u16, kind: &str) -> Result<(), RtDecodeError> {
    if usize::from(locals) > RT_MAX_LOCALS {
        return Err(RtDecodeError::new(format!(
            "{kind} requests {locals} locals; maximum is {RT_MAX_LOCALS}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_versioned_module() {
        let json = br#"{
          "version": 2,
          "states": [0],
          "functions": [],
          "handlers": [{
            "name": "q",
            "trigger": {"device": 0, "code": 20, "edge": 0, "source": 0},
            "entry": 0,
            "locals": 0
          }],
          "code": [
            {"op": "load_state", "slot": 0},
            {"op": "drop"},
            {"op": "halt"}
          ]
        }"#;
        let module = decode_rt_module(json).unwrap();
        assert_eq!(module.state_count(), 1);
        assert_eq!(module.handler_count(), 1);
    }
}
''',
)

write(
    "crates/rune-core/src/global_rt.rs",
    r'''
use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, OnceLock,
    },
};

use arc_swap::ArcSwapOption;

use crate::{
    Injector, InputEvent, RtBuildError, RtEngine, RtExecutionConfig, RtModule, RtScratch,
};

static DISPATCH_FAILURES: AtomicU64 = AtomicU64::new(0);

fn engine_slot() -> &'static ArcSwapOption<RtEngine> {
    static SLOT: OnceLock<ArcSwapOption<RtEngine>> = OnceLock::new();
    SLOT.get_or_init(ArcSwapOption::empty)
}

thread_local! {
    static SCRATCH: RefCell<RtScratch> = RefCell::new(RtScratch::new());
}

pub fn install_rt(module: RtModule) -> Result<(), RtBuildError> {
    let engine = RtEngine::new(module, RtExecutionConfig::default())?;
    engine_slot().store(Some(Arc::new(engine)));
    DISPATCH_FAILURES.store(0, Ordering::Release);
    Ok(())
}

pub fn clear_rt() {
    engine_slot().store(None);
}

#[must_use]
pub fn rt_loaded() -> bool {
    engine_slot().load().is_some()
}

#[must_use]
pub fn rt_state_get(slot: usize) -> Option<i64> {
    engine_slot().load().as_ref()?.state_get(slot)
}

pub fn rt_state_set(slot: usize, value: i64) -> bool {
    engine_slot()
        .load()
        .as_ref()
        .is_some_and(|engine| engine.state_set(slot, value))
}

#[must_use]
pub fn rt_dispatch_failures() -> u64 {
    DISPATCH_FAILURES.load(Ordering::Acquire)
}

pub(crate) fn dispatch_installed_rt_best_effort<I: Injector>(
    event: InputEvent,
    injector: &mut I,
) {
    let engine = engine_slot().load();
    let Some(engine) = engine.as_ref() else {
        return;
    };
    let failed = SCRATCH.with(|scratch| {
        let Ok(mut scratch) = scratch.try_borrow_mut() else {
            return true;
        };
        engine.dispatch(event, injector, &mut scratch).is_err()
    });
    if failed {
        DISPATCH_FAILURES.fetch_add(1, Ordering::Relaxed);
    }
}
''',
)

add_cargo_dependencies(
    "crates/rune-core/Cargo.toml",
    {
        "arc-swap": '"1.7"',
        "serde": '{ version = "1", features = ["derive"] }',
        "serde_json": '"1"',
    },
)

append_once(
    "crates/rune-core/src/lib.rs",
    "mod rt_vm;",
    r'''
mod global_rt;
mod rt_vm;
mod rt_wire;

pub use global_rt::{
    clear_rt, install_rt, rt_dispatch_failures, rt_loaded, rt_state_get, rt_state_set,
};
pub use rt_vm::{
    RtBuildError, RtDispatchReport, RtEngine, RtExecutionConfig, RtExecutionError, RtFault,
    RtFunction, RtHandler, RtInstruction, RtModule, RtScratch, RT_DEFAULT_INSTRUCTION_BUDGET,
    RT_MAX_CALL_DEPTH, RT_MAX_LOCALS, RT_MAX_OUTPUT_BATCH, RT_MAX_STACK,
};
pub use rt_wire::{decode_rt_module, RtDecodeError, RT_WIRE_VERSION};
''',
)

executor = ROOT / "crates/rune-core/src/executor.rs"
executor_text = executor.read_text(encoding="utf-8")
needle = "        let mut report = DispatchReport::default();\n"
insert = needle + "        crate::global_rt::dispatch_installed_rt_best_effort(event, injector);\n"
if "dispatch_installed_rt_best_effort(event, injector)" not in executor_text:
    if needle not in executor_text:
        raise RuntimeError("unable to locate Engine::dispatch insertion point")
    executor_text = executor_text.replace(needle, insert, 1)
    executor.write_text(executor_text, encoding="utf-8")

append_once(
    "crates/rune-native/src/lib.rs",
    "pub unsafe extern \"C\" fn rune_rt_load",
    r'''

/// Load a stateful realtime module encoded by `@rune/sdk`.
///
/// The module is decoded and installed on the control plane. Input events then execute it
/// entirely inside `rune-core`; no JavaScript callback is made on the input hot path.
#[no_mangle]
pub unsafe extern "C" fn rune_rt_load(bytes: *const u8, len: usize) -> i32 {
    if bytes.is_null() || len == 0 {
        return -1;
    }
    let bytes = unsafe { std::slice::from_raw_parts(bytes, len) };
    let module = match rune_core::decode_rt_module(bytes) {
        Ok(module) => module,
        Err(_) => return -2,
    };
    match rune_core::install_rt(module) {
        Ok(()) => 0,
        Err(_) => -3,
    }
}

#[no_mangle]
pub extern "C" fn rune_rt_clear() {
    rune_core::clear_rt();
}

#[no_mangle]
pub extern "C" fn rune_rt_is_loaded() -> u8 {
    u8::from(rune_core::rt_loaded())
}

#[no_mangle]
pub unsafe extern "C" fn rune_rt_state_get(slot: usize, output: *mut i64) -> i32 {
    if output.is_null() {
        return -1;
    }
    let Some(value) = rune_core::rt_state_get(slot) else {
        return -2;
    };
    unsafe { output.write(value) };
    0
}

#[no_mangle]
pub extern "C" fn rune_rt_state_set(slot: usize, value: i64) -> i32 {
    if rune_core::rt_state_set(slot, value) {
        0
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C" fn rune_rt_dispatch_failures() -> u64 {
    rune_core::rt_dispatch_failures()
}
''',
)

write(
    "packages/sdk/src/rt.ts",
    r'''
import * as ts from "typescript";

export const Key = {
  A: 0x04, B: 0x05, C: 0x06, D: 0x07, E: 0x08, F: 0x09, G: 0x0a,
  H: 0x0b, I: 0x0c, J: 0x0d, K: 0x0e, L: 0x0f, M: 0x10, N: 0x11,
  O: 0x12, P: 0x13, Q: 0x14, R: 0x15, S: 0x16, T: 0x17, U: 0x18,
  V: 0x19, W: 0x1a, X: 0x1b, Y: 0x1c, Z: 0x1d,
  Digit1: 0x1e, Digit2: 0x1f, Digit3: 0x20, Digit4: 0x21, Digit5: 0x22,
  Digit6: 0x23, Digit7: 0x24, Digit8: 0x25, Digit9: 0x26, Digit0: 0x27,
  Enter: 0x28, Escape: 0x29, Backspace: 0x2a, Tab: 0x2b, Space: 0x2c,
  F1: 0x3a, F2: 0x3b, F3: 0x3c, F4: 0x3d, F5: 0x3e, F6: 0x3f,
  F7: 0x40, F8: 0x41, F9: 0x42, F10: 0x43, F11: 0x44, F12: 0x45,
  ArrowRight: 0x4f, ArrowLeft: 0x50, ArrowDown: 0x51, ArrowUp: 0x52,
  LeftControl: 0xe0, LeftShift: 0xe1, LeftAlt: 0xe2, LeftMeta: 0xe3,
  RightControl: 0xe4, RightShift: 0xe5, RightAlt: 0xe6, RightMeta: 0xe7,
} as const;
export type Key = (typeof Key)[keyof typeof Key];

export const MouseButton = {
  Left: 0,
  Right: 1,
  Middle: 2,
  Back: 3,
  Forward: 4,
} as const;
export type MouseButton = (typeof MouseButton)[keyof typeof MouseButton];

export type RtSource = "physical" | "synthetic" | "any";

export interface RtEvent {
  readonly code: number;
  readonly edge: 0 | 1;
  readonly source: 0 | 1;
}

export interface RtHandlerOptions {
  source?: RtSource;
}

function compileOnly(): never {
  throw new Error("Rune realtime intrinsics can only be used inside rt.load/compileRt");
}

export const on = {
  keyDown(_key: Key, _handler: (event: RtEvent) => void, _options?: RtHandlerOptions): void {
    compileOnly();
  },
  keyUp(_key: Key, _handler: (event: RtEvent) => void, _options?: RtHandlerOptions): void {
    compileOnly();
  },
  mouseDown(
    _button: MouseButton,
    _handler: (event: RtEvent) => void,
    _options?: RtHandlerOptions,
  ): void {
    compileOnly();
  },
  mouseUp(
    _button: MouseButton,
    _handler: (event: RtEvent) => void,
    _options?: RtHandlerOptions,
  ): void {
    compileOnly();
  },
};

export const key = {
  down(_key: Key): void { compileOnly(); },
  up(_key: Key): void { compileOnly(); },
  tap(_key: Key): void { compileOnly(); },
};

export const mouse = {
  down(_button: MouseButton): void { compileOnly(); },
  up(_button: MouseButton): void { compileOnly(); },
  click(_button: MouseButton): void { compileOnly(); },
  move(_dx: number, _dy: number): void { compileOnly(); },
  wheel(_x: number, _y: number): void { compileOnly(); },
};

export const delay = {
  us(_microseconds: number): void { compileOnly(); },
};

export function held(_input: Key | MouseButton): boolean {
  return compileOnly();
}

export type RtInstruction =
  | { op: "push_i64"; value: number }
  | { op: "load_state" | "store_state" | "load_local" | "store_local"; slot: number }
  | { op: "held_key" | "key_down" | "key_up"; code: number }
  | { op: "held_mouse" | "mouse_down" | "mouse_up"; button: number }
  | { op: "jump" | "jump_if_false"; target: number }
  | { op: "call"; function: number; argc: number }
  | {
      op:
        | "load_event_code" | "load_event_edge" | "load_event_source"
        | "add" | "sub" | "mul" | "div" | "rem" | "neg"
        | "logical_not" | "bit_not" | "bit_and" | "bit_or" | "bit_xor"
        | "shl" | "shr" | "eq" | "ne" | "lt" | "le" | "gt" | "ge"
        | "return_value" | "drop" | "dup" | "mouse_move" | "mouse_wheel"
        | "delay_us" | "halt";
    };

export interface RtFunctionSpec {
  name: string;
  entry: number;
  params: number;
  locals: number;
}

export interface RtHandlerSpec {
  name: string;
  trigger: { device: 0 | 1; code: number; edge: 0 | 1; source: 0 | 1 | 2 };
  entry: number;
  locals: number;
}

export interface RtModuleSpec {
  version: 2;
  states: number[];
  stateNames: string[];
  functions: RtFunctionSpec[];
  handlers: RtHandlerSpec[];
  code: RtInstruction[];
}

export class RtCompileError extends Error {
  constructor(message: string, node?: ts.Node) {
    const location = node?.getSourceFile();
    const offset = node && location ? location.getLineAndCharacterOfPosition(node.getStart()) : undefined;
    super(offset ? `${message} (${offset.line + 1}:${offset.character + 1})` : message);
    this.name = "RtCompileError";
  }
}

interface PendingFunction {
  id: number;
  node: ts.FunctionDeclaration;
}

interface PendingHandler {
  name: string;
  device: 0 | 1;
  code: number;
  edge: 0 | 1;
  source: 0 | 1 | 2;
  node: ts.ArrowFunction | ts.FunctionExpression;
}

export function compileRt(definition: () => void): RtModuleSpec {
  return new ModuleCompiler(definition).compile();
}

export function encodeRtModule(module: RtModuleSpec): Uint8Array {
  return new TextEncoder().encode(JSON.stringify(module));
}

class ModuleCompiler {
  readonly code: RtInstruction[] = [];
  readonly states: number[] = [];
  readonly stateNames: string[] = [];
  readonly stateSlots = new Map<string, number>();
  readonly constants = new Map<string, number>();
  readonly functions = new Map<string, PendingFunction>();
  readonly pendingHandlers: PendingHandler[] = [];

  private readonly root: ts.ArrowFunction | ts.FunctionExpression;

  constructor(definition: () => void) {
    this.root = parseDefinition(definition);
  }

  compile(): RtModuleSpec {
    this.collectTopLevel();

    const functionSpecs: RtFunctionSpec[] = [];
    for (const [name, pending] of [...this.functions.entries()].sort((a, b) => a[1].id - b[1].id)) {
      const entry = this.code.length;
      const compiler = new FunctionCompiler(this, pending.node, "function");
      compiler.compileBody(pending.node.body);
      compiler.emit({ op: "push_i64", value: 0 });
      compiler.emit({ op: "return_value" });
      functionSpecs.push({
        name,
        entry,
        params: pending.node.parameters.length,
        locals: compiler.localCount,
      });
    }

    const handlerSpecs: RtHandlerSpec[] = [];
    for (const pending of this.pendingHandlers) {
      const entry = this.code.length;
      const compiler = new FunctionCompiler(this, pending.node, "handler");
      compiler.compileBody(pending.node.body);
      compiler.emit({ op: "halt" });
      handlerSpecs.push({
        name: pending.name,
        trigger: {
          device: pending.device,
          code: pending.code,
          edge: pending.edge,
          source: pending.source,
        },
        entry,
        locals: compiler.localCount,
      });
    }

    if (this.code.length === 0) {
      this.code.push({ op: "halt" });
    }

    return {
      version: 2,
      states: this.states,
      stateNames: this.stateNames,
      functions: functionSpecs,
      handlers: handlerSpecs,
      code: this.code,
    };
  }

  private collectTopLevel(): void {
    const body = asBlock(this.root.body);
    let functionId = 0;
    for (const statement of body.statements) {
      if (ts.isVariableStatement(statement)) {
        const isConst = (statement.declarationList.flags & ts.NodeFlags.Const) !== 0;
        for (const declaration of statement.declarationList.declarations) {
          const name = identifierName(declaration.name, "realtime top-level variables");
          if (!declaration.initializer) {
            throw new RtCompileError(`top-level variable ${name} needs a constant initializer`, declaration);
          }
          const value = this.constant(declaration.initializer);
          this.assertGlobalNameFree(name, declaration);
          if (isConst) {
            this.constants.set(name, value);
          } else {
            const slot = this.states.length;
            this.stateSlots.set(name, slot);
            this.stateNames.push(name);
            this.states.push(value);
          }
        }
        continue;
      }
      if (ts.isFunctionDeclaration(statement)) {
        if (!statement.name || !statement.body) {
          throw new RtCompileError("realtime helper functions need a name and body", statement);
        }
        const name = statement.name.text;
        this.assertGlobalNameFree(name, statement);
        this.functions.set(name, { id: functionId++, node: statement });
        continue;
      }
      if (ts.isExpressionStatement(statement) && ts.isStringLiteral(statement.expression)) {
        continue;
      }
      if (ts.isExpressionStatement(statement) && ts.isCallExpression(statement.expression)) {
        this.collectHandler(statement.expression);
        continue;
      }
      throw new RtCompileError(
        "the realtime module top level only accepts let/const, function declarations, and on.* handlers",
        statement,
      );
    }
  }

  private collectHandler(call: ts.CallExpression): void {
    const name = propertyChain(call.expression);
    const trigger = {
      "on.keyDown": { device: 0 as const, edge: 0 as const },
      "on.keyUp": { device: 0 as const, edge: 1 as const },
      "on.mouseDown": { device: 1 as const, edge: 0 as const },
      "on.mouseUp": { device: 1 as const, edge: 1 as const },
    }[name];
    if (!trigger) {
      throw new RtCompileError(`unsupported top-level call ${name}`, call);
    }
    if (call.arguments.length < 2 || call.arguments.length > 3) {
      throw new RtCompileError(`${name} expects input, handler, and optional options`, call);
    }
    const code = this.constant(call.arguments[0]);
    const handler = unwrapExpression(call.arguments[1]);
    if (!ts.isArrowFunction(handler) && !ts.isFunctionExpression(handler)) {
      throw new RtCompileError(`${name} handler must be an inline function`, handler);
    }
    const source = parseSource(call.arguments[2]);
    this.pendingHandlers.push({
      name: `${name.slice(3)}_${code}_${this.pendingHandlers.length}`,
      device: trigger.device,
      code,
      edge: trigger.edge,
      source,
      node: handler,
    });
  }

  functionId(name: string, node: ts.Node): number {
    const functionSpec = this.functions.get(name);
    if (!functionSpec) {
      throw new RtCompileError(`unknown realtime helper ${name}`, node);
    }
    return functionSpec.id;
  }

  constant(node: ts.Expression): number {
    const expression = unwrapExpression(node);
    if (ts.isNumericLiteral(expression)) {
      return safeInteger(Number(expression.text), expression);
    }
    if (expression.kind === ts.SyntaxKind.TrueKeyword) return 1;
    if (expression.kind === ts.SyntaxKind.FalseKeyword) return 0;
    if (ts.isIdentifier(expression)) {
      const value = this.constants.get(expression.text);
      if (value === undefined) {
        throw new RtCompileError(`${expression.text} is not a compile-time constant`, expression);
      }
      return value;
    }
    if (ts.isPropertyAccessExpression(expression)) {
      const owner = propertyChain(expression.expression);
      const table = owner === "Key" ? Key : owner === "MouseButton" ? MouseButton : undefined;
      if (!table || !(expression.name.text in table)) {
        throw new RtCompileError(`${propertyChain(expression)} is not a Rune constant`, expression);
      }
      return table[expression.name.text as keyof typeof table];
    }
    if (ts.isPrefixUnaryExpression(expression)) {
      const value = this.constant(expression.operand);
      switch (expression.operator) {
        case ts.SyntaxKind.PlusToken: return value;
        case ts.SyntaxKind.MinusToken: return safeInteger(-value, expression);
        case ts.SyntaxKind.TildeToken: return ~value;
        case ts.SyntaxKind.ExclamationToken: return Number(value === 0);
        default: throw new RtCompileError("unsupported constant unary operator", expression);
      }
    }
    if (ts.isBinaryExpression(expression)) {
      const left = this.constant(expression.left);
      const right = this.constant(expression.right);
      switch (expression.operatorToken.kind) {
        case ts.SyntaxKind.PlusToken: return safeInteger(left + right, expression);
        case ts.SyntaxKind.MinusToken: return safeInteger(left - right, expression);
        case ts.SyntaxKind.AsteriskToken: return safeInteger(left * right, expression);
        case ts.SyntaxKind.SlashToken: return safeInteger(Math.trunc(left / right), expression);
        case ts.SyntaxKind.PercentToken: return safeInteger(left % right, expression);
        case ts.SyntaxKind.AmpersandToken: return left & right;
        case ts.SyntaxKind.BarToken: return left | right;
        case ts.SyntaxKind.CaretToken: return left ^ right;
        case ts.SyntaxKind.LessThanLessThanToken: return left << right;
        case ts.SyntaxKind.GreaterThanGreaterThanToken: return left >> right;
        default: throw new RtCompileError("unsupported constant binary operator", expression);
      }
    }
    throw new RtCompileError("expected a compile-time integer constant", expression);
  }

  private assertGlobalNameFree(name: string, node: ts.Node): void {
    if (this.stateSlots.has(name) || this.constants.has(name) || this.functions.has(name)) {
      throw new RtCompileError(`duplicate realtime top-level name ${name}`, node);
    }
  }
}

class FunctionCompiler {
  private readonly locals = new Map<string, number>();
  private readonly breakStack: number[][] = [];
  private readonly continueStack: number[][] = [];
  private readonly kind: "function" | "handler";
  private readonly eventName?: string;

  constructor(
    private readonly module: ModuleCompiler,
    node: ts.FunctionLikeDeclaration,
    kind: "function" | "handler",
  ) {
    this.kind = kind;
    const parameters = node.parameters;
    if (kind === "handler" && parameters.length > 1) {
      throw new RtCompileError("realtime handlers accept zero or one event parameter", node);
    }
    if (kind === "handler" && parameters[0]) {
      this.eventName = identifierName(parameters[0].name, "handler event parameter");
    } else {
      for (const parameter of parameters) {
        this.allocateLocal(identifierName(parameter.name, "function parameter"), parameter);
      }
    }
  }

  get localCount(): number {
    return this.locals.size;
  }

  emit(instruction: RtInstruction): number {
    const index = this.module.code.length;
    this.module.code.push(instruction);
    return index;
  }

  compileBody(body: ts.ConciseBody): void {
    if (ts.isBlock(body)) {
      this.compileStatements(body.statements);
    } else {
      const hasValue = this.compileExpression(body);
      if (this.kind === "function") {
        if (!hasValue) this.emit({ op: "push_i64", value: 0 });
        this.emit({ op: "return_value" });
      } else {
        if (hasValue) this.emit({ op: "drop" });
        this.emit({ op: "halt" });
      }
    }
  }

  private compileStatements(statements: ts.NodeArray<ts.Statement> | readonly ts.Statement[]): void {
    for (const statement of statements) this.compileStatement(statement);
  }

  private compileStatement(statement: ts.Statement): void {
    if (ts.isBlock(statement)) {
      this.compileStatements(statement.statements);
      return;
    }
    if (ts.isVariableStatement(statement)) {
      for (const declaration of statement.declarationList.declarations) {
        const name = identifierName(declaration.name, "local variable");
        const slot = this.allocateLocal(name, declaration);
        if (declaration.initializer) {
          if (!this.compileExpression(declaration.initializer)) {
            throw new RtCompileError("local initializer must produce a value", declaration.initializer);
          }
          this.emit({ op: "store_local", slot });
          this.emit({ op: "drop" });
        }
      }
      return;
    }
    if (ts.isExpressionStatement(statement)) {
      if (this.compileExpression(statement.expression)) this.emit({ op: "drop" });
      return;
    }
    if (ts.isIfStatement(statement)) {
      this.requireValue(statement.expression);
      const jumpFalse = this.emit({ op: "jump_if_false", target: 0 });
      this.compileStatement(statement.thenStatement);
      if (statement.elseStatement) {
        const jumpEnd = this.emit({ op: "jump", target: 0 });
        this.patch(jumpFalse, this.module.code.length);
        this.compileStatement(statement.elseStatement);
        this.patch(jumpEnd, this.module.code.length);
      } else {
        this.patch(jumpFalse, this.module.code.length);
      }
      return;
    }
    if (ts.isWhileStatement(statement)) {
      const condition = this.module.code.length;
      this.requireValue(statement.expression);
      const jumpEnd = this.emit({ op: "jump_if_false", target: 0 });
      this.beginLoop();
      this.compileStatement(statement.statement);
      this.patchContinues(condition);
      this.emit({ op: "jump", target: condition });
      const end = this.module.code.length;
      this.patch(jumpEnd, end);
      this.endLoop(end);
      return;
    }
    if (ts.isDoStatement(statement)) {
      const body = this.module.code.length;
      this.beginLoop();
      this.compileStatement(statement.statement);
      const condition = this.module.code.length;
      this.patchContinues(condition);
      this.requireValue(statement.expression);
      const jumpEnd = this.emit({ op: "jump_if_false", target: 0 });
      this.emit({ op: "jump", target: body });
      const end = this.module.code.length;
      this.patch(jumpEnd, end);
      this.endLoop(end);
      return;
    }
    if (ts.isForStatement(statement)) {
      if (statement.initializer) {
        if (ts.isVariableDeclarationList(statement.initializer)) {
          for (const declaration of statement.initializer.declarations) {
            const name = identifierName(declaration.name, "for-loop variable");
            const slot = this.allocateLocal(name, declaration);
            if (declaration.initializer) {
              this.requireValue(declaration.initializer);
              this.emit({ op: "store_local", slot });
              this.emit({ op: "drop" });
            }
          }
        } else if (this.compileExpression(statement.initializer)) {
          this.emit({ op: "drop" });
        }
      }
      const condition = this.module.code.length;
      let jumpEnd: number | undefined;
      if (statement.condition) {
        this.requireValue(statement.condition);
        jumpEnd = this.emit({ op: "jump_if_false", target: 0 });
      }
      this.beginLoop();
      this.compileStatement(statement.statement);
      const increment = this.module.code.length;
      this.patchContinues(increment);
      if (statement.incrementor && this.compileExpression(statement.incrementor)) {
        this.emit({ op: "drop" });
      }
      this.emit({ op: "jump", target: condition });
      const end = this.module.code.length;
      if (jumpEnd !== undefined) this.patch(jumpEnd, end);
      this.endLoop(end);
      return;
    }
    if (ts.isBreakStatement(statement)) {
      const targets = this.breakStack.at(-1);
      if (!targets) throw new RtCompileError("break used outside a loop", statement);
      targets.push(this.emit({ op: "jump", target: 0 }));
      return;
    }
    if (ts.isContinueStatement(statement)) {
      const targets = this.continueStack.at(-1);
      if (!targets) throw new RtCompileError("continue used outside a loop", statement);
      targets.push(this.emit({ op: "jump", target: 0 }));
      return;
    }
    if (ts.isReturnStatement(statement)) {
      if (this.kind === "handler") {
        if (statement.expression && this.compileExpression(statement.expression)) {
          this.emit({ op: "drop" });
        }
        this.emit({ op: "halt" });
      } else {
        if (statement.expression) this.requireValue(statement.expression);
        else this.emit({ op: "push_i64", value: 0 });
        this.emit({ op: "return_value" });
      }
      return;
    }
    if (ts.isEmptyStatement(statement)) return;
    throw new RtCompileError(`unsupported realtime statement ${ts.SyntaxKind[statement.kind]}`, statement);
  }

  private compileExpression(node: ts.Expression): boolean {
    const expression = unwrapExpression(node);
    if (ts.isNumericLiteral(expression)) {
      this.emit({ op: "push_i64", value: safeInteger(Number(expression.text), expression) });
      return true;
    }
    if (expression.kind === ts.SyntaxKind.TrueKeyword || expression.kind === ts.SyntaxKind.FalseKeyword) {
      this.emit({ op: "push_i64", value: expression.kind === ts.SyntaxKind.TrueKeyword ? 1 : 0 });
      return true;
    }
    if (ts.isIdentifier(expression)) {
      const local = this.locals.get(expression.text);
      if (local !== undefined) {
        this.emit({ op: "load_local", slot: local });
        return true;
      }
      const state = this.module.stateSlots.get(expression.text);
      if (state !== undefined) {
        this.emit({ op: "load_state", slot: state });
        return true;
      }
      const constant = this.module.constants.get(expression.text);
      if (constant !== undefined) {
        this.emit({ op: "push_i64", value: constant });
        return true;
      }
      throw new RtCompileError(`unknown realtime identifier ${expression.text}`, expression);
    }
    if (ts.isPropertyAccessExpression(expression)) {
      if (this.eventName && ts.isIdentifier(expression.expression) && expression.expression.text === this.eventName) {
        const op = {
          code: "load_event_code" as const,
          edge: "load_event_edge" as const,
          source: "load_event_source" as const,
        }[expression.name.text];
        if (!op) throw new RtCompileError(`unknown event field ${expression.name.text}`, expression);
        this.emit({ op });
        return true;
      }
      this.emit({ op: "push_i64", value: this.module.constant(expression) });
      return true;
    }
    if (ts.isPrefixUnaryExpression(expression)) {
      if (expression.operator === ts.SyntaxKind.PlusPlusToken || expression.operator === ts.SyntaxKind.MinusMinusToken) {
        return this.compileIncrement(expression.operand, expression.operator === ts.SyntaxKind.PlusPlusToken, false);
      }
      this.requireValue(expression.operand);
      const op = {
        [ts.SyntaxKind.MinusToken]: "neg" as const,
        [ts.SyntaxKind.ExclamationToken]: "logical_not" as const,
        [ts.SyntaxKind.TildeToken]: "bit_not" as const,
      }[expression.operator];
      if (expression.operator === ts.SyntaxKind.PlusToken) return true;
      if (!op) throw new RtCompileError("unsupported realtime unary operator", expression);
      this.emit({ op });
      return true;
    }
    if (ts.isPostfixUnaryExpression(expression)) {
      return this.compileIncrement(expression.operand, expression.operator === ts.SyntaxKind.PlusPlusToken, true);
    }
    if (ts.isBinaryExpression(expression)) {
      return this.compileBinary(expression);
    }
    if (ts.isConditionalExpression(expression)) {
      this.requireValue(expression.condition);
      const jumpFalse = this.emit({ op: "jump_if_false", target: 0 });
      this.requireValue(expression.whenTrue);
      const jumpEnd = this.emit({ op: "jump", target: 0 });
      this.patch(jumpFalse, this.module.code.length);
      this.requireValue(expression.whenFalse);
      this.patch(jumpEnd, this.module.code.length);
      return true;
    }
    if (ts.isCallExpression(expression)) {
      return this.compileCall(expression);
    }
    throw new RtCompileError(`unsupported realtime expression ${ts.SyntaxKind[expression.kind]}`, expression);
  }

  private compileBinary(expression: ts.BinaryExpression): boolean {
    const kind = expression.operatorToken.kind;
    if (kind === ts.SyntaxKind.EqualsToken || isCompoundAssignment(kind)) {
      const target = this.resolveWritable(expression.left);
      if (kind !== ts.SyntaxKind.EqualsToken) {
        this.loadWritable(target);
      }
      this.requireValue(expression.right);
      if (kind !== ts.SyntaxKind.EqualsToken) {
        const op = compoundOpcode(kind, expression);
        this.emit({ op });
      }
      this.storeWritable(target);
      return true;
    }
    if (kind === ts.SyntaxKind.AmpersandAmpersandToken) {
      this.requireValue(expression.left);
      this.emit({ op: "dup" });
      const jumpEnd = this.emit({ op: "jump_if_false", target: 0 });
      this.emit({ op: "drop" });
      this.requireValue(expression.right);
      this.patch(jumpEnd, this.module.code.length);
      return true;
    }
    if (kind === ts.SyntaxKind.BarBarToken) {
      this.requireValue(expression.left);
      this.emit({ op: "dup" });
      const jumpRight = this.emit({ op: "jump_if_false", target: 0 });
      const jumpEnd = this.emit({ op: "jump", target: 0 });
      this.patch(jumpRight, this.module.code.length);
      this.emit({ op: "drop" });
      this.requireValue(expression.right);
      this.patch(jumpEnd, this.module.code.length);
      return true;
    }
    this.requireValue(expression.left);
    this.requireValue(expression.right);
    const op = binaryOpcode(kind, expression);
    this.emit({ op });
    return true;
  }

  private compileCall(call: ts.CallExpression): boolean {
    const name = propertyChain(call.expression);
    if (ts.isIdentifier(call.expression) && this.module.functions.has(call.expression.text)) {
      for (const argument of call.arguments) this.requireValue(argument);
      this.emit({
        op: "call",
        function: this.module.functionId(call.expression.text, call.expression),
        argc: call.arguments.length,
      });
      return true;
    }
    if (name === "held") {
      if (call.arguments.length !== 1) throw new RtCompileError("held expects one input", call);
      const argument = unwrapExpression(call.arguments[0]);
      const value = this.module.constant(argument);
      const isMouse = ts.isPropertyAccessExpression(argument)
        && propertyChain(argument.expression) === "MouseButton";
      this.emit(isMouse ? { op: "held_mouse", button: value } : { op: "held_key", code: value });
      return true;
    }
    if (name === "key.down" || name === "key.up" || name === "key.tap") {
      if (call.arguments.length !== 1) throw new RtCompileError(`${name} expects one key`, call);
      const code = this.module.constant(call.arguments[0]);
      if (name !== "key.up") this.emit({ op: "key_down", code });
      if (name !== "key.down") this.emit({ op: "key_up", code });
      return false;
    }
    if (name === "mouse.down" || name === "mouse.up" || name === "mouse.click") {
      if (call.arguments.length !== 1) throw new RtCompileError(`${name} expects one button`, call);
      const button = this.module.constant(call.arguments[0]);
      if (name !== "mouse.up") this.emit({ op: "mouse_down", button });
      if (name !== "mouse.down") this.emit({ op: "mouse_up", button });
      return false;
    }
    if (name === "mouse.move" || name === "mouse.wheel") {
      if (call.arguments.length !== 2) throw new RtCompileError(`${name} expects x and y`, call);
      this.requireValue(call.arguments[0]);
      this.requireValue(call.arguments[1]);
      this.emit({ op: name === "mouse.move" ? "mouse_move" : "mouse_wheel" });
      return false;
    }
    if (name === "delay.us") {
      if (call.arguments.length !== 1) throw new RtCompileError("delay.us expects microseconds", call);
      this.requireValue(call.arguments[0]);
      this.emit({ op: "delay_us" });
      return false;
    }
    throw new RtCompileError(`unsupported realtime call ${name}`, call);
  }

  private compileIncrement(node: ts.Expression, increment: boolean, postfix: boolean): boolean {
    const target = this.resolveWritable(node);
    this.loadWritable(target);
    if (postfix) this.emit({ op: "dup" });
    this.emit({ op: "push_i64", value: 1 });
    this.emit({ op: increment ? "add" : "sub" });
    this.storeWritable(target);
    if (postfix) this.emit({ op: "drop" });
    return true;
  }

  private requireValue(expression: ts.Expression): void {
    if (!this.compileExpression(expression)) {
      throw new RtCompileError("expression does not produce a value", expression);
    }
  }

  private allocateLocal(name: string, node: ts.Node): number {
    if (this.locals.has(name) || this.module.stateSlots.has(name) || this.module.constants.has(name)) {
      throw new RtCompileError(`duplicate or shadowed realtime name ${name}`, node);
    }
    const slot = this.locals.size;
    if (slot >= 256) throw new RtCompileError("realtime local slot limit exceeded", node);
    this.locals.set(name, slot);
    return slot;
  }

  private resolveWritable(node: ts.Expression): { kind: "local" | "state"; slot: number } {
    const expression = unwrapExpression(node);
    if (!ts.isIdentifier(expression)) {
      throw new RtCompileError("realtime assignments require an identifier target", expression);
    }
    const local = this.locals.get(expression.text);
    if (local !== undefined) return { kind: "local", slot: local };
    const state = this.module.stateSlots.get(expression.text);
    if (state !== undefined) return { kind: "state", slot: state };
    throw new RtCompileError(`${expression.text} is not writable realtime state`, expression);
  }

  private loadWritable(target: { kind: "local" | "state"; slot: number }): void {
    this.emit({ op: target.kind === "local" ? "load_local" : "load_state", slot: target.slot });
  }

  private storeWritable(target: { kind: "local" | "state"; slot: number }): void {
    this.emit({ op: target.kind === "local" ? "store_local" : "store_state", slot: target.slot });
  }

  private patch(index: number, target: number): void {
    const instruction = this.module.code[index];
    if (!instruction || (instruction.op !== "jump" && instruction.op !== "jump_if_false")) {
      throw new Error(`internal Rune compiler error: instruction ${index} is not a jump`);
    }
    instruction.target = target;
  }

  private beginLoop(): void {
    this.breakStack.push([]);
    this.continueStack.push([]);
  }

  private patchContinues(target: number): void {
    for (const jump of this.continueStack.at(-1) ?? []) this.patch(jump, target);
  }

  private endLoop(end: number): void {
    for (const jump of this.breakStack.pop() ?? []) this.patch(jump, end);
    this.continueStack.pop();
  }
}

type BinaryOpcode =
  | "add" | "sub" | "mul" | "div" | "rem" | "bit_and" | "bit_or" | "bit_xor"
  | "shl" | "shr" | "eq" | "ne" | "lt" | "le" | "gt" | "ge";

function binaryOpcode(kind: ts.SyntaxKind, node: ts.Node): BinaryOpcode {
  const opcode = new Map<ts.SyntaxKind, BinaryOpcode>([
    [ts.SyntaxKind.PlusToken, "add"],
    [ts.SyntaxKind.MinusToken, "sub"],
    [ts.SyntaxKind.AsteriskToken, "mul"],
    [ts.SyntaxKind.SlashToken, "div"],
    [ts.SyntaxKind.PercentToken, "rem"],
    [ts.SyntaxKind.AmpersandToken, "bit_and"],
    [ts.SyntaxKind.BarToken, "bit_or"],
    [ts.SyntaxKind.CaretToken, "bit_xor"],
    [ts.SyntaxKind.LessThanLessThanToken, "shl"],
    [ts.SyntaxKind.GreaterThanGreaterThanToken, "shr"],
    [ts.SyntaxKind.EqualsEqualsToken, "eq"],
    [ts.SyntaxKind.EqualsEqualsEqualsToken, "eq"],
    [ts.SyntaxKind.ExclamationEqualsToken, "ne"],
    [ts.SyntaxKind.ExclamationEqualsEqualsToken, "ne"],
    [ts.SyntaxKind.LessThanToken, "lt"],
    [ts.SyntaxKind.LessThanEqualsToken, "le"],
    [ts.SyntaxKind.GreaterThanToken, "gt"],
    [ts.SyntaxKind.GreaterThanEqualsToken, "ge"],
  ]).get(kind);
  if (!opcode) throw new RtCompileError("unsupported realtime binary operator", node);
  return opcode;
}

function isCompoundAssignment(kind: ts.SyntaxKind): boolean {
  return [
    ts.SyntaxKind.PlusEqualsToken,
    ts.SyntaxKind.MinusEqualsToken,
    ts.SyntaxKind.AsteriskEqualsToken,
    ts.SyntaxKind.SlashEqualsToken,
    ts.SyntaxKind.PercentEqualsToken,
    ts.SyntaxKind.AmpersandEqualsToken,
    ts.SyntaxKind.BarEqualsToken,
    ts.SyntaxKind.CaretEqualsToken,
    ts.SyntaxKind.LessThanLessThanEqualsToken,
    ts.SyntaxKind.GreaterThanGreaterThanEqualsToken,
  ].includes(kind);
}

function compoundOpcode(kind: ts.SyntaxKind, node: ts.Node): BinaryOpcode {
  const simple = new Map<ts.SyntaxKind, ts.SyntaxKind>([
    [ts.SyntaxKind.PlusEqualsToken, ts.SyntaxKind.PlusToken],
    [ts.SyntaxKind.MinusEqualsToken, ts.SyntaxKind.MinusToken],
    [ts.SyntaxKind.AsteriskEqualsToken, ts.SyntaxKind.AsteriskToken],
    [ts.SyntaxKind.SlashEqualsToken, ts.SyntaxKind.SlashToken],
    [ts.SyntaxKind.PercentEqualsToken, ts.SyntaxKind.PercentToken],
    [ts.SyntaxKind.AmpersandEqualsToken, ts.SyntaxKind.AmpersandToken],
    [ts.SyntaxKind.BarEqualsToken, ts.SyntaxKind.BarToken],
    [ts.SyntaxKind.CaretEqualsToken, ts.SyntaxKind.CaretToken],
    [ts.SyntaxKind.LessThanLessThanEqualsToken, ts.SyntaxKind.LessThanLessThanToken],
    [ts.SyntaxKind.GreaterThanGreaterThanEqualsToken, ts.SyntaxKind.GreaterThanGreaterThanToken],
  ]).get(kind);
  if (simple === undefined) throw new RtCompileError("unsupported compound assignment", node);
  return binaryOpcode(simple, node);
}

function parseDefinition(definition: () => void): ts.ArrowFunction | ts.FunctionExpression {
  const source = `const __rune_definition = (${definition.toString()});`;
  const file = ts.createSourceFile("rune-inline.ts", source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
  const diagnostics = file.parseDiagnostics;
  if (diagnostics.length > 0) {
    throw new RtCompileError(
      `unable to parse realtime definition: ${ts.flattenDiagnosticMessageText(diagnostics[0].messageText, "\n")}`,
    );
  }
  const statement = file.statements[0];
  if (!statement || !ts.isVariableStatement(statement)) {
    throw new RtCompileError("unable to recover realtime definition source");
  }
  const initializer = statement.declarationList.declarations[0]?.initializer;
  if (!initializer) throw new RtCompileError("realtime definition has no function body");
  const expression = unwrapExpression(initializer);
  if (ts.isArrowFunction(expression) || ts.isFunctionExpression(expression)) return expression;
  throw new RtCompileError("rt.load expects an inline arrow or function expression", expression);
}

function asBlock(body: ts.ConciseBody): ts.Block {
  if (!ts.isBlock(body)) throw new RtCompileError("realtime module definition must use a block body", body);
  return body;
}

function unwrapExpression(expression: ts.Expression): ts.Expression {
  let current = expression;
  while (
    ts.isParenthesizedExpression(current)
    || ts.isAsExpression(current)
    || ts.isTypeAssertionExpression(current)
    || ts.isNonNullExpression(current)
  ) {
    current = current.expression;
  }
  return current;
}

function propertyChain(node: ts.Expression): string {
  const expression = unwrapExpression(node);
  if (ts.isIdentifier(expression)) return expression.text;
  if (ts.isPropertyAccessExpression(expression)) {
    return `${propertyChain(expression.expression)}.${expression.name.text}`;
  }
  return "<dynamic>";
}

function identifierName(name: ts.BindingName, context: string): string {
  if (!ts.isIdentifier(name)) throw new RtCompileError(`${context} do not support destructuring`, name);
  return name.text;
}

function safeInteger(value: number, node: ts.Node): number {
  if (!Number.isSafeInteger(value)) {
    throw new RtCompileError("realtime numbers must be safe integers", node);
  }
  return value;
}

function parseSource(argument: ts.Expression | undefined): 0 | 1 | 2 {
  if (!argument) return 0;
  const expression = unwrapExpression(argument);
  if (!ts.isObjectLiteralExpression(expression)) {
    throw new RtCompileError("handler options must be an object literal", expression);
  }
  for (const property of expression.properties) {
    if (!ts.isPropertyAssignment(property) || property.name.getText() !== "source") continue;
    if (!ts.isStringLiteral(property.initializer)) {
      throw new RtCompileError("handler source must be a string literal", property.initializer);
    }
    const source = { physical: 0 as const, synthetic: 1 as const, any: 2 as const }[
      property.initializer.text as RtSource
    ];
    if (source === undefined) throw new RtCompileError("invalid handler source", property.initializer);
    return source;
  }
  return 0;
}
''',
)

write(
    "packages/sdk/src/rt-native.ts",
    r'''
import { dlopen, FFIType, ptr, suffix } from "bun:ffi";
import { resolve } from "node:path";

import { compileRt, encodeRtModule, type RtModuleSpec } from "./rt";

const SYMBOLS = {
  rune_rt_load: { args: [FFIType.ptr, FFIType.u64], returns: FFIType.i32 },
  rune_rt_clear: { args: [], returns: FFIType.void },
  rune_rt_is_loaded: { args: [], returns: FFIType.u8 },
  rune_rt_state_get: { args: [FFIType.u64, FFIType.ptr], returns: FFIType.i32 },
  rune_rt_state_set: { args: [FFIType.u64, FFIType.i64], returns: FFIType.i32 },
  rune_rt_dispatch_failures: { args: [], returns: FFIType.u64 },
} as const;

type NativeLibrary = ReturnType<typeof openNative>;

export interface RtLoadOptions {
  library?: string;
}

export class RtRuntime {
  private native?: NativeLibrary;
  private module?: RtModuleSpec;

  compile(definition: () => void): RtModuleSpec {
    return compileRt(definition);
  }

  load(definition: (() => void) | RtModuleSpec, options: RtLoadOptions = {}): RtModuleSpec {
    const module = typeof definition === "function" ? compileRt(definition) : definition;
    const native = this.ensureNative(options.library);
    const bytes = encodeRtModule(module);
    const result = native.symbols.rune_rt_load(ptr(bytes), bytes.byteLength);
    if (result !== 0) {
      throw new Error(`Rune failed to load realtime module (native error ${result})`);
    }
    this.module = module;
    return module;
  }

  clear(): void {
    this.ensureNative().symbols.rune_rt_clear();
    this.module = undefined;
  }

  get loaded(): boolean {
    return this.ensureNative().symbols.rune_rt_is_loaded() !== 0;
  }

  state(nameOrSlot: string | number): number {
    const slot = this.resolveState(nameOrSlot);
    const output = new BigInt64Array(1);
    const result = this.ensureNative().symbols.rune_rt_state_get(slot, ptr(output));
    if (result !== 0) throw new Error(`Rune state slot ${slot} is unavailable`);
    const value = Number(output[0]);
    if (!Number.isSafeInteger(value)) {
      throw new Error(`Rune state slot ${slot} exceeds JavaScript's safe integer range`);
    }
    return value;
  }

  setState(nameOrSlot: string | number, value: number): void {
    if (!Number.isSafeInteger(value)) throw new TypeError("Rune state values must be safe integers");
    const slot = this.resolveState(nameOrSlot);
    const result = this.ensureNative().symbols.rune_rt_state_set(slot, BigInt(value));
    if (result !== 0) throw new Error(`Rune state slot ${slot} is unavailable`);
  }

  get dispatchFailures(): bigint {
    return this.ensureNative().symbols.rune_rt_dispatch_failures();
  }

  private resolveState(nameOrSlot: string | number): number {
    if (typeof nameOrSlot === "number") {
      if (!Number.isInteger(nameOrSlot) || nameOrSlot < 0) throw new TypeError("invalid state slot");
      return nameOrSlot;
    }
    if (!this.module) throw new Error("no realtime module is loaded by this controller");
    const slot = this.module.stateNames.indexOf(nameOrSlot);
    if (slot < 0) throw new Error(`unknown Rune state ${nameOrSlot}`);
    return slot;
  }

  private ensureNative(path?: string): NativeLibrary {
    if (!this.native) this.native = openNative(path ?? defaultLibraryPath());
    return this.native;
  }
}

function openNative(path: string) {
  return dlopen(path, SYMBOLS);
}

function defaultLibraryPath(): string {
  const configured = process.env.RUNE_NATIVE_LIBRARY;
  if (configured) return configured;
  const stem = process.platform === "win32" ? "rune_native" : "librune_native";
  return resolve(process.cwd(), "target", "release", `${stem}.${suffix}`);
}

export const rt = new RtRuntime();
''',
)

append_once(
    "packages/sdk/src/index.ts",
    'export * from "./rt";',
    r'''
export * from "./rt";
export * from "./rt-native";
''',
)

sdk_package_path = ROOT / "packages/sdk/package.json"
sdk_package = json.loads(sdk_package_path.read_text(encoding="utf-8"))
sdk_package.setdefault("dependencies", {})["typescript"] = "^7.0.0"
sdk_package_path.write_text(json.dumps(sdk_package, indent=2) + "\n", encoding="utf-8")

write(
    "packages/sdk/test/rt.test.ts",
    r'''
import { describe, expect, test } from "bun:test";

import {
  Key,
  compileRt,
  delay,
  encodeRtModule,
  held,
  key,
  on,
} from "../src/rt";

describe("stateful realtime TypeScript compiler", () => {
  test("lowers persistent state, conditions, loops, and helper calls", () => {
    const module = compileRt(() => {
      let combo = 0;
      const threshold = 3;

      function burst(count: number): number {
        for (let index = 0; index < count; index++) {
          key.tap(Key.E);
          delay.us(40);
        }
        return count;
      }

      on.keyDown(Key.Q, (event) => {
        combo++;
        if (combo >= threshold && held(Key.LeftShift) && event.code === Key.Q) {
          burst(2);
          combo = 0;
        }
      });
    });

    expect(module.stateNames).toEqual(["combo"]);
    expect(module.states).toEqual([0]);
    expect(module.functions).toHaveLength(1);
    expect(module.handlers).toHaveLength(1);
    expect(module.code.some((instruction) => instruction.op === "call")).toBe(true);
    expect(module.code.some((instruction) => instruction.op === "jump_if_false")).toBe(true);
    expect(module.code.some((instruction) => instruction.op === "store_state")).toBe(true);

    const encoded = encodeRtModule(module);
    const decoded = JSON.parse(new TextDecoder().decode(encoded)) as { version: number };
    expect(decoded.version).toBe(2);
  });

  test("rejects dynamic object allocation on the realtime path", () => {
    expect(() =>
      compileRt(() => {
        on.keyDown(Key.Q, () => {
          const invalid = [1, 2, 3];
          key.tap(invalid[0] as never);
        });
      }),
    ).toThrow();
  });
});
''',
)

write(
    "examples/stateful.ts",
    r'''
import { Key, delay, held, key, on, rt } from "@rune/sdk";

rt.load(() => {
  // `let` at the module-definition level becomes persistent native state.
  let combo = 0;
  const threshold = 3;

  function burst(count: number): number {
    for (let index = 0; index < count; index++) {
      key.tap(Key.E);
      delay.us(40);
    }
    return count;
  }

  on.keyDown(Key.Q, (event) => {
    combo++;

    if (combo >= threshold && held(Key.LeftShift) && event.code === Key.Q) {
      burst(2);
      combo = 0;
    }
  });
});

console.log("Rune stateful realtime module loaded");
''',
)

write(
    "docs/typescript-runtime.md",
    r'''
# Stateful TypeScript runtime

Rune uses TypeScript for more than a declarative macro builder, but it deliberately does not run an unrestricted JavaScript VM on the input thread.

## Two execution planes

```text
Bun / full TypeScript control plane
  - imports, files, network, configuration, overlay updates
  - hot reload and diagnostics
  - can read and update native state through FFI

AOT realtime TypeScript subset
  - persistent integer state
  - if / else and conditional expressions
  - for / while / do loops
  - helper functions and bounded calls
  - integer and boolean expressions
  - key, mouse, held-state, event, and delay intrinsics
  - compiled once to native bytecode
```

The module definition is parsed from its function source and is never executed as JavaScript:

```ts
rt.load(() => {
  let combo = 0;

  function burst(count: number) {
    for (let i = 0; i < count; i++) key.tap(Key.E);
  }

  on.keyDown(Key.Q, () => {
    combo++;
    if (combo === 3) {
      burst(2);
      combo = 0;
    }
  });
});
```

`combo` is stored in an `AtomicI64` slot owned by the native runtime. Conditions, loops, and helper calls are bytecode instructions. A physical input does not invoke Bun, N-API, a promise, or a JavaScript callback.

## Fixed hot-path resources

Each input thread reuses fixed native scratch memory:

- 256 value-stack entries
- 256 local slots
- 32 call frames
- 64 batched output events
- a default 100,000-instruction budget per handler

The instruction budget terminates accidental infinite loops. Dynamic object/array allocation, closures created during dispatch, exceptions, async functions, generators, recursion without a depth bound, and arbitrary library calls are rejected by the compiler.

## Persistence and control-plane access

State persists across input events for the lifetime of the loaded module. Full TypeScript can inspect or change it outside the input callback:

```ts
console.log(rt.state("combo"));
rt.setState("combo", 0);
```

Installing a new module currently resets its state to declared initial values. State migration across hot reload is a separate feature because it needs an explicit schema/version policy.

## Timing caveat

The VM removes JavaScript scheduling and allocation jitter from the event path. It does not make a general-purpose desktop OS hard realtime. Published latency claims must be based on per-backend p50/p95/p99 measurements from OS-event visibility to native injection submission.

`delay.us()` currently uses the same absolute-deadline sleep/spin strategy as the first native executor. A later scheduler should turn delayed continuations into preallocated queue entries so a long macro does not occupy the observer thread.
''',
)

readme = ROOT / "README.md"
readme_text = readme.read_text(encoding="utf-8")
section = r'''

## Stateful realtime TypeScript

Rune also compiles a bounded TypeScript subset for stateful hot-path logic. Top-level `let` declarations become persistent native state; `if`, loops, and helper functions become VM bytecode rather than per-event JavaScript callbacks.

```ts
rt.load(() => {
  let count = 0;

  function tapTwice() {
    for (let i = 0; i < 2; i++) key.tap(Key.E);
  }

  on.keyDown(Key.Q, () => {
    count++;
    if (count === 3) {
      tapTwice();
      count = 0;
    }
  });
});
```

Full Bun/TypeScript remains the control plane. See [`docs/typescript-runtime.md`](docs/typescript-runtime.md) for the supported realtime subset and its fixed execution limits.
'''
if "## Stateful realtime TypeScript" not in readme_text:
    readme.write_text(readme_text.rstrip() + textwrap.dedent(section), encoding="utf-8")

root_package_path = ROOT / "package.json"
root_package = json.loads(root_package_path.read_text(encoding="utf-8"))
root_package.setdefault("scripts", {})["example:stateful"] = "bun examples/stateful.ts"
root_package_path.write_text(json.dumps(root_package, indent=2) + "\n", encoding="utf-8")

# Remove the one-shot archive/bootstrap mechanism after materialization.
shutil.rmtree(ROOT / ".bootstrap", ignore_errors=True)
(ROOT / ".github/workflows/bootstrap.yml").unlink(missing_ok=True)

print("Rune stateful realtime TypeScript runtime installed")
