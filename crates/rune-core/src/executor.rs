use core::fmt;
use std::{
    hint::spin_loop,
    thread,
    time::{Duration, Instant},
};

use crate::{Action, InputEvent, OutputEvent, Program, ProgramSet};

pub const MAX_OUTPUT_BATCH: usize = 64;

pub trait Injector {
    type Error;

    /// Submit a contiguous zero-delay output batch to the native platform API.
    fn send(&mut self, events: &[OutputEvent]) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, Copy)]
pub struct ExecutionConfig {
    /// The final part of a short wait is actively spun to avoid scheduler overshoot.
    pub spin_threshold: Duration,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            spin_threshold: Duration::from_micros(100),
        }
    }
}

pub struct ExecutionScratch {
    output: [OutputEvent; MAX_OUTPUT_BATCH],
    len: usize,
}

impl ExecutionScratch {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            output: [OutputEvent::Empty; MAX_OUTPUT_BATCH],
            len: 0,
        }
    }

    fn push<I: Injector>(
        &mut self,
        event: OutputEvent,
        injector: &mut I,
    ) -> Result<(), I::Error> {
        if self.len == self.output.len() {
            self.flush(injector)?;
        }
        self.output[self.len] = event;
        self.len += 1;
        Ok(())
    }

    fn flush<I: Injector>(&mut self, injector: &mut I) -> Result<(), I::Error> {
        if self.len != 0 {
            let len = std::mem::replace(&mut self.len, 0);
            injector.send(&self.output[..len])?;
        }
        Ok(())
    }
}

impl Default for ExecutionScratch {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DispatchReport {
    pub programs: u16,
    pub output_events: u32,
}

#[derive(Debug)]
pub struct DispatchError<E> {
    pub program_id: u16,
    pub program_name: Box<str>,
    pub source: E,
}

impl<E: fmt::Display> fmt::Display for DispatchError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "program {} ({:?}) failed: {}",
            self.program_id, self.program_name, self.source
        )
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for DispatchError<E> {}

#[derive(Debug)]
pub struct Engine {
    programs: ProgramSet,
    config: ExecutionConfig,
}

impl Engine {
    #[must_use]
    pub fn new(programs: ProgramSet, config: ExecutionConfig) -> Self {
        Self { programs, config }
    }

    #[must_use]
    pub fn programs(&self) -> &ProgramSet {
        &self.programs
    }

    pub fn dispatch<I: Injector>(
        &self,
        event: InputEvent,
        injector: &mut I,
        scratch: &mut ExecutionScratch,
    ) -> Result<DispatchReport, DispatchError<I::Error>> {
        let mut report = DispatchReport::default();
        for (program_id, program) in self.programs.matching(event) {
            let output_events = self
                .execute(program, injector, scratch)
                .map_err(|source| DispatchError {
                    program_id,
                    program_name: program.name.clone(),
                    source,
                })?;
            report.programs = report.programs.saturating_add(1);
            report.output_events = report.output_events.saturating_add(output_events);
        }
        Ok(report)
    }

    fn execute<I: Injector>(
        &self,
        program: &Program,
        injector: &mut I,
        scratch: &mut ExecutionScratch,
    ) -> Result<u32, I::Error> {
        let mut output_events = 0_u32;
        let mut deadline = Instant::now();

        for &action in program.actions.iter() {
            match action {
                Action::DelayUs(delay) => {
                    scratch.flush(injector)?;
                    deadline = deadline
                        .checked_add(Duration::from_micros(u64::from(delay)))
                        .unwrap_or_else(Instant::now);
                    wait_until(deadline, self.config.spin_threshold);
                }
                output => {
                    if let Some(event) = OutputEvent::from_action(output) {
                        scratch.push(event, injector)?;
                        output_events = output_events.saturating_add(1);
                    }
                }
            }
        }
        scratch.flush(injector)?;
        Ok(output_events)
    }
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

#[cfg(test)]
mod tests {
    use crate::{
        Action, Edge, InputDevice, InputEvent, InputSource, Program, ProgramSet, SourceFilter,
        Trigger,
    };

    use super::{Engine, ExecutionConfig, ExecutionScratch, Injector};

    #[derive(Default)]
    struct RecordingInjector {
        batches: Vec<Vec<crate::OutputEvent>>,
    }

    impl Injector for RecordingInjector {
        type Error = core::convert::Infallible;

        fn send(&mut self, events: &[crate::OutputEvent]) -> Result<(), Self::Error> {
            self.batches.push(events.to_vec());
            Ok(())
        }
    }

    #[test]
    fn batches_actions_separated_by_delay() {
        let programs = ProgramSet::new(vec![Program {
            name: "combo".into(),
            trigger: Trigger {
                device: InputDevice::Keyboard,
                code: 0x14,
                edge: Edge::Down,
                source: SourceFilter::Physical,
            },
            actions: vec![
                Action::KeyDown(0x08),
                Action::MouseDown(crate::MouseButton::Left),
                Action::DelayUs(0),
                Action::MouseUp(crate::MouseButton::Left),
                Action::KeyUp(0x08),
            ]
            .into_boxed_slice(),
        }])
        .unwrap();
        let engine = Engine::new(programs, ExecutionConfig::default());
        let mut injector = RecordingInjector::default();
        let mut scratch = ExecutionScratch::new();

        let report = engine
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

        assert_eq!(report.programs, 1);
        assert_eq!(report.output_events, 4);
        assert_eq!(injector.batches.len(), 2);
        assert_eq!(injector.batches[0].len(), 2);
        assert_eq!(injector.batches[1].len(), 2);
    }
}
