# Runtime Verification

[한국어](runtime-verification.ko.md)

## Portable source gates

```bash
bun install --frozen-lockfile
bun run typecheck
bun run test:ts
bun run test:docs
bun run pack:dry-run
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --release --locked
```

GitHub Actions repeats Rust tests, Clippy, and release builds on Linux, macOS, and Windows; checks Rust 1.81; verifies both npm tarballs; and runs the compiler → wire format → simulator Quick Start.

## Native target gates

Run on every release OS after granting platform permissions:

```bash
bun run build:native
bun run inspect:runtime
bun run test:platform-loopback
bun run test:consume-macos # macOS only
bun run bench:platform -- 10000
target/release/spellwire-overlay --smoke
bun run test:overlay-live
```

Windows uses `target/release/spellwire-overlay.exe`. Linux overlay smoke requires a graphical session. The loopback verifies real native injection, global observation, synthetic classification, second-stage VM execution, and named state access.

[Platform Verification Guide](platform-verification.md) expands these gates into separate macOS, Windows, and Linux procedures, explains expected output, and provides a copyable result report.

## Current local evidence

On macOS arm64, the following passed:

- complete Rust workspace tests/Clippy/release build;
- TypeScript build and tests;
- ABI v4 load, bulk state snapshot, and permission read through Bun FFI;
- global tagged F20 injection observed through `CGEventTap` and handled by the synthetic VM trigger;
- CoreGraphics suppression probe: baseline/inactive-gate transitions `2/2`, active native handler hit `1`, forwarded transitions `0`;
- native observer publication into `DynamicInputLane` with zero drops in the smoke scenario;
- default transparent/topmost/non-focusable/click-through overlay plus hidden opaque/focusable/decorated/resizable non-default policy creation at Retina resolution;
- live overlay mutation rendering with the resolved window policy returned to Bun;
- direct state-immediate VM workload over 200,000 local samples: 42 ns p50, 84 ns p95, and 84 ns p99 for trigger lookup + VM + null injection;
- native OS-submission benchmark execution.

On Windows 10 x64 in an interactive desktop session, the following passed:

- `bun run check` and the locked release workspace build;
- native observe → VM → `SendInput` → observe loopback, synthetic classification, and held-input release during reload;
- dynamic-lane publication;
- default and custom overlay window-policy smoke plus live mutation rendering;
- package dry-run and both native benchmarks;
- VM benchmark p50 100 ns and p99 200 ns; platform submission p50 14.5 µs and p99 30.1 µs for that run.

The same injection check fails from an SSH service in Windows Session 0 with `ACCESS_DENIED`, so Windows live checks must run inside the signed-in interactive session. Physical consuming-hotkey suppression and visual per-pixel transparency are still manual acceptance items; the smoke renderer reported `alphaMode: "Opaque"`. Linux backend code passes cross-target source gates, but its live device/display run remains pending. Linux suppression is not implemented and its capability bit remains unset.

None of these checks measures physical switch-to-target-application latency. Such a claim needs external timestamped hardware or target-application instrumentation.
