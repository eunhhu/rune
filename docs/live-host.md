# Live Native Host Guide

[한국어](live-host.ko.md)

This guide explains how to run a Spellwire macro against global keyboard and mouse input, how hot reload and named state work, and how to shut the host down safely. Start with [Quick Start](quick-start.md) if you have not compiled and simulated a macro yet.

## What runs where

Spellwire deliberately separates realtime work from ordinary TypeScript:

| Part | Runs in | Appropriate work |
| --- | --- | --- |
| Realtime handler | Bounded native VM | State updates, conditions, bounded loops, held checks, key/mouse output, `sleepUs()` |
| Native host | Native worker and OS backend | Global observation, injection, delayed continuations, state storage |
| Control plane | Bun | Loading, permission checks, hot reload, logging, UI, files, network calls |
| Dynamic lane | Bun over a shared ring | Best-effort reactions to observed input that do not need realtime guarantees |

Code inside `rt.onKeyDown()` and the other `rt.on*()` registrations is compiled ahead of time. Running a `.spellwire.ts` file with `bun` alone only records JavaScript fallback handlers; it does not install global hooks. Use `spellwire run` or `NativeHost` for live input.

## Fastest path

Create a project and start it:

```bash
bun create spellwire my-automation
cd my-automation
bun run start
```

The generated project has exactly three scripts:

| Script | Result |
| --- | --- |
| `bun run start` | Compile source in memory and start the native host |
| `bun run watch` | Start the same host and hot reload accepted source changes |
| `bun run build` | Write `dist/main.spellwire.bin` and `dist/main.spellwire.bin.json` |

`start` and `watch` check global-input permissions once before host startup. If a grant is missing, macOS requests it and every platform prints a targeted recovery message. No permission check, allocation, JavaScript callback, or file watcher is added to native realtime event dispatch.

## Prerequisites

A source checkout needs:

- Bun 1.4.0 or newer;
- Rust 1.81 or newer;
- the platform permissions described in [Platform Verification](platform-verification.md).

From the repository root, install dependencies and build the native runtime and overlay:

```bash
bun install --frozen-lockfile
bun run build:native
```

The build creates these files:

```text
Windows: target/release/spellwire_native.dll
         target/release/spellwire-overlay.exe
macOS:   target/release/libspellwire_native.dylib
         target/release/spellwire-overlay
Linux:   target/release/libspellwire_native.so
         target/release/spellwire-overlay
```

Normal `run`/`watch` commands prepare permissions automatically. For platform diagnostics, the source CLI still exposes this advanced status command:

```bash
bun packages/spellwire/src/cli.ts permissions
```

A fully ready host prints `observe: granted` and `inject: granted`. ABI `4` is expected. Windows/macOS report capabilities `0x77`; Linux reports `0x37` because original-input suppression is pending there. Platform-specific caveats still apply: Windows UIPI is target-specific, macOS has two privacy grants, and Linux requires device-file access.

## Three CLI workflows

### 1. Compile and simulate first

Compile-time errors are safer to diagnose before global input starts:

```bash
bun packages/spellwire/src/cli.ts compile macro.spellwire.ts
cargo run -q -p spellwire-cli --locked -- inspect macro.spellwire.bin
cargo run -q -p spellwire-cli --locked -- simulate macro.spellwire.bin key-down:Q key-up:Q
```

The compile command writes both files below:

```text
macro.spellwire.bin
macro.spellwire.bin.json
```

The JSON file is the state manifest. Do not discard it when running the binary directly.

### 2. Start live input

For a source checkout:

```bash
bun packages/spellwire/src/cli.ts run macro.spellwire.ts
```

For an installed package, use the equivalent public command:

```bash
bunx spellwire run macro.spellwire.ts
```

Successful startup prints:

```text
running /absolute/path/to/macro.spellwire.ts (press Ctrl+C to stop)
```

Press `Ctrl+C` once to stop. The CLI closes the file watcher, stops the native observer and worker, cancels delayed continuations, releases synthetic keys or buttons still held by the host, and unloads the native library.

### 3. Enable hot reload

```bash
bun packages/spellwire/src/cli.ts watch macro.spellwire.ts
```

Every accepted source change prints `reloaded`. A rejected edit prints `reload failed: ...`; the process stays alive so you can correct the file. Reloads are serialized, so several rapid file events cannot mutate the host concurrently.

### 4. Run a compiled binary

The default manifest is `<binary>.json`:

```bash
bun packages/spellwire/src/cli.ts run macro.spellwire.bin
```

Use another manifest path explicitly when necessary:

```bash
bun packages/spellwire/src/cli.ts run macro.spellwire.bin --manifest configs/macro-state.json
```

### CLI command reference

| Command | Purpose |
| --- | --- |
| `spellwire run [source-or-binary]` | Compile source in memory and immediately start the owned native host |
| `spellwire watch [source-or-binary]` | Start the same path with serialized hot reload |
| `spellwire compile [source] [output]` | AOT compile and write the binary plus state manifest |

The default input for all three commands is `src/main.spellwire.ts`. `--library <path>` overrides native library discovery; `--manifest <path>` overrides the adjacent manifest for compiled input.

## Use the unified programmatic API

Application state, hot reload, overlay binding, permissions, and shutdown fit in one owner:

```ts
import { Spellwire, ui } from "spellwire";

const app = await Spellwire.start({
  input: "macro.spellwire.ts",
  watch: true,
  overlay: (state) => ui.text(String(state.phase ?? 0)),
});

await app.untilSignal();
```

See [State-driven native overlay](overlay.md) for modern layout/style properties and the exact state-update path.

## Use `NativeHost` as a low-level host

Use the low-level host when another lifecycle owner needs manual permission, watcher, lane, or shutdown control.

```ts
import {
  NativeHost,
  NativePermission,
} from "spellwire";

const host = await NativeHost.load("macro.spellwire.ts");

const required = NativePermission.Observe | NativePermission.Inject;
let permissions = host.permissionStatus();
if ((permissions & required) !== required) {
  permissions = host.requestPermissions();
}
if ((permissions & required) !== required) {
  host.close();
  throw new Error("Spellwire needs observation and injection permissions");
}

const watcher = host.watch({
  debounceMs: 75,
  preserveState: true,
  onReload: () => console.log("reloaded"),
  onError: (error) => console.error("reload failed", error),
});

try {
  host.start();
  // Replace "phase" with a state name from your generated manifest.
  console.log("phase", host.state("phase").get());

  await new Promise<void>((resolveStop) => {
    const stop = (): void => {
      process.off("SIGINT", stop);
      process.off("SIGTERM", stop);
      resolveStop();
    };
    process.once("SIGINT", stop);
    process.once("SIGTERM", stop);
  });
} finally {
  watcher.close();
  host.close();
}
```

`close()` is idempotent and calls `stop()` when needed. Always place it in `finally`; an exception should not leave global hooks or a synthetic held input active.

### Lifecycle rules

| Operation | Result |
| --- | --- |
| `NativeHost.load(path)` | Compile/read the program, validate ABI, allocate the native host |
| `permissionStatus()` | Read current observe/inject bits without prompting |
| `requestPermissions()` | Request on macOS; recheck on Windows/Linux |
| `start()` | Create injector, observer, runtime worker, and scheduler |
| `reload()` | Recompile/reread, cancel old continuations, release held outputs, install the new program |
| `stop()` | Stop observation and runtime work but keep the wrapper open |
| `close()` | Stop if necessary, free the host, close the dynamic library |

Repeated `start()` and `stop()` calls through the TypeScript wrapper are safe no-ops when already in that state. Calling other operations after `close()` throws `Spellwire native host is closed`.

## Named state and hot reload

Module-scope integer and boolean `let` values used by realtime handlers become native state:

```ts
let phase = 0;
let enabled = true;
```

Read and update them from Bun:

```ts
host.state("enabled").set(false);
console.log(host.state("enabled").get());
console.log(host.states.phase?.get());
```

During a running reload, the TypeScript wrapper preserves a value only when both the source name and state kind still match:

| Edit | Reload result |
| --- | --- |
| Keep `let phase = 0` as a number | Preserve current value |
| Move `phase` to another slot | Preserve by name, not old slot |
| Rename `phase` to `step` | Initialize `step` from source |
| Change number to boolean | Initialize the new kind from source |
| `reload({ preserveState: false })` | Initialize all state from source |

Do not keep an old `NativeState` object across reload if the manifest can change. Read it again from `host.state(name)` after `reload()`.

## Observe events in Bun with `DynamicInputLane`

Realtime handlers do not call JavaScript. When ordinary Bun code also needs input notifications, attach a shared single-producer/single-consumer ring:

```ts
import {
  DynamicInputLane,
  InputDevice,
  InputEdge,
  Key,
  NativeHost,
} from "spellwire";

const lane = new DynamicInputLane(1024);
const host = await NativeHost.load("macro.spellwire.ts");

const unsubscribe = lane.on(
  InputDevice.Keyboard,
  Key.Q,
  InputEdge.Down,
  (event) => {
    const timestampNs =
      (BigInt(event.timestampHi >>> 0) << 32n) |
      BigInt(event.timestampLo >>> 0);
    console.log({ source: event.source, timestampNs });
  },
);

host.attachDynamicLane(lane);
let timer: ReturnType<typeof setInterval> | undefined;
try {
  host.start();
  timer = setInterval(() => {
    const drained = lane.drain(1024);
    if (drained > 0 || lane.ring.dropped > 0) {
      console.log({ drained, queued: lane.ring.size, dropped: lane.ring.dropped });
    }
  }, 8);
  await Bun.sleep(10_000);
} finally {
  if (timer !== undefined) clearInterval(timer);
  unsubscribe();
  host.close();
}
```

Capacity must be a power of two between 2 and 2^31. A full ring increments `lane.ring.dropped`; it never overwrites unread events. Choose capacity and drain cadence from measured traffic. This lane is best-effort control-plane plumbing, not a place for latency-critical automation.

`drain()` returns the number of records consumed. It cannot be called reentrantly on the same lane. Adding or removing a handler during dispatch affects subsequent events, not the current snapshot.

`host.dispatch(...)` is an explicit VM input used by tests and embedders. It does not replace the global observer and should not be confused with physical device input.

## Native library discovery

`NativeHost` checks paths in this order:

1. `nativeLibraryPath` or CLI `--library`;
2. `SPELLWIRE_NATIVE_LIBRARY`;
3. the packaged `native/<platform>-<arch>/` directory;
4. workspace `target/release/`;
5. workspace `target/debug/`.

The overlay uses the same pattern with `executablePath`, `SPELLWIRE_OVERLAY_EXECUTABLE`, the packaged directory, and workspace release/debug outputs.

Example explicit load:

```ts
const host = await NativeHost.load("macro.spellwire.ts", {
  nativeLibraryPath: "/absolute/path/to/libspellwire_native.dylib",
});
```

Use an absolute path when diagnosing discovery problems. Do not copy a library built for another OS or CPU architecture.

## Prevent accidental recursion

Synthetic output is observed again and tagged as `InputSource.Synthetic`. Prefer a physical-only trigger when an output might match its own input:

```ts
rt.onKeyDown(
  Key.Q,
  () => {
    tapKey(Key.Q);
  },
  { source: InputSource.Physical },
);
```

Use `InputSource.Any` only when recursion is intentional and bounded by state or another condition.

## Failure guide

| Message or symptom | Meaning | Action |
| --- | --- | --- |
| `Spellwire native library not found` | No matching library in discovery paths | Run `bun run build:native`, then check `--library` or `SPELLWIRE_NATIVE_LIBRARY` |
| `native ABI ... is incompatible` | JS wrapper and native library are from different builds | Rebuild both from the same commit; remove stale path overrides |
| `observe: missing` | Global observer cannot open | Grant Input Monitoring on macOS or evdev access on Linux |
| `inject: missing` | Injector cannot open | Grant Accessibility on macOS or `/dev/uinput` write access on Linux |
| `status -9` | Platform hook, event tap, device, or injection failed | Read the appended native error and follow [Platform Verification](platform-verification.md) |
| `status -10` | Native worker channel failed | Stop/close the host and reproduce with logs |
| `status -11` | Delayed-continuation capacity exhausted | Reduce overlapping delayed handlers or raise the native limit in a custom build |
| `unsupported USB HID key usage` | Backend has no safe translation for that key | Use a supported exported `Key` and verify the target keyboard layout |
| Reload succeeds but a value resets | State name/kind changed, or preservation was disabled | Compare the old and new manifests |
| Bun callback misses events | Dynamic lane filled before it was drained | Increase power-of-two capacity, drain more often, inspect `ring.dropped` |

For OS-specific commands and a copyable test report, continue with [Platform Verification](platform-verification.md). For compiler and simulator failures, see [Troubleshooting](troubleshooting.md).
