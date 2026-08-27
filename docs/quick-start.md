# Quick Start

[한국어](quick-start.ko.md)

Spellwire can be installed into an existing Bun project or tested from a source checkout. The alpha supports deterministic simulation and a live native global-input host.

Compile and simulate the example before enabling global input. [Live Native Host Guide](live-host.md) explains lifecycle options, and [Platform Verification Guide](platform-verification.md) provides OS-specific permission and test commands.

## Install from npm

After the first npm release:

```bash
bun add spellwire
```

Or scaffold a project:

```bash
bun create spellwire my-automation
cd my-automation
bun run start
```

Release packages are assembled with a native library and overlay executable for each supported platform.

The generated project uses three commands:

```bash
bun run start  # compile in memory and run once
bun run watch  # run and hot reload source changes
bun run build  # write dist/main.spellwire.bin plus its JSON manifest
```

`start` and `watch` check/request global-input permissions automatically. No separate setup command is needed for the normal path.

The scaffold puts realtime logic and the state-driven overlay in one `src/main.ts`. The compiler extracts only realtime handlers into native bytecode, while `Spellwire.start()` keeps unrestricted application/UI code on Bun and owns the shared lifecycle. See [Overlay](overlay.md) for layout and window options.

## Develop from source

### Requirements

- Bun 1.4.0 or newer
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
  Key,
  MouseButton,
  clickMouse,
  keyDown,
  keyUp,
  rt,
  sleep,
} from "spellwire";

let combo = 0;
let enabled = true;

function tap(key: Key, count: number): void {
  for (let index = 0; index < count; index++) {
    keyDown(key);
    keyUp(key);
    sleep.us(40);
  }
}

rt.hotkey(
  "Q",
  () => {
    combo++;
    if (combo >= 3) {
      tap(Key.E, 2);
      clickMouse(MouseButton.Left);
      combo = 0;
    }
  },
  { repeat: false, when: () => enabled },
);

rt.hotkey("F8", () => {
  enabled = !enabled;
}, { consume: false, repeat: false });
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

`NativeHost` uses this manifest to expose `host.states.combo` / `host.state("enabled")` and to preserve compatible values by name during hot reload.

## Run live global input

In a generated project, use the three scripts above. In a source checkout, first build the native library and overlay:

```bash
bun run build:native
```

Run once or watch:

```bash
bun packages/spellwire/src/cli.ts run macro.spellwire.ts
bun packages/spellwire/src/cli.ts watch macro.spellwire.ts
```

Both commands compile `.ts` in memory and prepare permissions before starting the platform observer/injector. `watch` adds serialized filesystem reloads. They also accept a `.spellwire.bin` plus its adjacent JSON manifest. Press `Ctrl+C` to stop cleanly.

Verify injection → observation → VM state on the current machine:

```bash
bun run test:platform-loopback
bun run bench:platform -- 10000
target/release/spellwire-overlay --smoke
bun run test:overlay-live
```

On Windows, the overlay executable is `target/release/spellwire-overlay.exe`. On Linux, the overlay command needs an active graphical session. `bench:platform` measures native OS submission-call return time, not physical key-to-application latency.

The loopback command should print JSON with all of these success fields:

```json
{"loopback":"ok","observed":1,"reloadReleasedHeldInput":true}
```

The actual output also includes `platform`, `arch`, and `elapsedUs`. See [Platform Verification](platform-verification.md) for the complete expected output, OS setup, and a result template.

## Verify the checkout

```bash
bun run check
cargo clippy --workspace --all-targets --locked
```

The permanent GitHub workflow additionally runs Rust tests/builds on Linux, macOS, and Windows, verifies Rust 1.81, and repeats this Quick Start as a smoke test.

## Platform validation status

macOS arm64 has local permission, loopback, suppression, dynamic-lane, overlay, and submission-benchmark verification. Windows x64 has interactive-session source, build, loopback/reload, dynamic-lane, overlay lifecycle/window-policy, package, and benchmark verification; physical suppression and visual transparency remain manual checks. Linux has source coverage but still needs device and graphical-session verification. See [Platform Status](platforms.md) for setup and exact limitations.

## Where to continue

- Build a production host: [Live Native Host Guide](live-host.md)
- Learn every supported handler construct: [TypeScript Runtime](typescript-runtime.md)
- Look up exports and signatures: [API Reference](api.md)
- Verify macOS, Windows, or Linux: [Platform Verification Guide](platform-verification.md)
- Diagnose a failure: [Troubleshooting](troubleshooting.md)
