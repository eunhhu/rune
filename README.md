# Rune

Rune is an ahead-of-time TypeScript macro compiler and a bounded native Rust VM. The repository currently lives at `eunhhu/spellwire`; package, crate, wire-format, and ABI names in this branch still use the `rune` prefix.

The central idea is that TypeScript remains the authoring language while latency-sensitive state, conditions, loops, helper functions, and output actions execute as native bytecode rather than as a Bun callback for every input event.

> **Current milestone:** the compiler, wire format, native VM, C ABI, deterministic simulator, and TypeScript fallback tools work. System-wide Windows/macOS/Linux observation and injection are **not implemented in this branch yet**, so the Quick Start exercises the real native VM with simulated input events instead of installing a global hook.

## What works today

| Capability | Status |
| --- | --- |
| TypeScript AOT compiler | Implemented |
| Persistent integer/boolean state | Implemented |
| `if`, loops, assignments, and helper functions | Implemented |
| Native Rust VM and versioned binary format | Implemented |
| Physical/synthetic/any trigger filters | Implemented |
| Native C ABI with host output callback | Implemented |
| Deterministic native VM simulator | Implemented |
| JavaScript fallback/debug lane | Implemented |
| Retained overlay scene model | Implemented |
| Global OS input observation/injection | Planned |
| Native transparent overlay renderer | Planned |
| Published microsecond latency claim | Not claimed |

## Quick Start

Requirements: Bun 1.3.14+ and Rust stable. The workspace declares Rust 1.81 as its MSRV and verifies it in CI.

```bash
git clone https://github.com/eunhhu/spellwire.git
cd spellwire
bun run setup
```

Compile the included stateful macro and execute it in the native VM simulator:

```bash
bun run compile:example
bun run inspect:example
bun run simulate:example
```

`compile:example` produces:

```text
examples/stateful.rune.bin
examples/stateful.rune.bin.json
```

The binary contains native VM bytecode. The JSON file is a control-plane manifest with handler counts and named persistent-state slots.

See **[Quick Start](docs/quick-start.md)** for a custom script, simulator event syntax, and verification commands.

## TypeScript authoring model

A Rune source file is ordinary TypeScript. The compiler scans top-level realtime registrations and lowers only the supported code reachable from their handlers.

```ts
import {
  InputSource,
  Key,
  MouseButton,
  clickMouse,
  keyDown,
  keyHeld,
  keyUp,
  rt,
  sleepUs,
} from "@rune/sdk";

let phase = 0;
let enabled = true;

function tapRepeated(key: Key, count: number): void {
  for (let index = 0; index < count; index++) {
    keyDown(key);
    keyUp(key);
  }
}

rt.onKeyDown(
  Key.Q,
  () => {
    if (!enabled || keyHeld(Key.LeftShift)) return;

    phase = (phase + 1) % 3;
    tapRepeated(Key.E, phase + 1);

    if (phase === 2) {
      clickMouse(MouseButton.Left);
      sleepUs(80);
    }
  },
  { source: InputSource.Physical },
);

rt.onKeyDown(Key.F8, () => {
  enabled = !enabled;
});
```

Module-scope mutable integer and boolean variables referenced by realtime handlers become persistent native state. Handler-local variables become fixed VM locals. Top-level helper functions are inlined into native bytecode.

There is no `rt.load()` wrapper in the current API. The `.rune.ts` module itself is the compilation unit.

## Execution architecture

```text
macro.rune.ts
    │
    ├─ ordinary Bun/TypeScript may coexist outside the realtime dependency graph
    │
    └─ @rune/compiler
           │  parse + validate + lower once
           ▼
      versioned .rune.bin
           │
           ├─ rune-sim              deterministic development runner
           └─ rune-native C ABI     embedding boundary for a future/live host
                      │
                      ▼
             trigger LUT → native VM → fixed output batches
```

The VM dispatch path does not parse TypeScript, schedule a JavaScript callback, create a Promise, or perform string/hash lookup. It uses prevalidated bytecode, fixed-capacity scratch storage, contiguous trigger buckets, and an instruction budget.

## Documentation

- [Documentation index](docs/index.md)
- [Quick Start](docs/quick-start.md)
- [API Reference](docs/api.md)
- [TypeScript Runtime](docs/typescript-runtime.md)
- [Architecture](docs/architecture.md)
- [Native C ABI](docs/native-abi.md)
- [Platform Status](docs/platforms.md)
- [Overlay](docs/overlay.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Implementation Status](docs/status.md)
- [Verification](docs/runtime-verification.md)

## Development

```bash
bun install --frozen-lockfile
bun run typecheck
bun run test:ts
cargo fmt --all -- --check
cargo test --workspace --locked
cargo build --workspace --release --locked
```

Run the core benchmark:

```bash
bun run bench
```

The benchmark measures trigger lookup, VM execution, and a null injector. It does not measure USB polling, OS hook delivery, platform injection, or target-application polling.

## Performance claims

Rune does not currently claim guaranteed microsecond physical-input latency. A meaningful claim requires each direct platform backend to exist and report p50/p95/p99/p99.9 jitter for the interval from OS-visible input to native injection submission. Physical switch latency, device polling, desktop scheduling, and target-application polling are separate scopes.

## License

MIT
