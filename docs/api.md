# API Reference

Import public APIs from `spellwire`.

## Realtime registration

- `rt.onKeyDown(key, handler, options?)`
- `rt.onKeyUp(key, handler, options?)`
- `rt.onMouseDown(button, handler, options?)`
- `rt.onMouseUp(button, handler, options?)`

`options.source` accepts `InputSource.Physical`, `InputSource.Synthetic`, or `InputSource.Any`.

Module-scope integer and boolean `let` values referenced by a handler become persistent native state. Handler-local values reset for every dispatch.

## Output intrinsics

- `keyDown(key)` / `keyUp(key)` / `tapKey(key)`
- `mouseDown(button)` / `mouseUp(button)` / `clickMouse(button)`
- `moveMouse(dx, dy)` / `wheelMouse(x, y)`
- `sleepUs(duration)`

## Input state

- `keyHeld(key)`
- `mouseHeld(button)`

## Keys and buttons

`Key` uses USB HID keyboard usage IDs. `MouseButton` provides `Left`, `Right`, `Middle`, `Back`, and `Forward`.

## Compiler

The main package includes and exports:

- `compileSource(source, options?)`
- `encodeModule(module)`
- `SpellwireCompileError`
- compiler IR and diagnostic types from `spellwire/compiler`

CLI equivalents:

```bash
spellwire compile <input.spellwire.ts> [output.spellwire.bin]
spellwire <input.spellwire.ts> [output.spellwire.bin]
spellwire --help
spellwire --version
```

## Best-effort JavaScript lane

`DynamicInputLane` drains fixed records from a `SharedArrayBuffer` SPSC ring. It is useful for UI, logging, and dynamic subscriptions, but it does not share the native realtime latency contract.

`NativeState` exposes persistent VM state through a host-provided `NativeStateBridge`.

## JavaScript fallback testing

`withRealtimeActionSink()` executes output intrinsics against a test sink when a handler is evaluated as ordinary JavaScript. This is intended for unit tests, not realtime input dispatch.

## Overlay

`OverlayScene` is currently a retained scene-data model. Native transparent render backends are still in progress.

## Native ABI

`spellwire-native` exports a stable C ABI for:

- ABI and capability discovery
- engine creation and destruction
- input-event dispatch
- persistent-state get/set
- batched output callbacks

The current capability bitset advertises host callback injection only. Direct OS capabilities remain disabled until their backends are validated.
