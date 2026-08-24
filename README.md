# Spellwire

[한국어](README.ko.md)

Spellwire is a stateful realtime input-automation runtime for Bun and TypeScript. Analyzable input handlers are compiled ahead of time into bounded native bytecode instead of invoking JavaScript for every input event.

> Early alpha. The TypeScript AOT compiler, bounded native VM, lock-free consuming hotkeys/remaps, state gates, non-blocking delay scheduler, Bun FFI host, global platform backends, shared dynamic lane, and retained native overlay are implemented. macOS has live loopback and suppression verification; Windows and Linux still need target-machine runs.

## Install

After the first npm release:

```bash
bun add spellwire
```

Or create a project:

```bash
bun create spellwire my-automation
cd my-automation
bun run start
```

The public package includes the TypeScript API, compiler, and `spellwire` CLI. `bun create spellwire` is provided by `create-spellwire`.

Generated projects have three commands:

```bash
bun run start  # compile in memory and run
bun run watch  # run with native hot reload
bun run build  # write dist/main.spellwire.bin and its manifest
```

`start` and `watch` prepare platform permissions automatically before the native host starts.

Generated projects also include a state-driven modern overlay in `src/app.ts`. It uses `Spellwire.start()` plus Figma-style `ui.row`/`ui.column` auto layout; edit realtime logic and UI independently without a manual update loop. See [State-driven native overlay](docs/overlay.md).

## Stateful realtime TypeScript

```ts
import { Key, rt, tapKey } from "spellwire";

let enabled = true;
let presses = 0;

rt.hotkey("Ctrl+Shift+K", () => {
  presses += 1;
  tapKey(Key.Enter);
}, {
  repeat: false,
  when: () => enabled,
});

rt.hotkey("F8", () => {
  enabled = !enabled;
}, { consume: false });

rt.remap("CapsLock", "Escape", { when: () => enabled });
```

Portable strings replace modifier boilerplate, `when` gates action and pass-through together, and remaps emit paired down/up transitions automatically. Module-scope integer and boolean `let` declarations referenced by realtime handlers become persistent native state. Conditions, loops, arithmetic, helper functions, held-input queries, delays, and low-level `rt.onKey*`/`rt.onMouse*` registrations remain available when a larger state machine needs them. Ordinary Bun code outside realtime handlers remains unrestricted control-plane TypeScript.

## Build

```bash
bun run build
```

The generated project writes `dist/main.spellwire.bin` plus its JSON state manifest. To compile another path directly:

```bash
bunx spellwire compile src/main.spellwire.ts
```

Direct CLI output defaults next to the input source.

## Run locally

A fresh source checkout has a deterministic inspector/simulator:

```bash
git clone https://github.com/eunhhu/spellwire.git
cd spellwire
bun run setup
bun run compile:example
bun run inspect:example
bun run simulate:example
```

The simulator decodes the same binary format consumed by the C ABI, dispatches named key/mouse events through `spellwire-core`, prints native output batches, and shows persistent state after each event.

For live global input from a source checkout, build once and start watch mode:

```bash
bun run build:native
bun packages/spellwire/src/cli.ts watch examples/stateful.spellwire.ts
```

The CLI checks/requests platform permissions before starting. `Ctrl+C` stops the observer/runtime and releases synthetic inputs that remain held.

If this is your first live run, follow [Live Native Host Guide](docs/live-host.md) instead of guessing the lifecycle. It includes complete CLI and programmatic examples, safe shutdown, hot-reload state rules, dynamic input events, and error interpretation. Before testing a release on another machine, use the copyable checklist in [Platform Verification](docs/platform-verification.md).

## What works today

| Capability | Status |
| --- | --- |
| TypeScript AOT compiler | Implemented |
| Persistent integer/boolean state | Implemented |
| Conditions, loops, assignments, held checks, helper functions | Implemented |
| Portable consuming hotkeys, release triggers, state gates, and remaps | Windows/macOS implemented; Linux suppression pending |
| Native VM, versioned wire format, and fixed output batches | Implemented |
| Native inspector/simulator | Implemented |
| C ABI with explicit and owned-host lifecycle APIs | Implemented |
| Bun FFI host, named state, watch/reload, and SPSC dynamic lane | Implemented |
| Windows hooks/`SendInput`, macOS event tap/`CGEventPost`, Linux evdev/uinput | Implemented; macOS live-verified |
| State-driven auto-layout overlay, modern styling, retained dirty updates | Implemented; macOS live-verified |
| Cross-platform prebuilt artifact/signing workflow | Implemented; release credentials required |
| Physical end-to-end microsecond latency claim | Not claimed |

## Packages and crates

| Name | Purpose |
| --- | --- |
| `spellwire` | Public SDK, embedded compiler, and TypeScript CLI |
| `create-spellwire` | `bun create spellwire` initializer |
| `spellwire-core` | Bytecode decoder, trigger table, persistent-state VM, scheduler |
| `spellwire-native` | Stable C ABI, owned host, global observers, and native injectors |
| `spellwire-overlay` | Transparent retained renderer process using winit/wgpu |
| `spellwire-cli` / `spellwire-sim` | Native inspector and deterministic simulator |
| `spellwire-bench` | Native dispatch percentile benchmark |

## Documentation

- [Documentation index](docs/index.md)
- [Quick Start](docs/quick-start.md)
- [Live Native Host Guide](docs/live-host.md)
- [Platform Verification Guide](docs/platform-verification.md)
- [API reference](docs/api.md)
- [Hotkeys, remaps, state gates, and AutoHotkey migration](docs/automation.md)
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
cargo build --workspace --release --locked
```

`bun run check` includes TypeScript tests, Rust tests, formatting, and Clippy with warnings denied.

Run the native core benchmark:

```bash
bun run bench
bun run bench:platform -- 10000
```

Spellwire separates framework-boundary latency from switch debounce, USB polling, OS scheduling, compositor behavior, and target-application polling. Performance claims require platform-specific percentile and jitter measurements.

## License

MIT
