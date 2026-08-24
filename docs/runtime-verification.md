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
bun packages/spellwire/src/cli.ts permissions
bun run test:platform-loopback
bun run test:consume-macos # macOS only
bun run bench:platform -- 10000
target/release/spellwire-overlay --smoke
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
- transparent click-through overlay creation at Retina resolution and mutation rendering;
- native OS-submission benchmark execution.

Windows x64 and Linux x64 backend code also passes local cross-target Clippy. That proves compilation, not live permissions/device/display behavior. Windows suppression still needs a target-machine run; Linux suppression is not implemented and its capability bit remains unset.

None of these checks measures physical switch-to-target-application latency. Such a claim needs external timestamped hardware or target-application instrumentation.
