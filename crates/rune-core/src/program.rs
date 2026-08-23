use core::fmt;

use crate::{Edge, InputDevice, InputEvent, InputSource, SourceFilter, Trigger};

pub const MAX_KEY_CODE: usize = 256;
pub const MAX_MOUSE_BUTTON: usize = 8;
const BASE_TRIGGER_SLOTS: usize = MAX_KEY_CODE * 2 + MAX_MOUSE_BUTTON * 2;
const SOURCE_TABLES: usize = 3;
const TRIGGER_SLOTS: usize = BASE_TRIGGER_SLOTS * SOURCE_TABLES;

#[derive(Debug, Clone)]
pub struct Program {
    pub name: Box<str>,
    pub trigger: Trigger,
    pub actions: Box<[crate::Action]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramSetError {
    TooManyPrograms(usize),
    InvalidTrigger { name: Box<str>, trigger: Trigger },
    TooManyProgramsForTrigger { trigger: Trigger },
}

impl fmt::Display for ProgramSetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyPrograms(count) => {
                write!(f, "program count {count} exceeds the u16 runtime limit")
            }
            Self::InvalidTrigger { name, trigger } => {
                write!(f, "program {name:?} has an invalid trigger: {trigger:?}")
            }
            Self::TooManyProgramsForTrigger { trigger } => {
                write!(f, "too many programs share trigger {trigger:?}")
            }
        }
    }
}

impl std::error::Error for ProgramSetError {}

#[derive(Debug, Clone, Copy, Default)]
struct Bucket {
    start: u32,
    len: u16,
}

#[derive(Debug, Clone)]
pub struct ProgramSet {
    programs: Box<[Program]>,
    buckets: Box<[Bucket]>,
    program_ids: Box<[u16]>,
}

impl ProgramSet {
    pub fn new(programs: Vec<Program>) -> Result<Self, ProgramSetError> {
        if programs.len() > usize::from(u16::MAX) {
            return Err(ProgramSetError::TooManyPrograms(programs.len()));
        }

        let mut lists: Vec<Vec<u16>> = (0..TRIGGER_SLOTS).map(|_| Vec::new()).collect();
        for (program_id, program) in programs.iter().enumerate() {
            let Some(slot) = trigger_slot(program.trigger) else {
                return Err(ProgramSetError::InvalidTrigger {
                    name: program.name.clone(),
                    trigger: program.trigger,
                });
            };
            if lists[slot].len() == usize::from(u16::MAX) {
                return Err(ProgramSetError::TooManyProgramsForTrigger {
                    trigger: program.trigger,
                });
            }
            lists[slot].push(program_id as u16);
        }

        let total_ids = lists.iter().map(Vec::len).sum();
        let mut buckets = vec![Bucket::default(); TRIGGER_SLOTS];
        let mut program_ids = Vec::with_capacity(total_ids);
        for (slot, ids) in lists.into_iter().enumerate() {
            let start = program_ids.len();
            program_ids.extend(ids);
            buckets[slot] = Bucket {
                start: start as u32,
                len: (program_ids.len() - start) as u16,
            };
        }

        Ok(Self {
            programs: programs.into_boxed_slice(),
            buckets: buckets.into_boxed_slice(),
            program_ids: program_ids.into_boxed_slice(),
        })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.programs.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.programs.is_empty()
    }

    #[must_use]
    pub fn programs(&self) -> &[Program] {
        &self.programs
    }

    #[must_use]
    pub fn matching(&self, event: InputEvent) -> MatchingPrograms<'_> {
        let Some(base) = base_slot(event.device, event.code, event.edge) else {
            return MatchingPrograms::empty(&self.programs);
        };

        let exact_source = match event.source {
            InputSource::Physical => SourceFilter::Physical,
            InputSource::Synthetic => SourceFilter::Synthetic,
        };
        let exact = self.ids_for_slot(source_slot(base, exact_source));
        let any = self.ids_for_slot(source_slot(base, SourceFilter::Any));

        MatchingPrograms {
            programs: &self.programs,
            first: exact.iter(),
            second: any.iter(),
        }
    }

    fn ids_for_slot(&self, slot: usize) -> &[u16] {
        let bucket = self.buckets[slot];
        let start = bucket.start as usize;
        let end = start + usize::from(bucket.len);
        &self.program_ids[start..end]
    }
}

pub struct MatchingPrograms<'a> {
    programs: &'a [Program],
    first: core::slice::Iter<'a, u16>,
    second: core::slice::Iter<'a, u16>,
}

impl<'a> MatchingPrograms<'a> {
    fn empty(programs: &'a [Program]) -> Self {
        let empty: &'a [u16] = &[];
        Self {
            programs,
            first: empty.iter(),
            second: empty.iter(),
        }
    }
}

impl<'a> Iterator for MatchingPrograms<'a> {
    type Item = (u16, &'a Program);

    fn next(&mut self) -> Option<Self::Item> {
        self.first
            .next()
            .or_else(|| self.second.next())
            .map(|id| (*id, &self.programs[usize::from(*id)]))
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
    use crate::{Action, Edge, InputDevice, InputEvent, InputSource, SourceFilter, Trigger};

    use super::{Program, ProgramSet};

    fn program(name: &str, source: SourceFilter) -> Program {
        Program {
            name: name.into(),
            trigger: Trigger {
                device: InputDevice::Keyboard,
                code: 0x14,
                edge: Edge::Down,
                source,
            },
            actions: vec![Action::KeyDown(0x08)].into_boxed_slice(),
        }
    }

    #[test]
    fn matches_exact_and_any_without_allocating() {
        let set = ProgramSet::new(vec![
            program("physical", SourceFilter::Physical),
            program("any", SourceFilter::Any),
            program("synthetic", SourceFilter::Synthetic),
        ])
        .unwrap();

        let physical: Vec<_> = set
            .matching(InputEvent {
                device: InputDevice::Keyboard,
                code: 0x14,
                edge: Edge::Down,
                source: InputSource::Physical,
            })
            .map(|(_, p)| p.name.as_ref())
            .collect();
        assert_eq!(physical, ["physical", "any"]);

        let synthetic: Vec<_> = set
            .matching(InputEvent {
                device: InputDevice::Keyboard,
                code: 0x14,
                edge: Edge::Down,
                source: InputSource::Synthetic,
            })
            .map(|(_, p)| p.name.as_ref())
            .collect();
        assert_eq!(synthetic, ["synthetic", "any"]);
    }
}
