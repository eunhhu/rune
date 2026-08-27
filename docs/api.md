# Spellwire API reference

[한국어](api.ko.md)

This reference covers project commands, realtime automation, persistent state, application lifecycle, and overlay UI. Related guides explain platform verification, implementation details, and design decisions.

## Quick lookup

| I want to… | Use | Section |
| --- | --- | --- |
| Create a project | `bun create spellwire my-automation` | [Create, run, watch, build](#create-run-watch-build) |
| Register a consuming chord | `rt.hotkey("Ctrl+K", handler)` | [Realtime registration](#realtime-registration) |
| Remap one key | `rt.remap("CapsLock", "Escape")` | [Realtime registration](#realtime-registration) |
| Keep native state | module-scope `let enabled = true` | [Persistent realtime state](#persistent-realtime-state) |
| Send keyboard/mouse output | `tapKey`, `clickMouse`, `moveMouse` | [Realtime output intrinsics](#realtime-output-intrinsics) |
| Delay a handler | `sleep.ms(250)` or `sleep.seconds(2)` | [Realtime output intrinsics](#realtime-output-intrinsics) |
| Start input, watch, and UI together | `Spellwire.start(options)` | [Application lifecycle](#unified-application-lifecycle) |
| Show state in an overlay | `overlay: state => ui.text(...)` | [Modern overlay](#modern-overlay) |
| Build rows, columns, panels, and stacks | `ui.row`, `ui.column`, `ui.panel` | [UI constructors](#ui-constructors) |
| Set size, padding, gap, fill, border, shadow, or font | element props | [Layout and visual properties](#layout-and-visual-properties) |
| Bind only one UI subtree | `ui.bind(state, render)` | [Bindings and refresh](#bindings-and-refresh) |
| Show, hide, or manually refresh UI | `overlay.show()`, `hide()`, `app.refreshOverlay()` | [Overlay lifecycle](#overlay-lifecycle) |
| Configure topmost/transparency/focus | `overlayOptions.window` | [Window behavior](#window-behavior) |
| Control the native host directly | `NativeHost` | [Native host and permissions](#native-host-and-permissions) |

## Example: state-driven input and overlay

Generated projects contain one `src/main.ts`. The compiler extracts only realtime handlers into native bytecode; the same file's unrestricted application and overlay code stays on Bun. This keeps authoring unified without moving JavaScript onto the input-event path.

```ts
import { Key, Spellwire, rt, tapKey, ui } from "spellwire";

let enabled = true;
let presses = 0;

rt.hotkey("Q", () => {
  presses += 1;
  tapKey(Key.E);
}, { when: () => enabled });

rt.hotkey("F8", () => {
  enabled = !enabled;
}, { consume: false });

rt.remap("CapsLock", "Escape", { when: () => enabled });

const app = await Spellwire.start({
  input: import.meta.file,
  watch: Bun.argv.includes("--watch"),
  overlayOptions: { window: { title: "Spellwire Status" } },
  overlay: (state) => {
    const enabled = state.enabled === true;
    return ui.column(
      {
        x: 24,
        y: 48,
        width: 280,
        padding: 16,
        gap: 12,
        fill: "#111827ee",
        radius: 16,
        stroke: "#ffffff24",
        shadow: { fill: "#00000066", y: 8, blur: 24 },
      },
      ui.row(
        { width: "fill", gap: 8, align: "center" },
        ui.dot({ size: 8, fill: enabled ? "#34d399ff" : "#fb7185ff" }),
        ui.text(enabled ? "Active" : "Paused", {
          width: "fill",
          fill: "#ffffffff",
          fontSize: 16,
          fontWeight: 600,
        }),
        ui.badge("F8"),
      ),
      ui.text(`Q presses: ${String(state.presses ?? 0)}`, {
        fill: "#cbd5e1ff",
        fontFamily: "monospace",
        fontSize: 13,
      }),
    );
  },
});

await app.untilSignal();
```

## Create, run, watch, build

```bash
bun create spellwire my-automation
cd my-automation
bun run start
bun run watch
bun run build
```

| Command | Behavior |
| --- | --- |
| `bun run start` | Compile source in memory, prepare permissions, and run immediately |
| `bun run watch` | Run with native program reload after source changes |
| `bun run build` | Write `dist/main.spellwire.bin` and its named-state manifest |

Direct CLI equivalents:

```bash
bunx spellwire run [macro.spellwire.ts]
bunx spellwire watch [macro.spellwire.ts]
bunx spellwire compile macro.spellwire.ts [output.spellwire.bin]
```

Input defaults to `src/main.ts`, then falls back to legacy `src/main.spellwire.ts` when present. CLI `run` and `watch` operate the realtime native host; generated `bun run start` / `watch` execute the unified application including overlay code. Both compile realtime handlers in memory and prepare permissions once. Watch mode adds only control-plane filesystem reload.

## Package imports

```ts
import {
  InputSource,
  NativeCapability,
  NativeHost,
  NativeOverlayRenderer,
  Overlay,
  OverlayScene,
  Spellwire,
  Key,
  Modifier,
  MouseButton,
  clickMouse,
  keyDown,
  keyHeld,
  keyUp,
  mouseDown,
  mouseHeld,
  mouseUp,
  moveMouse,
  parseHotkey,
  rt,
  sleep,
  sleepHours,
  sleepMinutes,
  sleepMs,
  sleepSeconds,
  sleepUs,
  tapKey,
  ui,
  wheelMouse,
} from "spellwire";
```

Most applications need only `Spellwire`, `ui`, `rt`, keys/buttons, and output intrinsics. `NativeHost`, low-level overlay classes, compiler helpers, and fallback helpers are advanced escape hatches.

The compiler package exports programmatic compiler/encoder APIs:

```ts
import { compileSource, encodeModule } from "spellwire/compiler";
```

## Public export index

The package root exports the following public names. The detailed sections below cover the common application APIs; lower-level types are included here for editor search and library integration.

| Area | Exports |
| --- | --- |
| Keys and hotkeys | `Key`, `Modifier`, `MouseButton`, `InputSource`, `parseHotkey`, `ParsedHotkey` |
| Application | `Spellwire`, `SpellwireStartOptions` |
| Realtime registration | `rt`, `HotkeyOptions`, `RemapOptions`, `RealtimeOptions`, `RealtimeRegistration` |
| Realtime output | `keyDown`, `keyUp`, `tapKey`, `keyHeld`, `mouseDown`, `mouseUp`, `clickMouse`, `mouseHeld`, `moveMouse`, `wheelMouse`, `sleep`, `sleepUs`, `sleepMs`, `sleepSeconds`, `sleepMinutes`, `sleepHours` |
| Fallback testing | `getFallbackRealtimeRegistrations`, `withRealtimeActionSink`, `RealtimeActionSink` |
| Runtime and dynamic lane | `DynamicInputLane`, `EventSource`, `InputDevice`, `InputEdge`, `InputEvent`, `InputHandler`, `NativeState`, `NativeStateBridge`, `SpscInt32Ring` |
| Overlay runtime | `Overlay`, `OverlayView`, `OverlayScene`, `NativeOverlayRenderer`, `OverlayMountOptions`, `OverlayBindingOptions`, `OverlayReadable`, `OverlayStateSource` |
| Overlay UI | `ui`, `OverlayChild`, `OverlayElement`, `OverlayLayoutProps`, `OverlayFrameProps`, `OverlayTextProps`, `OverlayEllipseProps`, `OverlayDotProps`, `OverlayDividerProps`, `OverlayBadgeProps`, `OverlayAlign`, `OverlayJustify`, `OverlayInsets`, `OverlayLength` |
| Overlay primitives | `OverlayNode`, `OverlayNodeId`, `OverlayMutation`, `OverlayText`, `OverlayRect`, `OverlayEllipse`, `OverlayLine`, `OverlayFont`, `OverlayStroke`, `OverlayShadow` |
| Overlay process and window | `OverlayWindowOptions`, `ResolvedOverlayWindowOptions`, `NativeOverlayOptions`, `NativeOverlayReady`, `overlayExecutableFileName`, `resolveOverlayExecutable`, `resolveOverlayWindowOptions` |
| Native host | `NativeHost`, `NativeHostOptions`, `NativeHostWatcher`, `NativeWatchOptions`, `NativeManifest`, `ProgramDescriptor`, `NativeRuntimeInfo`, `NativeStateManifestEntry`, `NativeStateSnapshot` |
| Native capabilities | `NativeCapability`, `NativePermission`, `NATIVE_ABI_VERSION`, `inspectNativeRuntime`, `loadProgramDescriptor`, `nativeLibraryFileName`, `resolveNativeLibrary` |
| Compiler | `compileSource`, `SpellwireCompileError`, `CompileDiagnostic`, `CompileOptions`, `CompileResult`, `encodeModule` |
| Wire format and IR | `Opcode`, `SourceFilter`, `TriggerFlag`, `WIRE_VERSION`, `WIRE_HEADER_SIZE`, `WIRE_HANDLER_SIZE`, `WIRE_INSTRUCTION_SIZE`, `CompiledModule`, `Handler`, `Instruction`, `StateSlot` |

## Persistent realtime state

A module-scope `let` initialized with a safe integer or boolean becomes a persistent native `i64` state slot when referenced by a realtime handler:

```ts
let enabled = true;
let count = 0;

rt.hotkey("F8", () => {
  enabled = !enabled;
  count += 1;
});
```

Assignments, compound assignments, `++`/`--`, integer arithmetic, comparisons, boolean logic, bitwise operations, conditions, and bounded loops compile to native VM opcodes. Realtime dispatch uses numeric slots, not state-name lookup, JavaScript objects, or FFI. Common discarded updates such as `count++`, `count += 4`, `mask ^= 1`, `enabled = !enabled`, and constant stores compile directly to one slot/immediate opcode with no VM stack round trip. Module-scope `const` values are folded when statically representable.

State survives dispatches. Source reload preserves values by matching state name and kind unless `preserveState: false` is selected. `when` accepts one module-scope boolean state or its negation because the native suppression table must evaluate the same gate before dispatch.

Outside realtime handlers, use `app.host.state("name")`, `app.host.states.name`, or one bulk `app.host.snapshotStates()`. Those are control-plane FFI calls, not realtime opcodes.

## Realtime registration

### `rt.hotkey(chord, handler, options?)`

Registers a portable modifier chord or mouse chord. It consumes original input and requires exact modifiers by default.

```ts
let enabled = true;

rt.hotkey("Ctrl+Shift+K", () => {
  tapKey(Key.Enter);
}, {
  source: InputSource.Physical,
  consume: true,
  exactModifiers: true,
  repeat: false,
  edge: "down",
  when: () => enabled,
});
```

`edge` is `"down"` or `"up"`. `when` must return one module-scope boolean native state or its negation. The gate controls both VM dispatch and original-input suppression. `parseHotkey(chord)` exposes the same parser for validation/tooling and returns `{ device, code, modifiers }`; logical modifier bits are exported as `Modifier`.

Chord grammar is one trigger plus zero or more logical modifiers separated by `+`:

- modifiers: `Ctrl`/`Control`, `Shift`, `Alt`/`Option`, `Meta`/`Cmd`/`Command`/`Win`/`Super`;
- trigger: any exported `Key` member name, `A`–`Z`, `0`–`9`, common aliases such as `Esc`, `Return`, `PgUp`, `PgDn`, `Spacebar`, or mouse aliases `LButton`, `RButton`, `MButton`, `XButton1`, `XButton2`;
- names ignore case, spaces, `_`, and `-`;
- exactly one non-modifier trigger is required; combinations such as `A+B` are rejected.

```ts
rt.hotkey("Cmd+Space", handler);
rt.hotkey("Ctrl+Alt+K", handler);
rt.hotkey("Shift+LButton", handler);
```

### `rt.remap(from, to, options?)`

Compiles paired key down/up handlers and consumes the accepted source sequence:

```ts
rt.remap("CapsLock", "Escape", { when: () => enabled });
rt.remap(Key.CapsLock, Key.Escape, { repeat: false });
```

Both keys may be a single portable string name or `Key` value. Options are `source`, `repeat`, and `when`.

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

Low-level options are:

- `source`: `InputSource.Physical` (default), `Synthetic`, or `Any`;
- `consume`: original-input suppression, default `false` for low-level registrations;
- `modifiers`: `Modifier` bitmask;
- `exactModifiers`: reject extra modifiers, default `false`;
- `repeat`: accept repeat downs, default `true`;
- `when`: module-scope native boolean state gate.

The compiler resolves trigger arguments and options statically. Properties may use identifiers, quoted names, computed string names, or shorthand constant values. Spreads, duplicate properties, dynamic values, and unknown options fail compilation instead of defaulting silently.

For unusual release/suppression cases and the AutoHotkey migration matrix, the optional deep dive is [Automation semantics](automation.md).

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
sleep.us(80)
sleep.ms(250)
sleep.seconds(2)
sleep.minutes(1)
sleep.hours(1)
```

A zero-delay run of output intrinsics is collected into a fixed native output batch. In the live host, every `sleep.*()` helper flushes the batch and yields into the fixed-capacity absolute-deadline scheduler. The compatibility engine/simulator wait synchronously.

| API | Native effect |
| --- | --- |
| `keyDown(key)` / `keyUp(key)` | Emit one keyboard transition |
| `tapKey(key)` | Emit paired down/up transitions |
| `mouseDown(button)` / `mouseUp(button)` | Emit one mouse-button transition |
| `clickMouse(button)` | Emit paired mouse down/up transitions |
| `moveMouse(dx, dy)` | Emit relative pointer movement |
| `wheelMouse(x, y)` | Emit horizontal/vertical wheel movement |
| `sleep.us(duration)` / `sleepUs(duration)` | Microseconds |
| `sleep.ms(duration)` / `sleepMs(duration)` | Milliseconds |
| `sleep.seconds(duration)` / `sleepSeconds(duration)` | Seconds |
| `sleep.minutes(duration)` / `sleepMinutes(duration)` | Minutes |
| `sleep.hours(duration)` / `sleepHours(duration)` | Hours |

The namespace form is the shortest API; named helpers remain convenient for direct imports. Constant durations are scaled at compile time. Dynamic durations carry their unit scale in the same `DelayUs` instruction, so a helper call still costs one native delay opcode—there is no conversion callback, extra timer, or JavaScript work in the realtime path. Encoded microseconds use a non-negative signed 64-bit value; an unsupported platform deadline is rejected instead of wrapping. The JavaScript fallback additionally requires safe-integer input and output.

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

Inspect the current native library without starting the host:

```bash
bun run inspect:runtime
bun run inspect:runtime -- --request-permissions
```

The second command requests or refreshes platform permissions. Both print the ABI version, native library path, enabled capability names, and observe/inject permission flags.

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

`host.capabilities & NativeCapability.NativeInputSuppression` is nonzero on Windows/macOS and zero on the current Linux backend.

| Member | Contract |
| --- | --- |
| `NativeHost.load(input, options?)` | Compile `.ts` in memory or load `.bin` plus manifest; validate ABI and allocate host |
| `permissionStatus()` | Return observe/inject bitmask without prompting |
| `requestPermissions()` | Request macOS grants; recheck current status on Windows/Linux |
| `start()` / `stop()` | Start or stop owned observer, injector, runtime worker, and scheduler |
| `reload({ preserveState? })` | Serialize reload and preserve running state by compatible manifest name/kind by default |
| `watch(options?)` | Watch the input file with configurable debounce and reload callbacks |
| `state(name)` / `states[name]` | Access a named `NativeState` from the current manifest |
| `snapshotStates()` | Read every named state with one bulk native worker command |
| `attachDynamicLane(lane)` | Publish observed input into the lane's six-word shared records |
| `dispatch(...)` | Explicitly submit a VM input for tests or custom embedders |
| `close()` | Stop if needed, free native ownership, and close the dynamic library |

See [Live Native Host Guide](live-host.md) for complete long-running examples, signal handling, state-preservation rules, dynamic-lane timestamps/drops, library resolution order, and native error interpretation.

## Unified application lifecycle

Use this API for normal applications:

```ts
const app = await Spellwire.start({
  input: import.meta.file,
  watch: true,
  overlay: (state) => ui.text(String(state.count ?? 0)),
});

await app.untilSignal();
```

`Spellwire.start(options)` owns host load, permission preparation, native input start, optional file watch, state-driven overlay, and safe shutdown.

| `SpellwireStartOptions` | Default | Contract |
| --- | --- | --- |
| `input` | `"src/main.ts"`, then legacy source | Realtime TypeScript or compiled `.bin` path |
| `watch` | `false` | Watch input source and reload the native program |
| `debounceMs` | host default | Filesystem reload debounce |
| `preserveState` | `true` | Preserve compatible named state on reload |
| `requestPermissions` | `true` | Check/request observe and inject permissions before start |
| `onReload` | — | Called after a successful watched reload |
| `onError` | console/default propagation | Receives watch or asynchronous overlay failures |
| `overlay(state)` | — | Build an overlay from one shallow named-state snapshot |
| `overlayOptions` | — | Overlay polling, renderer startup, and native window options listed below |
| `nativeLibraryPath` | auto-discovered | Explicit native library override |
| `manifestPath` | adjacent manifest | Explicit manifest for compiled binary input |

| App member | Contract |
| --- | --- |
| `app.host` | Started `NativeHost`, including `states`, `reload`, and snapshots |
| `app.overlay` | Mounted `Overlay`, or `undefined` when no overlay callback was supplied |
| `app.refreshOverlay()` | Force one binding read/reconciliation pass; useful with `fps: 0` |
| `app.untilSignal()` | Wait for `SIGINT`/`SIGTERM`, then close safely |
| `app.close()` | Stop watcher, close renderer, stop host, and release tracked synthetic input |

## Modern overlay

`Spellwire.start({ overlay })` is the shortest state-to-screen path. `Overlay.mount(tree, options?)` is the standalone API. Both use a native retained renderer; there is no DOM, WebView, React, or per-frame JavaScript drawing callback.

### UI constructors

| API | Result |
| --- | --- |
| `ui.row(props, ...children)` | Horizontal auto layout |
| `ui.column(props, ...children)` | Vertical auto layout |
| `ui.panel(props, ...children)` | Semantic alias for a vertical frame |
| `ui.stack(props, ...children)` | Layer children from one padded origin |
| `ui.box(...)` / `ui.frame(...)` | Aliases for `ui.stack(...)` |
| `ui.text(value, props?)` | Text primitive |
| `ui.ellipse(props?)` | Ellipse primitive |
| `ui.dot({ size, ...props })` | Equal-width/height ellipse convenience |
| `ui.divider(props?)` | One-pixel fill-width divider by default |
| `ui.badge(label, props?)` | Styled frame plus text convenience |
| `ui.spacer(sizeOrProps?)` | Empty layout space |
| `ui.bind(source, render, options?)` | Cached state-bound subtree |
| `ui.when(source, content, fallback?)` | Conditional state-bound subtree |

Children may contain nested arrays, `false`, `null`, or `undefined`. Use `key` when conditional insertion or sibling reordering must preserve identity.

### Layout and visual properties

Common layout props apply to frames, text, ellipses, and spacers:

| Prop | Type | Meaning |
| --- | --- | --- |
| `key` | `string` | Stable reconciliation identity |
| `x`, `y` | `number` | Logical-pixel offset |
| `width`, `height` | `number \| "fill"` | Fixed size or remaining parent space; omission hugs content |
| `minWidth`, `minHeight` | `number` | Minimum measured size |
| `maxWidth`, `maxHeight` | `number` | Maximum measured size |
| `opacity` | `number` | Element opacity; parent opacity multiplies into descendants |

Frame-only props:

| Prop | Type / values |
| --- | --- |
| `padding` | `number` or `{ x?, y?, top?, right?, bottom?, left? }`; side-specific values win |
| `gap` | logical pixels between flow children |
| `align` | `"start" \| "center" \| "end" \| "stretch"` |
| `justify` | `"start" \| "center" \| "end" \| "space-between"` |
| `fill` | `#RRGGBB` or `#RRGGBBAA` |
| `radius` | corner radius in logical pixels |
| `stroke` | color string for 1 px, or `{ fill, width }` |
| `shadow` | `{ fill, x?, y?, blur?, spread? }` |

Text-only props:

| Prop | Type / values |
| --- | --- |
| `fill` | text color, `#RRGGBB` or `#RRGGBBAA` |
| `fontFamily` | `"system" \| "monospace"` |
| `fontSize`, `fontWeight`, `lineHeight`, `letterSpacing` | `number` |
| `textAlign` | `"left" \| "center" \| "right"` |

Ellipse supports `fill`, `stroke`, and `shadow`. `ui.dot` adds `size`. `ui.badge` adds `textFill`, `fontFamily`, `fontSize`, and `fontWeight` while retaining all frame props.

Example using the full modern style vocabulary:

```ts
ui.row(
  {
    key: "status",
    x: 24,
    y: 48,
    width: 320,
    minHeight: 56,
    padding: { x: 16, y: 12 },
    gap: 10,
    align: "center",
    justify: "space-between",
    fill: "#111827ee",
    radius: 16,
    stroke: { fill: "#ffffff30", width: 1 },
    shadow: { fill: "#00000066", y: 8, blur: 24 },
    opacity: 0.96,
  },
  ui.text("Active", {
    width: "fill",
    fill: "#ffffffff",
    fontFamily: "system",
    fontSize: 16,
    fontWeight: 600,
    lineHeight: 20,
    letterSpacing: 0.2,
  }),
  ui.badge("F8"),
);
```

### Bindings and refresh

`ui.bind` accepts `NativeState`, `NativeHost`, a getter function, or an object implementing `get()` / `snapshotStates()`:

```ts
const overlay = await Overlay.mount(
  ui.column(
    { padding: 12, gap: 8, fill: "#111827ee" },
    ui.bind(host.states.enabled, (enabled) =>
      ui.text(enabled ? "Enabled" : "Paused"),
    ),
    ui.bind(host.states.count, (count) => ui.text(`Count: ${count}`)),
  ),
);
```

`ui.bind` compares with shallow equality by default; `options.equals(left, right)` may replace it. Each unique source is read once per reconciliation pass. An unchanged pass performs no layout or renderer IPC. A changed binding reruns only its render callback, then the resolved tree is laid out and keyed primitives are diffed. Only changed primitives cross the process boundary; the native renderer redraws only affected bounds.

`Spellwire.start({ overlay })` binds one bulk host snapshot to the root callback. Any named-state change reruns that root callback. This is React-like render/reconcile behavior, not automatic signal dependency tracking. Use direct per-state `ui.bind` when callback-level granularity matters.

### Overlay lifecycle

```ts
const overlay = await Overlay.mount(tree, {
  fps: 30,
  executablePath: "/optional/spellwire-overlay",
  readyTimeoutMs: 5_000,
  window: { title: "Status", alwaysOnTop: true, clickThrough: true },
  onError: console.error,
});

await overlay.set(nextTree);
await overlay.refresh();
await overlay.hide();
await overlay.show();
await overlay.close();
```

| `OverlayMountOptions` | Default | Contract |
| --- | --- | --- |
| `fps` | `30` | Binding polls per second, `0` for manual refresh; valid range 0–240 |
| `executablePath` | auto-discovered | Explicit native renderer path |
| `readyTimeoutMs` | `5_000` | Renderer startup timeout |
| `window` | overlay-safe defaults | Native window policy; see the table below |
| `onError` | console | Asynchronous refresh error callback |
| `renderer` | create one | Reuse an existing `NativeOverlayRenderer`; unavailable through `SpellwireStartOptions.overlayOptions` |

Static trees create no timer. Poll ticks coalesce while a refresh is in flight, so slow updates do not build a backlog.

### Window behavior

`OverlayMountOptions.window`, `SpellwireStartOptions.overlayOptions.window`, and low-level `NativeOverlayRenderer.start({ window })` share this policy:

| `OverlayWindowOptions` | Default | Native request |
| --- | --- | --- |
| `title` | `"Spellwire Overlay"` | Window title, 1–256 characters |
| `transparent` | `true` | Request native transparent-window behavior and an alpha-capable surface when the backend supports it |
| `alwaysOnTop` | `true` | Always-on-top window level |
| `focusable` | `false` | Whether the overlay may activate/accept focus |
| `clickThrough` | `true` | Disable pointer hit testing when true |
| `decorations` | `false` | Native title bar/border |
| `resizable` | `false` | User resize policy |
| `visible` | `true` | Initial visibility; `show()` / `hide()` still work |

```ts
const app = await Spellwire.start({
  input: import.meta.file,
  overlayOptions: {
    window: {
      title: "Macro status",
      transparent: true,
      alwaysOnTop: true,
      focusable: false,
      clickThrough: true,
      decorations: false,
      resizable: false,
      visible: true,
    },
  },
  overlay: (state) => ui.text(String(state.enabled ?? false)),
});
```

The ready message exposes the validated, fully default-resolved request as `overlay.renderer.ready.window`; `overlay.renderer.ready.alphaMode` reports the surface mode selected by wgpu. The renderer is a native winit/wgpu process with no WebView layer. `focusable` and `clickThrough` are separate: a focusable window may ignore pointer hits, and a non-focusable window may receive them. macOS uses the prohibited activation policy when non-focusable and the accessory policy when focusable; Windows disables the native window when non-focusable; Linux uses the available winit/compositor hints.

Windows lifecycle and window-policy smoke tests pass, but current wgpu backends may report `alphaMode: "Opaque"`; the policy result alone does not prove per-pixel visual transparency. Verify transparency visually on the target Windows desktop. Focus, topmost, click-through, and transparency behavior can also vary across X11/Wayland compositors, so Linux requires a target-desktop check.

Initial monitor and size remain the primary monitor's full bounds; monitor routing and explicit window geometry are not yet public. On Windows, `focusable: false` also makes the window non-interactive. Set `focusable: true` for an interactive decorated tool window.

### Low-level retained overlay

`OverlayScene` and `NativeOverlayRenderer` are escape hatches for callers that already compute final coordinates. Primitive kinds are `text`, `rect`, `ellipse`, and `line`.

```ts
const renderer = await NativeOverlayRenderer.start();
const scene = new OverlayScene();
const id = scene.create({ kind: "text", x: 20, y: 20, text: "Ready", size: 16 });
await renderer.apply(scene);

scene.update(id, { kind: "text", x: 20, y: 20, text: "Running", size: 16 });
await renderer.apply(scene);

scene.remove(id);
await renderer.apply(scene);
await renderer.close();
```

Pending changes coalesce by node. Equal updates are no-ops and `apply(scene)` sends at most one mutation batch.

## Native ABI

The ABI contains both the owned platform host and a compatibility callback engine. See [Native C ABI](native-abi.md).
