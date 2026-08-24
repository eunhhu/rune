# Spellwire

Spellwire is a stateful realtime input-automation runtime for Bun and TypeScript. Analyzable input handlers are compiled ahead of time into bounded native bytecode instead of invoking JavaScript for every input event.

> Early alpha. The TypeScript API, AOT compiler, versioned `SPWR` bytecode, persistent-state Rust VM, C ABI, native simulator, package scaffolding, and JavaScript fallback lane work. Validated global OS observers/injectors, prebuilt native packages, and the native overlay renderer are still in progress.

## Install

After the first npm release:

```bash
bun add spellwire
```

Or create a project:

```bash
bun create spellwire my-automation
cd my-automation
bun run check
```

The public package includes the TypeScript API, compiler, and `spellwire` CLI. `bun create spellwire` is provided by `create-spellwire`.

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

Module-scope integer and boolean `let` declarations referenced by realtime handlers become persistent native state. Conditions, loops, arithmetic, helper functions, held-input queries, delays, and output intrinsics compile ahead of time. Ordinary Bun code outside realtime handlers remains unrestricted control-plane TypeScript.

## Compile

```bash
bunx spellwire compile src/main.spellwire.ts
```

This writes `src/main.spellwire.bin` plus a JSON state manifest.

## Run the real native VM locally

A fresh source checkout has a deterministic inspector/simulator:

```bash
git clone https://github.com/eunhhu/spellwire.git
cd spellwire
bun run setup
bun run compile:example
bun run inspect:example
bun run simulate:example
```

The simulator decodes the same binary format consumed by the C ABI, dispatches named key/mouse events through `spellwire-core`, prints native output batches, and shows persistent state after each event. It does not install a global OS hook.

## What works today

| Capability | Status |
| --- | --- |
| TypeScript AOT compiler | Implemented |
| Persistent integer/boolean state | Implemented |
| Conditions, loops, assignments, held checks, helper functions | Implemented |
| Native VM, versioned wire format, and fixed output batches | Implemented |
| Native inspector/simulator | Implemented |
| C ABI with explicit event dispatch and state access | Implemented |
| JavaScript fallback/debug lane and SPSC dynamic lane | Implemented |
| `spellwire` and `create-spellwire` package dry-runs | Implemented |
| Global Windows/macOS/Linux input observation/injection | Planned |
| Native transparent overlay renderer | Planned |
| Physical end-to-end microsecond latency claim | Not claimed |

## Packages and crates

| Name | Purpose |
| --- | --- |
| `spellwire` | Public SDK, embedded compiler, and TypeScript CLI |
| `create-spellwire` | `bun create spellwire` initializer |
| `spellwire-core` | Bytecode decoder, trigger table, persistent-state VM, scheduler |
| `spellwire-native` | Stable C ABI and future platform backend boundary |
| `spellwire-cli` / `spellwire-sim` | Native inspector and deterministic simulator |
| `spellwire-bench` | Native dispatch percentile benchmark |

## Documentation

- [Documentation index](docs/index.md)
- [Quick Start](docs/quick-start.md)
- [API reference](docs/api.md)
- [Realtime TypeScript](docs/typescript-runtime.md)
- [Architecture](docs/architecture.md)
- [Native C ABI](docs/native-abi.md)
- [Platforms](docs/platforms.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Publishing](docs/publishing.md)
- [Implementation status](docs/status.md)
- [Verification](docs/runtime-verification.md)
- [Overlay design](docs/overlay.md)

## Development

```bash
bun install --frozen-lockfile
bun run check
cargo clippy --workspace --all-targets --locked
cargo build --workspace --release --locked
```

Run the native core benchmark:

```bash
bun run bench
```

Spellwire separates framework-boundary latency from switch debounce, USB polling, OS scheduling, compositor behavior, and target-application polling. Performance claims require platform-specific percentile and jitter measurements.

## License

MIT
