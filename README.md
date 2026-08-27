# Spellwire

[한국어](README.ko.md)

Spellwire is a stateful realtime input-automation runtime for Bun and TypeScript. Analyzable input handlers are compiled ahead of time into bounded native bytecode instead of invoking JavaScript for every input event.

> Early alpha. The TypeScript AOT compiler, bounded native VM, lock-free consuming hotkeys/remaps, state gates, non-blocking delay scheduler, Bun FFI host, global platform backends, shared dynamic lane, and retained native overlay are implemented. macOS has live loopback and suppression verification. Windows has interactive-session loopback, injection, reload, and overlay lifecycle verification; physical suppression and visual transparency checks remain. Linux target verification remains.

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

Generated projects keep realtime handlers and the state-driven modern overlay together in `src/main.ts`. The compiler extracts only bounded handlers for the native VM; unrestricted application/overlay code stays on Bun. `Spellwire.start()` owns both lifecycles without a manual update loop.

## API at a glance

| Task | API |
| --- | --- |
| Consuming hotkey | `rt.hotkey("Ctrl+Shift+K", handler)` |
| Key remap | `rt.remap("CapsLock", "Escape")` |
| Persistent state | module-scope `let enabled = true` |
| Transient typed event | `const changed = effect("changed", schema)`, then `changed.emit(payload)` |
| Keyboard/mouse output | `tapKey`, `keyDown`, `clickMouse`, `moveMouse`, `wheelMouse` |
| Delay | `sleep.ms(250)`, `sleep.seconds(2)`, or unit-specific helpers |
| Start input + watch + UI | `Spellwire.start(options)` |
| Overlay layout | `ui.row`, `ui.column`, `ui.panel`, `ui.stack` |
| Overlay content | `ui.text`, `ui.ellipse`, `ui.dot`, `ui.badge`, `ui.divider` |
| State-bound UI | `overlay: state => ...`, `ui.bind`, `ui.when` |
| UI styling | `width`, `height`, `padding`, `gap`, `fill`, `stroke`, `shadow`, `opacity`, font props |
| Overlay window | `overlayOptions.window` (`alwaysOnTop`, `transparent`, `focusable`, `clickThrough`, …) |
| Electron/sidecar bridge | `SpellwireRpcServer`, `SpellwireRpcClient` |

See the **[API reference](docs/api.md)** for signatures, defaults, option tables, a complete state-to-overlay application, native window policy, and platform notes.

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

State holds the latest value; an effect reports that something happened. Both compile to numeric IDs and fixed `i64` payloads in the native VM. The worker publishes only actual state changes and effects through a preallocated SPSC lane, so the default overlay refreshes on changes instead of polling native state every frame. Local RPC can expose the same state/effects to Electron or a sidecar without putting serialization, sockets, or JavaScript on the input thread. See [Effects and RPC](docs/effects-rpc.md) for the complete API and performance boundaries.

## Build

```bash
bun run build
```

The generated project writes `dist/main.spellwire.bin` plus its JSON state manifest. To compile another path directly:

```bash
bunx spellwire compile src/main.ts
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

[Live Native Host Guide](docs/live-host.md) covers CLI and programmatic examples, safe shutdown, hot-reload state rules, dynamic input events, and error interpretation. Use [Platform Verification](docs/platform-verification.md) when testing a release on another machine.

## What works today

| Capability | Status |
| --- | --- |
| TypeScript AOT compiler | Implemented |
| Persistent integer/boolean state | Implemented |
| Fixed-payload typed effects and changed-state event lane | Implemented |
| Conditions, loops, assignments, held checks, helper functions | Implemented |
| Portable consuming hotkeys, release triggers, state gates, and remaps | Windows/macOS implemented; Linux suppression pending |
| Native VM, versioned wire format, and fixed output batches | Implemented |
| Native inspector/simulator | Implemented |
| C ABI with explicit and owned-host lifecycle APIs | Implemented |
| Bun FFI host, named state, watch/reload, and SPSC dynamic lane | Implemented |
| Authenticated local RPC for Electron/sidecars | Implemented |
| Windows hooks/`SendInput`, macOS event tap/`CGEventPost`, Linux evdev/uinput | Implemented; macOS live-verified; Windows interactive loopback verified; Windows physical suppression and Linux target runs pending |
| State-driven auto-layout overlay, modern styling, configurable native window policy, retained dirty updates | Implemented; macOS and Windows lifecycle/window-policy smoke verified; Windows visual transparency and Linux compositor checks pending |
| Cross-platform prebuilt artifact/signing workflow | Implemented; release credentials required |
| Physical end-to-end latency | Not measured |

## Packages and crates

| Name | Purpose |
| --- | --- |
| `spellwire` | Public SDK, embedded compiler, and TypeScript CLI |
| `create-spellwire` | `bun create spellwire` initializer |
| `spellwire-core` | Bytecode decoder, trigger table, persistent-state VM, scheduler |
| `spellwire-native` | Stable C ABI, owned host, global observers, and native injectors |
| `spellwire-overlay` | Native retained renderer process using winit/wgpu |
| `spellwire-cli` / `spellwire-sim` | Native inspector and deterministic simulator |
| `spellwire-bench` | Native dispatch percentile benchmark |

## Documentation

Start here:

- **[API reference](docs/api.md)** — create/run/build, hotkeys, state, output, lifecycle, overlay API, defaults, and limitations
- [Quick Start](docs/quick-start.md) — first project and first live run
- [Troubleshooting](docs/troubleshooting.md) — errors and platform setup
- [Platform Verification Guide](docs/platform-verification.md) — copyable macOS, Windows, and Linux checks

Optional deep dives:

- [Documentation index](docs/index.md)
- [Automation semantics and AutoHotkey migration](docs/automation.md)
- [Overlay renderer and performance design](docs/overlay.md)
- [Effects, state synchronization, and local RPC](docs/effects-rpc.md)
- [Realtime compiler subset](docs/typescript-runtime.md)
- [Live native host internals](docs/live-host.md)
- [Architecture](docs/architecture.md), [Native C ABI](docs/native-abi.md), and [Platforms](docs/platforms.md)
- [Publishing](docs/publishing.md), [Implementation status](docs/status.md), and [Verification](docs/runtime-verification.md)

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
bun run bench -- 1000000 --effect
bun run bench:platform -- 10000
```

Spellwire separates framework-boundary latency from switch debounce, USB polling, OS scheduling, compositor behavior, and target-application polling. Performance claims require platform-specific percentile and jitter measurements.

## License

MIT
