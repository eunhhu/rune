# API Reference

[한국어](api.ko.md)

This page documents the API that exists in the current source tree. It deliberately excludes older design sketches such as `macro(...)`, `spellwire.start()`, `rt.load(...)`, `on.keyDown(...)`, or `key.tap(...)`; those symbols are not exported by the current SDK.

## Package imports

```ts
import {
  InputSource,
  NativeHost,
  NativeOverlayRenderer,
  OverlayScene,
  Key,
  MouseButton,
  clickMouse,
  keyDown,
  keyHeld,
  keyUp,
  mouseDown,
  mouseHeld,
  mouseUp,
  moveMouse,
  rt,
  sleepUs,
  tapKey,
  wheelMouse,
} from "spellwire";
```

The compiler package exports programmatic compiler/encoder APIs. The CLI exposes three normal workflows:

```ts
import { compileSource, encodeModule } from "spellwire/compiler";
```

```bash
bunx spellwire run [macro.spellwire.ts]
bunx spellwire watch [macro.spellwire.ts]
bunx spellwire compile macro.spellwire.ts [output.spellwire.bin]
```

All three default to `src/main.spellwire.ts` when input is omitted. `run` and `watch` compile source in memory, prepare permissions once, then start the same owned native host. `watch` adds only control-plane filesystem reload.

## Realtime registration

### `rt.onKeyDown(key, handler, options?)`
### `rt.onKeyUp(key, handler, options?)`
### `rt.onMouseDown(button, handler, options?)`
### `rt.onMouseUp(button, handler, options?)`

Handlers must be top-level registration calls with an inline arrow function or function expression for AOT compilation.

```ts
rt.onKeyDown(
  Key.Q,
  () => {
    tapKey(Key.E);
  },
  { source: InputSource.Physical },
);
```

`options.source` accepts:

- `InputSource.Physical` — default;
- `InputSource.Synthetic`;
- `InputSource.Any`.

The compiler resolves key/button arguments and source options as constants.
The options object may be empty or contain one explicit, quoted, computed-string, or shorthand `source` property. Spreads and unknown properties fail compilation instead of defaulting silently.

## Realtime output intrinsics

These functions are lowered to native VM opcodes when called from compiled handlers:

```ts
keyDown(Key.E)
keyUp(Key.E)
tapKey(Key.E)

mouseDown(MouseButton.Left)
mouseUp(MouseButton.Left)
clickMouse(MouseButton.Left)

moveMouse(12, -4)
wheelMouse(0, 1)
sleepUs(80)
```

A zero-delay run of output intrinsics is collected into a fixed native output batch. In the live host, `sleepUs()` flushes the batch and yields into the fixed-capacity absolute-deadline scheduler. The compatibility engine/simulator wait synchronously.

Calling an output intrinsic directly in ordinary Bun code throws unless a fallback action sink has been installed with `withRealtimeActionSink()`.

## Held-input intrinsics

```ts
keyHeld(Key.LeftShift)
mouseHeld(MouseButton.Right)
```

The VM updates its input-state bitmap before invoking matching handlers. These functions read that bitmap without a platform query.

## Keys and buttons

`Key` values follow USB HID keyboard usage IDs. Examples:

```ts
Key.A
Key.Q
Key.Digit1
Key.Enter
Key.Escape
Key.Space
Key.F8
Key.ArrowUp
Key.LeftControl
Key.LeftShift
Key.LeftAlt
Key.LeftMeta
```

Mouse buttons:

```ts
MouseButton.Left
MouseButton.Right
MouseButton.Middle
MouseButton.Back
MouseButton.Forward
```

## Compiler API

### `compileSource(source, options?)`

```ts
const result = compileSource(source, {
  fileName: "macro.spellwire.ts",
  stackLimit: 128,
  instructionBudget: 100_000,
});
```

Returns:

- `module`: compiled states, handlers, instructions, local count, stack limit, and instruction budget;
- `sourceFile`: the parsed TypeScript source file.

Defaults are a stack limit of 128 and an instruction budget of 100,000 per handler dispatch. The native runtime caps the stack and local arrays at 256 entries.

Compilation failures throw `SpellwireCompileError` with file, line, column, and message diagnostics.

### `encodeModule(module)`

Serializes a compiled module to the versioned native binary format consumed by `spellwire-core::Program::decode` and `spellwire_engine_new`.

## JavaScript fallback/debug API

`rt.on*` also records JavaScript fallback registrations when the source file is executed normally by Bun. This path is for tests and debugging; it is not a realtime guarantee and does not install a global input observer.

### `getFallbackRealtimeRegistrations()`

Returns the registrations collected in the current process.

### `withRealtimeActionSink(sink, body)`

Temporarily installs a `RealtimeActionSink` so handler code can be exercised in JavaScript tests.

The sink receives:

```ts
interface RealtimeActionSink {
  key(code: number, down: boolean): void;
  mouseButton(button: number, down: boolean): void;
  mouseMove(dx: number, dy: number): void;
  mouseWheel(x: number, y: number): void;
  delayUs(duration: number): void;
  held(device: "keyboard" | "mouse", code: number): boolean;
}
```

## Dynamic control-plane lane

`DynamicInputLane` is a best-effort JavaScript lane backed by an SPSC `SharedArrayBuffer` ring. A native producer may write fixed six-word event records; Bun can drain them without scheduling a native-to-JS callback for each event.

```ts
const lane = new DynamicInputLane(1024);
const unsubscribe = lane.on(
  InputDevice.Keyboard,
  Key.Q,
  InputEdge.Down,
  (event) => console.log(event),
);

lane.drain();
unsubscribe();
```

This is control-plane plumbing, not the native realtime handler path.

Attach it to the live native producer before or after starting a host:

```ts
const host = await NativeHost.load("macro.spellwire.ts");
host.attachDynamicLane(lane);
host.start();

// Drain from a Bun worker/timer at an application-appropriate cadence.
lane.drain(1024);
host.detachDynamicLane();
```

Each delivered event is a stable readonly snapshot. Registering or unregistering handlers during dispatch affects only subsequent events, and `drain()` cannot be called reentrantly on the same lane.

## Native state wrapper

`NativeState<T>` wraps a numeric native state slot through a `NativeStateBridge`:

```ts
const enabled = new NativeState<boolean>(1, "boolean", bridge);
enabled.set(false);
console.log(enabled.get());
```

`NativeHost` constructs these wrappers from the compiler manifest:

```ts
const host = await NativeHost.load("macro.spellwire.ts");
host.start();
host.state("enabled").set(false);
console.log(host.states.phase?.get());
host.close();
```

`reload()` recompiles source/reloads bytes and preserves compatible state by source name and kind. `watch()` serializes filesystem-triggered reloads. `.bin` input uses the adjacent `.json` manifest or `manifestPath`.

## Native host and permissions

```ts
const host = await NativeHost.load("macro.spellwire.ts", {
  nativeLibraryPath: "/optional/explicit/library",
});

host.permissionStatus();
host.requestPermissions();
host.start();
await host.reload();
host.stop();
host.close();
```

The host resolves a packaged platform library, `SPELLWIRE_NATIVE_LIBRARY`, or workspace release/debug build. `close()` is idempotent. Stopping releases tracked synthetic held inputs.

| Member | Contract |
| --- | --- |
| `NativeHost.load(input, options?)` | Compile `.ts` in memory or load `.bin` plus manifest; validate ABI and allocate host |
| `permissionStatus()` | Return observe/inject bitmask without prompting |
| `requestPermissions()` | Request macOS grants; recheck current status on Windows/Linux |
| `start()` / `stop()` | Start or stop owned observer, injector, runtime worker, and scheduler |
| `reload({ preserveState? })` | Serialize reload and preserve running state by compatible manifest name/kind by default |
| `watch(options?)` | Watch the input file with configurable debounce and reload callbacks |
| `state(name)` / `states[name]` | Access a named `NativeState` from the current manifest |
| `attachDynamicLane(lane)` | Publish observed input into the lane's six-word shared records |
| `dispatch(...)` | Explicitly submit a VM input for tests or custom embedders |
| `close()` | Stop if needed, free native ownership, and close the dynamic library |

See [Live Native Host Guide](live-host.md) for complete long-running examples, signal handling, state-preservation rules, dynamic-lane timestamps/drops, library resolution order, and native error interpretation.

## Overlay scene model

`OverlayScene` retains text/rect/line nodes. `NativeOverlayRenderer.start()` launches the companion transparent renderer; `apply(scene)` sends only pending mutations. `show()`, `hide()`, `clear()`, and `close()` are control-plane operations. See [Overlay](overlay.md).

## Native ABI

The ABI contains both the owned platform host and a compatibility callback engine. See [Native C ABI](native-abi.md).
