use std::{convert::Infallible, hint::black_box, time::Instant};

use rune_core::{
    key, Action, Edge, Engine, ExecutionConfig, ExecutionScratch, Injector, InputDevice,
    InputEvent, InputSource, OutputEvent, Program, ProgramSet, SourceFilter, Trigger,
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

    let programs = ProgramSet::new(vec![Program {
        name: "dispatch-benchmark".into(),
        trigger: Trigger {
            device: InputDevice::Keyboard,
            code: key::Q,
            edge: Edge::Down,
            source: SourceFilter::Physical,
        },
        actions: vec![Action::KeyDown(key::E), Action::KeyUp(key::E)].into_boxed_slice(),
    }])
    .expect("benchmark program should compile");
    let engine = Engine::new(programs, ExecutionConfig::default());
    let event = InputEvent {
        device: InputDevice::Keyboard,
        code: key::Q,
        edge: Edge::Down,
        source: InputSource::Physical,
    };
    let mut injector = NullInjector;
    let mut scratch = ExecutionScratch::new();

    for _ in 0..WARMUP {
        black_box(
            engine
                .dispatch(event, &mut injector, &mut scratch)
                .expect("null injection cannot fail"),
        );
    }

    let mut timings = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        black_box(
            engine
                .dispatch(event, &mut injector, &mut scratch)
                .expect("null injection cannot fail"),
        );
        timings.push(start.elapsed().as_nanos() as u64);
    }
    timings.sort_unstable();

    println!("Rune core dispatch benchmark ({samples} samples)");
    println!("p50  {:>8} ns", percentile(&timings, 50.0));
    println!("p95  {:>8} ns", percentile(&timings, 95.0));
    println!("p99  {:>8} ns", percentile(&timings, 99.0));
    println!("p999 {:>8} ns", percentile(&timings, 99.9));
    println!("max  {:>8} ns", timings.last().copied().unwrap_or(0));
    println!();
    println!(
        "This measures trigger lookup + VM execution + a null injector. It does not measure HID, OS injection, or target-application polling."
    );
}

fn percentile(sorted: &[u64], percentile: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = ((percentile / 100.0) * (sorted.len().saturating_sub(1) as f64)).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}
