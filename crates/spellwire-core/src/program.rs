use core::fmt;

use crate::{Edge, InputDevice, InputEvent, InputSource, Instruction, SourceFilter, Trigger};

pub const MAX_KEY_CODE: usize = 256;
pub const MAX_MOUSE_BUTTON: usize = 8;
const BASE_TRIGGER_SLOTS: usize = MAX_KEY_CODE * 2 + MAX_MOUSE_BUTTON * 2;
const SOURCE_TABLES: usize = 3;
const TRIGGER_SLOTS: usize = BASE_TRIGGER_SLOTS * SOURCE_TABLES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handler {
    pub trigger: Trigger,
    pub entry: u32,
}

#[derive(Debug, Clone)]
pub struct Program {
    pub initial_state: Box<[i64]>,
    pub handlers: Box<[Handler]>,
    pub code: Box<[Instruction]>,
    pub local_count: u16,
    pub stack_limit: u16,
    pub instruction_budget: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramError {
    NoHandlers,
    NoCode,
    TooManyHandlers(usize),
    InvalidTrigger(Trigger),
    InvalidEntry { handler: usize, entry: u32 },
    InvalidJump { instruction: usize, target: u32 },
    InvalidStateSlot { instruction: usize, slot: u16 },
    InvalidLocalSlot { instruction: usize, slot: u16 },
    StackLimitTooLarge(u16),
    LocalCountTooLarge(u16),
    ZeroInstructionBudget,
    TooManyHandlersForTrigger(Trigger),
}

impl fmt::Display for ProgramError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoHandlers => f.write_str("program has no input handlers"),
            Self::NoCode => f.write_str("program has no bytecode"),
            Self::TooManyHandlers(count) => write!(f, "handler count {count} exceeds u16"),
            Self::InvalidTrigger(trigger) => write!(f, "invalid trigger {trigger:?}"),
            Self::InvalidEntry { handler, entry } => {
                write!(f, "handler {handler} points to invalid entry {entry}")
            }
            Self::InvalidJump { instruction, target } => {
                write!(f, "instruction {instruction} jumps to invalid target {target}")
            }
            Self::InvalidStateSlot { instruction, slot } => {
                write!(f, "instruction {instruction} uses invalid state slot {slot}")
            }
            Self::InvalidLocalSlot { instruction, slot } => {
                write!(f, "instruction {instruction} uses invalid local slot {slot}")
            }
            Self::StackLimitTooLarge(limit) => write!(f, "stack limit {limit} exceeds runtime cap"),
            Self::LocalCountTooLarge(count) => write!(f, "local count {count} exceeds runtime cap"),
            Self::ZeroInstructionBudget => f.write_str("instruction budget must be non-zero"),
            Self::TooManyHandlersForTrigger(trigger) => {
                write!(f, "too many handlers share trigger {trigger:?}")
            }
        }
    }
}

impl std::error::Error for ProgramError {}

#[derive(Debug, Clone, Copy, Default)]
struct Bucket {
    start: u32,
    len: u16,
}

#[derive(Debug, Clone)]
pub struct HandlerTable {
    buckets: Box<[Bucket]>,
    handler_ids: Box<[u16]>,
}

impl HandlerTable {
    /// Builds the fixed trigger lookup table for a validated handler list.
    ///
    /// # Errors
    ///
    /// Returns [`ProgramError`] when the list is empty, exceeds wire-format limits, contains an
    /// invalid trigger, or places too many handlers in one trigger bucket.
    pub fn build(handlers: &[Handler]) -> Result<Self, ProgramError> {
        if handlers.is_empty() {
            return Err(ProgramError::NoHandlers);
        }
        if handlers.len() > usize::from(u16::MAX) {
            return Err(ProgramError::TooManyHandlers(handlers.len()));
        }

        let mut lists: Vec<Vec<u16>> = (0..TRIGGER_SLOTS).map(|_| Vec::new()).collect();
        for (handler_id, handler) in handlers.iter().enumerate() {
            let Some(slot) = trigger_slot(handler.trigger) else {
                return Err(ProgramError::InvalidTrigger(handler.trigger));
            };
            if lists[slot].len() == usize::from(u16::MAX) {
                return Err(ProgramError::TooManyHandlersForTrigger(handler.trigger));
            }
            let handler_id = u16::try_from(handler_id)
                .map_err(|_| ProgramError::TooManyHandlers(handlers.len()))?;
            lists[slot].push(handler_id);
        }

        let total = lists.iter().map(Vec::len).sum();
        let mut buckets = vec![Bucket::default(); TRIGGER_SLOTS];
        let mut handler_ids = Vec::with_capacity(total);
        for (slot, ids) in lists.into_iter().enumerate() {
            let start = handler_ids.len();
            handler_ids.extend(ids);
            let bucket_start =
                u32::try_from(start).map_err(|_| ProgramError::TooManyHandlers(handlers.len()))?;
            let bucket_len = u16::try_from(handler_ids.len() - start)
                .map_err(|_| ProgramError::TooManyHandlers(handlers.len()))?;
            buckets[slot] = Bucket { start: bucket_start, len: bucket_len };
        }

        Ok(Self {
            buckets: buckets.into_boxed_slice(),
            handler_ids: handler_ids.into_boxed_slice(),
        })
    }

    #[must_use]
    pub fn matching(&self, event: InputEvent) -> MatchingHandlers<'_> {
        let Some(base) = base_slot(event.device, event.code, event.edge) else {
            return MatchingHandlers::empty();
        };
        let exact = match event.source {
            InputSource::Physical => SourceFilter::Physical,
            InputSource::Synthetic => SourceFilter::Synthetic,
        };
        MatchingHandlers {
            first: self.ids_for_slot(source_slot(base, exact)).iter(),
            second: self.ids_for_slot(source_slot(base, SourceFilter::Any)).iter(),
        }
    }

    fn ids_for_slot(&self, slot: usize) -> &[u16] {
        let bucket = self.buckets[slot];
        let start = bucket.start as usize;
        &self.handler_ids[start..start + usize::from(bucket.len)]
    }
}

pub struct MatchingHandlers<'a> {
    first: core::slice::Iter<'a, u16>,
    second: core::slice::Iter<'a, u16>,
}

impl MatchingHandlers<'_> {
    fn empty() -> Self {
        let empty: &[u16] = &[];
        Self { first: empty.iter(), second: empty.iter() }
    }
}

impl Iterator for MatchingHandlers<'_> {
    type Item = u16;

    fn next(&mut self) -> Option<Self::Item> {
        self.first.next().or_else(|| self.second.next()).copied()
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
