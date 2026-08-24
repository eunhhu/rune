# API Reference

This page documents the API that exists in the current source tree. It deliberately excludes older design sketches such as `macro(...)`, `spellwire.start()`, `rt.load(...)`, `on.keyDown(...)`, or `key.tap(...)`; those symbols are not exported by the current SDK.

## Package imports

```ts
import {
  InputSource,
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

The compiler package exports programmatic compiler/encoder APIs and a CLI:

```ts
import { compileSource, encodeModule } from "spellwire/compiler";
```

```bash
bunx spellwire compile macro.spellwire.ts [output.spellwire.bin]
```

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

A zero-delay run of output intrinsics is collected into a fixed native output batch. `sleepUs()` flushes the current batch, advances an absolute monotonic deadline, and waits synchronously in the current VM implementation.

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

## Native state wrapper

`NativeState<T>` wraps a numeric native state slot through a `NativeStateBridge`:

```ts
const enabled = new NativeState<boolean>(1, "boolean", bridge);
enabled.set(false);
console.log(enabled.get());
```

The compiler-generated JSON manifest supplies names, slots, and kinds. The current repository does not yet provide the complete Bun FFI host that turns that manifest into named state properties.

## Overlay scene model

`OverlayScene` is an in-memory retained scene/mutation model. It does not create a window or render pixels yet. See [Overlay](overlay.md).

## Native ABI

The C ABI creates/owns a native VM, dispatches explicit input events, exposes state slots, and sends output batches to a host callback. See [Native C ABI](native-abi.md).
