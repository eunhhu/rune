# Spellwire

Spellwire is a stateful realtime input-automation runtime for Bun and TypeScript. TypeScript remains the authoring language, while analyzable input handlers are compiled to fixed-memory native bytecode instead of invoking JavaScript for every physical event.

> Early alpha. The compiler, bytecode VM, persistent state, control-flow lowering, C ABI, and package scaffolding are implemented. Validated direct OS observers/injectors, prebuilt native packages, and the lightweight native overlay are still in progress.

## Install

After the first npm release, add Spellwire to an existing Bun project:

```bash
bun add spellwire
```

Or create a new project:

```bash
bun create spellwire my-automation
cd my-automation
bun run check
```

The unscoped `spellwire` package includes the TypeScript API, compiler, and CLI. `bun create spellwire` is provided by `create-spellwire`.

## Stateful realtime TypeScript

```ts
import {
  InputSource,
  Key,
  MouseButton,
  clickMouse,
  keyHeld,
  rt,
  sleepUs,
  tapKey,
} from "spellwire";

let phase = 0;
let enabled = true;

function tapRepeated(key: Key, count: number): void {
  for (let index = 0; index < count; index++) {
    tapKey(key);
  }
}

rt.onKeyDown(
  Key.Q,
  () => {
    if (!enabled || !keyHeld(Key.LeftShift)) return;

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

Module-scope integer and boolean `let` declarations captured by realtime handlers become persistent native state. Conditions, loops, arithmetic, helper functions, held-input queries, delays, and output intrinsics compile ahead of time. Full Bun/JavaScript remains available for control-plane code outside the compiled handlers.

## Compile

```bash
bunx spellwire compile src/main.spellwire.ts
```

This writes `src/main.spellwire.bin` and `src/main.spellwire.bin.json`.

## What is usable in this alpha

You can install/scaffold the packages, author stateful TypeScript handlers, compile bytecode, test intrinsics through the JavaScript fallback lane, and embed the Rust VM through its C ABI.

The npm package does **not yet bundle prebuilt global-input backends**. Direct Windows/macOS/Linux observation and injection are deliberately not advertised until recursion, permissions, and latency have been validated on each platform. See [implementation status](docs/status.md).

## Develop from source

```bash
git clone https://github.com/eunhhu/spellwire.git
cd spellwire
bun install
bun run check
cargo build -p spellwire-native --release
bun run compile:example
```

## Packages

| Package | Purpose |
| --- | --- |
| `spellwire` | Public SDK, embedded compiler, and `spellwire` CLI |
| `create-spellwire` | `bun create spellwire` initializer |
| `spellwire-core` | Native bytecode, VM, persistent state, and scheduler |
| `spellwire-native` | Stable C ABI and platform integration boundary |
| `spellwire-bench` | Native runtime percentile benchmark |

## Documentation

- [Quick Start](docs/quick-start.md)
- [API reference](docs/api.md)
- [Realtime TypeScript](docs/typescript-runtime.md)
- [Architecture](docs/architecture.md)
- [Platforms and permissions](docs/platforms.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Publishing](docs/publishing.md)
- [Implementation status](docs/status.md)
- [Overlay design](docs/overlay.md)

## Performance contract

Spellwire separates framework latency from switch/USB polling, OS scheduling, compositor behavior, and target-application polling. Published performance claims must include platform-specific p50/p95/p99/p99.9 and maximum jitter.

## License

MIT
