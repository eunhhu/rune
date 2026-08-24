# Quick Start

Spellwire can be installed into an existing Bun project or tested from a source checkout. The current alpha compiles stateful TypeScript into native bytecode and runs that bytecode through the real Rust VM simulator. It does **not** yet install a global keyboard/mouse observer.

## Install from npm

After the first npm release:

```bash
bun add spellwire
```

Or scaffold a project:

```bash
bun create spellwire my-automation
cd my-automation
bun run check
```

The generated project can author and compile `.spellwire.ts` modules. Live global input still requires a host/backend that is not bundled in this alpha.

## Develop from source

### Requirements

- Bun 1.3.14 or newer
- Rust stable
- Git

The workspace declares Rust 1.81 as its minimum supported Rust version and checks that version in CI.

### 1. Clone and build

```bash
git clone https://github.com/eunhhu/spellwire.git
cd spellwire
bun run setup
```

`bun run setup` performs a frozen Bun install, builds the TypeScript project references, and builds the complete Rust workspace in release mode.

### 2. Compile the included macro

```bash
bun run compile:example
```

Expected files:

```text
examples/stateful.spellwire.bin
examples/stateful.spellwire.bin.json
```

Inspect the compiled native program:

```bash
bun run inspect:example
```

The inspector prints handler count, persistent-state count, instruction count, resource limits, initial state, trigger source, and bytecode entry points.

### 3. Run the native VM simulator

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

### 4. Create a macro

Create `macro.spellwire.ts`:

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
} from "spellwire";

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
bunx spellwire compile macro.spellwire.ts
```

Inspect and simulate it:

```bash
cargo run -q -p spellwire-cli --locked -- inspect macro.spellwire.bin
cargo run -q -p spellwire-cli --locked -- simulate macro.spellwire.bin \
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

The compiler writes `<program>.spellwire.bin.json` next to the binary. Its `states` object maps source names to numeric native slots:

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

`spellwire-native` currently exposes a host-callback ABI and reports only `HostCallbackInjection`. It does not yet expose a function that starts Windows Raw Input/hooks, a macOS event tap, or Linux evdev/uinput processing. Therefore:

- the simulator requires no Accessibility/Input Monitoring/udev permissions;
- running the TypeScript source directly with Bun registers JavaScript fallback handlers but does not observe global input;
- any document or example claiming that `bun macro.spellwire.ts` already installs a cross-platform global hook is outdated.

See [Platform Status](platforms.md) and [Implementation Status](status.md) for the next milestone.
