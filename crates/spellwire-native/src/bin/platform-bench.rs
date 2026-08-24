use std::{hint::black_box, process::ExitCode, time::Instant};

use spellwire_core::OutputEvent;
use spellwire_native::platform;

const DEFAULT_SAMPLES: usize = 10_000;
const WARMUP_SAMPLES: usize = 100;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("platform submission benchmark failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let samples = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_SAMPLES);
    let mut injector = platform::create_injector()?;
    let no_movement = [OutputEvent::MouseMove { dx: 0, dy: 0 }];

    for _ in 0..WARMUP_SAMPLES.min(samples) {
        injector.send(&no_movement)?;
    }

    let mut timings = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        black_box(injector.send(&no_movement)?);
        timings.push(u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX));
    }
    timings.sort_unstable();

    println!("Spellwire platform submission benchmark ({samples} zero-delta mouse batches)");
    println!("p50  {:>8} ns", percentile(&timings, 500));
    println!("p95  {:>8} ns", percentile(&timings, 950));
    println!("p99  {:>8} ns", percentile(&timings, 990));
    println!("p999 {:>8} ns", percentile(&timings, 999));
    println!("max  {:>8} ns", timings.last().copied().unwrap_or(0));
    println!();
    println!("Scope: native OS submission call return; device delivery and application polling excluded.");
    Ok(())
}

fn percentile(sorted: &[u64], permille: usize) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = sorted.len().saturating_sub(1).saturating_mul(permille).saturating_add(500) / 1_000;
    sorted[rank.min(sorted.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::percentile;

    #[test]
    fn percentile_uses_nearest_rank() {
        assert_eq!(percentile(&[10, 20, 30, 40, 50], 500), 30);
        assert_eq!(percentile(&[10, 20, 30, 40, 50], 999), 50);
    }
}
