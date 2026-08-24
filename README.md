# Spellwire

Spellwire is a low-latency desktop automation runtime whose scripting language is TypeScript.

The important distinction is that Spellwire does **not** send every physical input through a JavaScript callback. Normal TypeScript used inside a realtime handler is compiled ahead of time to compact native bytecode. Persistent state, branches, loops, helper functions, deadlines, and key/mouse output then execute in Rust on the input path.

```ts
import { Key, MouseButton, clickMouse, keyDown, keyUp, rt, sleepUs } from "spellwire";

let phase = 0;          // persistent native state
let enabled = true;     // persistent native state

function burst(key: Key, count: number): void {
  for (let index = 0; index < count; index++) {
    keyDown(key);
    keyUp(key);
  }
}

rt.onKeyDown(Key.Q, () => {
  if (!enabled) return;

  phase = (phase + 1) % 3;
  burst(Key.E, phase + 1);

  if (phase === 2) {
    clickMouse(MouseButton.Left);
    sleepUs(80);
  }
});
```

The callback above is parsed at build/load time. It is not invoked by Bun when `Q` is pressed.

## Execution lanes

Spellwire deliberately has two lanes:

- **Realtime AOT lane:** analyzable TypeScript is lowered to a fixed-width native instruction stream. There is no JS callback, promise, allocation, hash lookup, or event-loop hop per input event.
- **Dynamic Bun lane:** arbitrary TypeScript, objects, async I/O, plugins, configuration, and UI logic remain in Bun. Native input records can be delivered through a preallocated `SharedArrayBuffer` SPSC ring rather than a native-to-JS callback.

This split keeps TypeScript expressive without pretending arbitrary JavaScript has deterministic microsecond tail latency. See [the TypeScript runtime design](docs/typescript-runtime.md).

## Workspace

```text
crates/spellwire-core/       versioned bytecode, trigger table, persistent state VM
crates/spellwire-native/     stable C ABI between Bun/native hosts and spellwire-core
crates/spellwire-bench/      percentile benchmark for trigger lookup + VM dispatch
packages/sdk/           TypeScript API, dynamic event lane, retained overlay scene
packages/compiler/      TypeScript AST → Spellwire bytecode compiler and CLI
examples/               stateful TypeScript macros
```

## Build

Requirements:

- Rust stable (MSRV 1.81)
- Bun

```bash
bun install
bun run typecheck
bun run test:ts
cargo test --workspace
cargo build -p spellwire-native --release
```

Compile an example:

```bash
bun run compile:example
```

That writes a `.spellwire.bin` bytecode module and a state manifest next to the source file.

## Performance contract

Spellwire measures separate boundaries instead of advertising an ambiguous “input latency” number:

1. OS event visible to Spellwire → trigger resolved
2. trigger resolved → native output submission
3. deadline overshoot and p50/p95/p99/p99.9 jitter

USB polling, physical switch debounce, target-application polling, compositor policy, and display latency are reported separately. The repository includes a core VM benchmark, but Spellwire does not claim cross-platform microsecond end-to-end latency until each native backend is measured on real hardware.

## Platform and overlay status

The current main branch contains the portable compiler, runtime VM, C ABI, dynamic JS lane, and retained overlay scene model. Direct Windows/macOS/Linux input and transparent native overlay backends are isolated behind the native boundary and are not marked complete until their permission, recursion, and latency behavior is validated per platform. See [implementation status](docs/status.md) and [overlay design](docs/overlay.md).
