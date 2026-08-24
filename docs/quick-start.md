# Quick Start

This guide starts from a fresh clone, compiles a stateful TypeScript macro, and executes the resulting bytecode in Rune's native Rust VM.

The current branch does **not** install a global keyboard/mouse hook. Its runnable development path is compiler + native VM simulation. That is enough to evaluate the TypeScript API, persistent-state semantics, control flow, output batching, binary format, and VM diagnostics before direct OS backends land.

## Requirements

- Bun 1.3.14 or newer
- Rust stable
- Git

The workspace declares Rust 1.81 as its minimum supported Rust version and checks that version in CI.

## 1. Clone and build

```bash
git clone https://github.com/eunhhu/spellwire.git
cd spellwire
bun run setup
```

`bun run setup` performs a frozen Bun install, builds the TypeScript project references, and builds the complete Rust workspace in release mode.

## 2. Compile the included macro

```bash
bun run compile:example
```

Expected files:

```text
examples/stateful.rune.bin
examples/stateful.rune.bin.json
```

Inspect the compiled native program:

```bash
bun run inspect:example
```

The inspector prints handler count, persistent-state count, instruction count, resource limits, initial state, trigger source, and bytecode entry points.

## 3. Run the native VM simulator

```bash
bun run simulate:example
```

The script dispatches three `Q` press/release pairs into the real Rust VM. For every input event the simulator prints:

- matched handler count;
- executed instruction count;
- output-event count;
- zero-delay output batches;
- persistent state after dispatch.

The example changes `phase` across events, emits a variable number of `E` taps, and conditionally emits a left click plus an 80 µs VM delay.

## 4. Create a macro

Create `macro.rune.ts`:

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

let combo = 0;
let enabled = true;

function tap(key: Key, count: number): void {
  for (let index = 0; index < count; index++) {
    keyDown(key);
    keyUp(key);
    sleepUs(40);
  }
}

rt.onKeyDown(
  Key.Q,
  () => {
    if (!enabled || keyHeld(Key.LeftShift)) return;

    combo++;
    if (combo >= 3) {
      tap(Key.E, 2);
      clickMouse(MouseButton.Left);
      combo = 0;
    }
  },
  { source: InputSource.Physical },
);

rt.onKeyDown(Key.F8, () => {
  enabled = !enabled;
});
```

Compile it:

```bash
bun packages/compiler/src/cli.ts macro.rune.ts
```

Inspect and simulate it:

```bash
cargo run -q -p rune-cli -- inspect macro.rune.bin
cargo run -q -p rune-cli -- simulate macro.rune.bin \
  key-down:Q key-up:Q \
  key-down:Q key-up:Q \
  key-down:Q key-up:Q
```

## Simulator event syntax

```text
key-down:Q
key-up:Q
key-down:LeftShift
key-up:0xe1
mouse-down:left
mouse-up:left
key-down:Q:synthetic
```

Accepted event kinds are `key-down`, `key-up`, `mouse-down`, and `mouse-up`. The optional final field is `physical` or `synthetic`; physical is the default.

Key names are case-insensitive and ignore hyphens/underscores. Common USB HID names, letters, digits, F1–F12, modifiers, arrows, and hexadecimal `0xNN` codes are accepted.

## Generated manifest

The compiler writes `<program>.rune.bin.json` next to the binary. Its `states` object maps source names to numeric native slots:

```json
{
  "states": {
    "combo": { "slot": 0, "kind": "number" },
    "enabled": { "slot": 1, "kind": "boolean" }
  }
}
```

A future/live Bun host can use this manifest with the native state get/set ABI instead of hard-coding slot indexes.

## Verify the checkout

```bash
bun run check
cargo clippy --workspace --all-targets --locked
```

The permanent GitHub workflow additionally runs Rust tests/builds on Linux, macOS, and Windows, verifies Rust 1.81, and repeats this Quick Start as a smoke test.

## Live system input status

`rune-native` currently exposes a host-callback ABI and reports only `HostCallbackInjection`. It does not yet expose a function that starts Windows Raw Input/hooks, a macOS event tap, or Linux evdev/uinput processing. Therefore:

- the simulator requires no Accessibility/Input Monitoring/udev permissions;
- running the TypeScript source directly with Bun registers JavaScript fallback handlers but does not observe global input;
- any document or example claiming that `bun macro.ts` already installs a cross-platform global hook is outdated.

See [Platform Status](platforms.md) and [Implementation Status](status.md) for the next milestone.
