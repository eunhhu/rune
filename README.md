# Rune

Rune is a native realtime input automation runtime with a TypeScript scripting SDK.

TypeScript is the authoring language, but latency-sensitive handlers do **not** run as JavaScript callbacks on every input event. Rune supports both a simple macro builder and a stateful realtime TypeScript subset compiled into a native Rust VM.

> Early MVP. Input runtime and stateful RT execution are the current focus; the native overlay renderer is still under construction.

## Quick Start

Requirements: Bun 1.3.14+ and Rust 1.81+.

```bash
git clone https://github.com/eunhhu/rune.git
cd rune
bun install
cargo build -p rune-native --release
bun run example:lunge
```

Or create `macro.ts`:

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

rune.load(lunge).start();
```

```bash
bun macro.ts
```

Full setup, permissions, and first-run instructions: **[Quick Start](docs/quick-start.md)**.

## Stateful realtime TypeScript

The reason Rune uses TypeScript is not only syntax. Stateful macros can keep variables across events and use conditions, loops, and functions while still avoiding JS execution on the input hot path.

```ts
import { Key, delay, held, key, on, rt } from "@rune/sdk";

rt.load(() => {
  let combo = 0;

  function burst(count: number) {
    for (let i = 0; i < count; i++) {
      key.tap(Key.E);
      delay.us(40);
    }
  }

  on.keyDown(Key.Q, () => {
    combo++;

    if (combo >= 3 && held(Key.LeftShift)) {
      burst(2);
      combo = 0;
    }
  });
});
```

`combo`, the branch, the loop, and the function execute in Rune's native VM. Bun remains the unrestricted control plane for configuration, networking, disk I/O, UI state, plugins, and hot reload.

Read **[TypeScript Runtime](docs/typescript-runtime.md)** for the execution model and supported subset.

## Architecture

```text
Bun / TypeScript application
   │
   ├─ normal TS                configuration / I/O / UI / plugins
   │
   ├─ macro(...)              compile-time flat native program
   │
   └─ rt.load(...)            stateful TS → native bytecode
                               │
                               ▼
                       Rust realtime runtime
                 ┌───────────────────────────────┐
physical input → │ trigger LUT → VM → injection │ → OS input stream
                 └───────────────────────────────┘
```

The intended hot path has no per-event JavaScript callback, Promise, async runtime, or hash-table lookup.

## Platform backends

| Platform | Observe | Inject | Status |
| --- | --- | --- | --- |
| Windows | native global input backend | `SendInput` | MVP |
| macOS | `CGEventTap` | `CGEventPost` | MVP |
| Linux | evdev | uinput | MVP |

Scripts use Rune/USB-HID-style key identifiers instead of platform-specific scan codes.

See **[Platform Notes](docs/platforms.md)** for permissions and limitations.

## Documentation

- [Quick Start](docs/quick-start.md) — fresh clone to first macro
- [API Reference](docs/api.md) — macro builder, runtime, keys, mouse
- [TypeScript Runtime](docs/typescript-runtime.md) — persistent state, conditions, loops, functions
- [Architecture](docs/architecture.md) — control plane vs realtime plane
- [Platform Notes](docs/platforms.md) — Windows/macOS/Linux backends
- [Overlay](docs/overlay.md) — retained native overlay design
- [Troubleshooting](docs/troubleshooting.md) — permissions, native loading, timing issues
- [Status](docs/status.md) — implemented vs not-yet-claimed features

## Development

```bash
bun install
cargo test --workspace
cargo build -p rune-native --release
bun run typecheck
bun run test:ts
```

Run the native core benchmark:

```bash
bun run bench
```

Rune does not currently claim a guaranteed physical-switch-to-application microsecond latency. Performance work is measured at the framework boundary using percentile/jitter distributions; USB polling, OS scheduling, and target-application polling are outside that boundary.

## Overlay

The overlay is isolated from input execution and is designed as a retained native scene handed to a dedicated renderer thread. Rune will not introduce Electron/webview rendering into the realtime runtime just for convenience.

See [Overlay](docs/overlay.md).

## License

MIT
