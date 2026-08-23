# Rune

Rune is a native realtime input automation runtime with a TypeScript scripting SDK.

TypeScript describes macros, but **TypeScript never runs on the realtime input hot path**. A script is evaluated once, compiled into a compact versioned program, and loaded into a Rust runtime that observes, matches, schedules, and injects input natively.

> Rune is an early MVP. The realtime input runtime is implemented; the retained native overlay renderer is designed but not implemented yet.

## Why Rune exists

Most desktop automation libraries optimize for convenience. Rune optimizes for predictable low latency and low jitter:

- no JavaScript callbacks per input event
- no allocation in trigger lookup or VM execution
- no async runtime on the input thread
- fixed-size trigger lookup tables
- batched zero-delay output sequences
- absolute monotonic deadlines with an optional spin tail
- direct OS input APIs instead of a generic automation dependency

## Architecture

```text
macro.ts                         control plane
   │
   ├─ @rune/sdk builder callback runs once
   ├─ emits versioned RUNE binary IR
   └─ Bun FFI loads the IR
              │
              ▼
       rune-native / Rust        realtime plane
   ┌──────────────────────────────────────────┐
   │ native observer → trigger LUT → VM       │
   │                         │                │
   │                         └→ native inject │
   └──────────────────────────────────────────┘
```

The intended platform path is direct and small:

| Platform | Observe | Inject | MVP status |
| --- | --- | --- | --- |
| Windows | Raw Input | SendInput | implemented |
| macOS | CGEventTap | CGEventPost | implemented |
| Linux | evdev | uinput | implemented |

The backends share USB HID key identifiers, so scripts do not contain Windows scan codes, macOS virtual key codes, or Linux input-event codes.

## Example

```ts
import { Key, MouseButton, macro, rune } from "@rune/sdk";

const lunge = macro("lunge", (m) => {
  m.on.keyDown(Key.Q).run(
    m.key.down(Key.E),
    m.mouse.down(MouseButton.Left),
    m.delay.us(80),
    m.mouse.up(MouseButton.Left),
    m.key.up(Key.E),
  );
});

rune.configure({ spinThresholdUs: 100 }).load(lunge).start();
```

The callback passed to `macro()` is a compile-time builder. It is not retained and is never invoked when Q is pressed.

Other compile-time helpers can expand into the same flat native program:

```ts
const burst = macro("burst", (m) => {
  m.on.mouseDown(MouseButton.Back).run(
    m.repeat(3, m.mouse.click(MouseButton.Left), m.delay.us(75)),
  );
});
```

## Repository layout

```text
crates/rune-core/    IR decoder, trigger table, VM, deadline scheduler
crates/rune-native/  C ABI plus Windows/macOS/Linux input backends
crates/rune-bench/   dependency-free core dispatch percentile benchmark
packages/sdk/        TypeScript DSL, encoder, and Bun FFI control plane
examples/            runnable Rune scripts
docs/                architecture, platform, and overlay design notes
```

## Development

Requirements:

- Rust 1.81 or newer
- Bun 1.3.14 or newer

```bash
bun install
cargo test --workspace
bun run typecheck
bun run test:ts
cargo build -p rune-native --release
bun run example:lunge
```

The SDK searches `target/release`, `target/debug`, and packaged native-artifact directories. `RUNE_NATIVE_PATH` can point at an explicit `.dll`, `.dylib`, or `.so`.

On Linux, reading `/dev/input/event*` and creating `/dev/uinput` requires explicit permission. See [`docs/platforms.md`](docs/platforms.md) before running Rune.

## Benchmarking

```bash
cargo run -p rune-bench --release -- 1000000
```

The bundled benchmark measures only native trigger lookup, VM execution, and a null injector. It deliberately does not claim switch-to-application latency; USB polling, OS scheduling, the platform injection API, and target-application polling are separate parts of that path.

## Overlay status

The overlay is deliberately isolated from input execution. The design is a retained native scene with immutable snapshots handed to a platform render thread, never per-frame JavaScript draw callbacks. See [`docs/overlay.md`](docs/overlay.md).

The first input-runtime MVP does **not** advertise the overlay capability yet. That avoids quietly replacing the zero-cost design with Electron, a webview, or a heavyweight cross-platform GUI stack.

## License

MIT
