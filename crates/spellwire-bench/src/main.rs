use std::{convert::Infallible, hint::black_box, time::Instant};

use spellwire_core::{
    key, Edge, Handler, Injector, InputDevice, InputEvent, InputSource, Instruction, Opcode,
    OutputEvent, Program, Runtime, RuntimeConfig, SourceFilter, Trigger, VmScratch,
};

const DEFAULT_SAMPLES: usize = 1_000_000;
const WARMUP: usize = 100_000;

struct NullInjector;

impl Injector for NullInjector {
    type Error = Infallible;

    #[inline(always)]
    fn send(&mut self, events: &[OutputEvent]) -> Result<(), Self::Error> {
        black_box(events);
        Ok(())
    }
}

fn main() {
    let samples = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_SAMPLES);

    let program = Program {
        initial_state: vec![0].into_boxed_slice(),
        handlers: vec![Handler {
            trigger: Trigger {
                device: InputDevice::Keyboard,
                code: key::Q,
                edge: Edge::Down,
                source: SourceFilter::Physical,
            },
            entry: 0,
        }]
        .into_boxed_slice(),
        code: vec![
            Instruction::new(Opcode::LoadState).with_a(0),
            Instruction::new(Opcode::PushConst).with_immediate(1),
            Instruction::new(Opcode::Add),
            Instruction::new(Opcode::StoreState).with_a(0),
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
    let mut injector = NullInjector;
    let mut scratch = VmScratch::new();

    for _ in 0..WARMUP {
        black_box(runtime.dispatch(event, &mut injector, &mut scratch).unwrap());
    }

    let mut timings = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        black_box(runtime.dispatch(event, &mut injector, &mut scratch).unwrap());
        timings.push(start.elapsed().as_nanos() as u64);
    }
    timings.sort_unstable();

    println!("Spellwire VM dispatch benchmark ({samples} samples)");
    println!("p50  {:>8} ns", percentile(&timings, 50.0));
    println!("p95  {:>8} ns", percentile(&timings, 95.0));
    println!("p99  {:>8} ns", percentile(&timings, 99.0));
    println!("p999 {:>8} ns", percentile(&timings, 99.9));
    println!("max  {:>8} ns", timings.last().copied().unwrap_or(0));
    println!();
    println!(
        "Scope: trigger lookup + persistent-state VM + null injection. HID, OS injection and target polling are excluded."
    );
}

fn percentile(sorted: &[u64], percentile: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = ((percentile / 100.0) * (sorted.len().saturating_sub(1) as f64)).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}
