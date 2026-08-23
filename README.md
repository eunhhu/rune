# Rune

Rune is a native realtime input automation runtime with a TypeScript scripting SDK.

The design goal is simple: TypeScript describes macros, but **TypeScript never runs on the realtime input hot path**. Scripts compile to compact native programs that are evaluated and dispatched by a Rust runtime.

> Early development. The public API and platform backends are still evolving.

## Architecture

```text
TypeScript / Bun
  └─ @rune/sdk DSL
       └─ compact ProgramSpec
            └─ N-API control plane
                 └─ rune-core
                    ├─ precompiled trigger table
                    ├─ allocation-free VM hot path
                    ├─ monotonic deadline scheduler
                    └─ native platform backend
```

The intended platform backends are:

| Platform | Observe | Inject |
| --- | --- | --- |
| Windows | Raw Input | SendInput |
| macOS | CGEventTap / IOHID | CGEventPost |
| Linux | evdev | uinput |

Overlay rendering is intentionally a separate subsystem so input latency is never coupled to UI work.

## Repository layout

```text
crates/rune-core/   Native IR, VM, scheduler, platform abstraction
crates/rune-napi/   Thin N-API control plane for Bun/Node
packages/sdk/       TypeScript macro DSL and native binding loader
examples/           Example Rune scripts
```

## Example

```ts
import { Key, MouseButton, macro, rune } from "@rune/sdk";

rune.load(
  macro("lunge", (m) => {
    m.on.keyDown(Key.Q).run(
      m.key.down(Key.E),
      m.mouse.down(MouseButton.Left),
      m.delay.us(80),
      m.mouse.up(MouseButton.Left),
      m.key.up(Key.E),
    );
  }),
);

rune.start();
```

The builder callback above runs once while loading the script. It emits an IR program; it is **not** called when the physical key event arrives.

## Performance model

Rune measures latency from the point an OS input event becomes visible to the runtime until the corresponding native injection call is submitted. Physical switch latency, USB polling, OS scheduling, and target-application input polling are outside that measurement.

The realtime path is designed around:

- no JavaScript callbacks
- no heap allocation per input event
- no async runtime
- immutable precompiled programs
- fixed-size trigger lookup
- batched zero-delay output sequences
- absolute monotonic deadlines for delayed sequences

## Development

Requirements:

- Rust stable
- Bun

```bash
bun install
cargo test --workspace
bun run typecheck
```

## Status

The core IR/VM and SDK surface are being built first. Platform backends are implemented behind a small trait so each OS can be optimized independently without leaking platform details into user scripts.
