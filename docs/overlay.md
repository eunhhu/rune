# State-driven native overlay

[한국어](overlay.ko.md)

Spellwire's modern overlay API combines native state, Figma-style auto layout, retained diffing, and safe lifecycle ownership. No absolute coordinate math or manual `scene.update()` / `renderer.apply()` loop is required.

For the matching realtime hotkey source and the complete input → native state → overlay update path, see [Hotkeys and state-driven automation](automation.md#one-state-drives-input-and-overlay).

## Complete state + overlay program

```ts
import { Key, Spellwire, rt, tapKey, ui } from "spellwire";

let enabled = true;
let activations = 0;

rt.hotkey("Q", () => {
  activations++;
  tapKey(Key.E);
}, { when: () => enabled });

rt.hotkey("F8", () => {
  enabled = !enabled;
}, { consume: false });

const app = await Spellwire.start({
  input: import.meta.file,
  watch: true,
  overlayOptions: {
    window: { title: "Spellwire Status", alwaysOnTop: true, clickThrough: true },
  },
  overlay: (state) => {
    const enabled = state.enabled === true;
    return ui.column(
      {
        x: 24, y: 48, width: 300, padding: 16, gap: 12,
        fill: "#111827ee", radius: 16, stroke: "#ffffff24",
        shadow: { fill: "#00000066", y: 8, blur: 24 },
      },
      ui.text("SPELLWIRE", {
        fill: "#94a3b8ff", fontSize: 12, fontWeight: 700, letterSpacing: 1,
      }),
      ui.row(
        { width: "fill", gap: 8, align: "center" },
        ui.dot({ size: 8, fill: enabled ? "#34d399ff" : "#fb7185ff" }),
        ui.text(enabled ? "Active" : "Paused", {
          width: "fill", fontSize: 16, fontWeight: 600,
        }),
        ui.badge("F8"),
      ),
      ui.divider(),
      ui.row(
        { width: "fill", justify: "space-between" },
        ui.text("Activations", { fill: "#94a3b8ff" }),
        ui.text(String(state.activations ?? 0), { fontFamily: "monospace" }),
      ),
    );
  },
});

await app.untilSignal();
```

`Spellwire.start()` owns the native host, permission request, optional source watcher, overlay process, state binding, and shutdown. The compiler reads this same file but extracts only `rt.*` handlers for native execution; overlay code remains unrestricted Bun TypeScript. `untilSignal()` handles `SIGINT`/`SIGTERM`, closes the renderer, stops the host, and releases held synthetic input.

The runnable repository version is [`examples/state-overlay.ts`](../examples/state-overlay.ts).

## How a native state change reaches the screen

1. A realtime handler updates a persistent native state slot; JavaScript is not called from input dispatch.
2. The native worker publishes only changed state through the SPSC event lane, and `Spellwire.start` reads the maintained named-state cache after that change.
3. If the shallow state snapshot is unchanged, layout and renderer IPC are skipped.
4. If it changed, the `overlay(state)` function returns a lightweight element tree.
5. Stable keys/path positions reconcile that tree against retained primitives. Unchanged primitives emit no mutation.
6. All mutations are coalesced by node and sent as one native batch.
7. The renderer clears and rasterizes only the union of old/new affected bounds, uploads only that aligned texture region, and presents the retained texture.

This path is separate from the realtime input callback. Static overlays create no JavaScript timer, per-frame callback, or recurring IPC.

`Spellwire.start({ overlay })` subscribes to changed-state records and creates no overlay polling timer by default. For fully manual refresh, set zero explicitly and update only at known control-plane boundaries:

```ts
const app = await Spellwire.start({
  overlayOptions: { fps: 0 },
  overlay: (state) => ui.text(String(state.activations ?? 0)),
});

await app.refreshOverlay();
```

## Reactivity model

The root `overlay(state)` callback is React-like render/reconcile: one cached named-state snapshot is compared shallowly, and a changed slot reruns that root callback. It is not automatic signal dependency tracking. The retained layer is still fine-grained: keyed primitives are compared individually, unchanged nodes produce no IPC, and the renderer redraws only affected bounds.

For callback-level fine granularity, use `ui.bind(host.states.enabled, render)` or another narrow readable source. Only the changed binding callback reruns. This explicit split avoids proxies, dependency tracking, allocations, and JavaScript work in the realtime input path.

## UI constructors

| API | Purpose |
|---|---|
| `ui.row(props, ...children)` | Horizontal auto layout |
| `ui.column(props, ...children)` / `ui.panel(...)` | Vertical auto layout |
| `ui.stack(props, ...children)` / `ui.frame(...)` / `ui.box(...)` | Layered or absolute-in-frame composition |
| `ui.text(value, props)` | System/monospace text with size, weight, line height, tracking, alignment |
| `ui.ellipse(props)` / `ui.dot(props)` | Ellipse and status-dot primitives |
| `ui.divider(props)` | Fill-width separator |
| `ui.badge(label, props)` | Compact auto-sized label surface |
| `ui.spacer(sizeOrProps)` | Fixed or fill layout space |
| `ui.bind(source, render, options)` | Bind `NativeState`, `NativeHost`, getter, or custom readable source |
| `ui.when(source, content, fallback)` | Conditional retained subtree |

Children may be nested arrays, `false`, `null`, or `undefined`, so normal TypeScript conditional composition works.

## Layout properties

Frames support horizontal, vertical, and stacked auto layout:

```ts
ui.row({
  x: 24,
  y: 48,
  width: 320,          // number or "fill"
  height: "fill",
  minWidth: 200,
  maxWidth: 480,
  padding: { x: 16, y: 12 },
  gap: 8,
  align: "center",     // start | center | end | stretch
  justify: "space-between", // start | center | end | space-between
});
```

- Numeric dimensions use logical overlay pixels. The renderer applies the monitor scale factor once at the process boundary, so 300 remains 300 points on Retina instead of shrinking to 150.
- `"fill"` consumes remaining space on the corresponding auto-layout axis.
- Omitted dimensions hug content.
- `padding` accepts one number or `{ x, y, top, right, bottom, left }`; side-specific values win.
- `row` and `column` perform flow layout. `stack` layers children from the same padded origin and honors child `x`/`y` offsets.
- `key` preserves identity when conditionally inserting or reordering siblings.

## Visual properties

Frames and ellipses support:

```ts
{
  fill: "#111827ee",
  stroke: { fill: "#ffffff30", width: 1 }, // color string means 1 px
  radius: 16,                              // frames
  shadow: { fill: "#00000066", x: 0, y: 8, blur: 24, spread: 0 },
  opacity: 0.96,
}
```

Text supports:

```ts
{
  fill: "#ffffffff",
  opacity: 1,
  fontFamily: "system",     // system | monospace
  fontSize: 16,
  fontWeight: 600,
  lineHeight: 20,
  letterSpacing: 0.2,
  textAlign: "left",        // left | center | right
}
```

Colors are `#RRGGBB` or `#RRGGBBAA`. Parent opacity multiplies through descendants. Use visible text or shape changes with color for important states; do not make color the only status signal.

## Native window options

Configure native window behavior through `overlayOptions.window` (or `Overlay.mount(..., { window })` / `NativeOverlayRenderer.start({ window })`):

```ts
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
```

These values are the defaults except for the title. `clickThrough` controls pointer hit testing; `focusable` separately controls activation/focus. `visible: false` creates the renderer hidden until `show()`. The validated values are available at `app.overlay?.renderer.ready.window`.

The renderer uses a native winit/wgpu window and surface with no DOM or WebView layer. macOS uses the prohibited activation policy when non-focusable and the accessory policy when focusable; Windows disables a non-focusable window; Linux applies the hints exposed by winit. `app.overlay?.renderer.ready.alphaMode` reports the selected surface mode. Windows policy/live-update smoke tests pass, but `alphaMode: "Opaque"` requires a separate visual transparency check. X11/Wayland compositor rules can differ, so Linux needs target-desktop verification. Primary-monitor full bounds remain fixed startup geometry; public multi-monitor routing is still pending.

## Direct binding without `Spellwire.start()`

`ui.bind` accepts a single `NativeState`, a `NativeHost` snapshot source, a getter, or an object implementing `get()` / `snapshotStates()`:

```ts
const overlay = await Overlay.mount(
  ui.column(
    { padding: 12, fill: "#111827ee" },
    ui.bind(host.states.enabled, (enabled) =>
      ui.text(enabled ? "Enabled" : "Paused"),
    ),
  ),
);
```

Multiple bindings to the same source are read once per reconciliation pass. Prefer binding the host once when several values are displayed; it reads one maintained cache instead of one FFI call per state.

`OverlayMountOptions.fps` accepts 0–240; zero means manual refresh. Direct `Overlay.mount` defaults to 30, while `Spellwire.start` uses changed-state events unless an explicit `fps` is supplied. `executablePath`, `readyTimeoutMs`, and `window` control native startup, while `onError` handles asynchronous refresh failures.

## Low-level retained escape hatch

`OverlayScene` and `NativeOverlayRenderer` remain available for custom engines. Primitive nodes are `text`, `rect`, `ellipse`, and `line`. `create`, `update`, and `remove` coalesce pending changes by node; `apply(scene)` sends one batch. Repeating an equal `update` is a no-op.

Use this layer only when another layout engine already provides final coordinates. Application UI should prefer `Overlay.mount()` or `Spellwire.start()`.

## Performance contract

- no JavaScript call from realtime input dispatch;
- no layout, IPC, or renderer work when state and scene are unchanged;
- no default overlay polling timer in `Spellwire.start`; changed state triggers one cached snapshot reconciliation;
- explicit polling ticks coalesce instead of building a backlog when a refresh takes longer than its interval;
- one binding read per unique source;
- keyed primitive equality checks without JSON hashing;
- one coalesced IPC batch and native redraw per update;
- dirty-region CPU raster and 256-byte-row-aligned partial GPU upload;
- dedicated renderer process and main-thread winit/wgpu surface;
- `ControlFlow::Wait` while idle.

Run the repeatable control-plane benchmark:

```bash
bun run bench:overlay
```

On the development macOS arm64 machine, three isolated runs of a 26-primitive state-bound panel over 20,000 changing snapshots measured 71–72 µs p50, 90–92 µs p95, and 213–220 µs p99 for reconciliation plus mutation publication. This is a local baseline, not a universal compositor-latency claim. Target-machine checks should also record native presentation latency, idle CPU/RSS, and input p99 with overlay off/on.

## Current boundaries

- Overlay-safe defaults are non-focusable and click-through; both policies are configurable. Interactive controls/widgets are not yet included.
- Windows per-pixel transparency requires visual verification when wgpu selects `alphaMode: "Opaque"`.
- System and monospace font families are supported; arbitrary font-file loading is not yet public.
- The primary monitor is used; multi-monitor routing is not yet public.
- Images, arbitrary vector paths, clipping, and animation are not yet public APIs.
- Linux needs an active graphical session and compositor-specific smoke testing.

## Smoke test

```bash
bun run build:native
target/release/spellwire-overlay --smoke
bun run test:overlay-live
```

The executable smoke prints one `ready` JSON object with physical surface dimensions, monitor scale factor, alpha mode, and resolved window policy. The live smoke additionally starts a real host, verifies configured/default window options, writes named state, bulk-snapshots it, updates two retained text nodes, and verifies clean shutdown.
