# Native Overlay

[한국어](overlay.ko.md)

Spellwire ships a retained scene model and a separate native renderer process. Input dispatch never waits for the renderer or calls JavaScript.

## Use

```ts
import { NativeOverlayRenderer, OverlayScene } from "spellwire";

const scene = new OverlayScene();
const panel = scene.create({
  kind: "rect",
  x: 20,
  y: 20,
  width: 260,
  height: 72,
  radius: 14,
  color: "#121216cc",
});
const label = scene.create({
  kind: "text",
  x: 42,
  y: 42,
  text: "42 µs",
  size: 20,
  color: "#ffffffff",
});

const renderer = await NativeOverlayRenderer.start();
await renderer.apply(scene);

scene.update(label, { kind: "text", x: 42, y: 42, text: "38 µs", size: 20 });
await renderer.apply(scene);

scene.remove(panel);
await renderer.apply(scene);
await renderer.close();
```

Nodes support optional `#RRGGBB` or `#RRGGBBAA` colors:

```ts
type OverlayNode =
  | { kind: "text"; x: number; y: number; text: string; size: number; color?: string }
  | { kind: "rect"; x: number; y: number; width: number; height: number; radius: number; color?: string }
  | { kind: "line"; x1: number; y1: number; x2: number; y2: number; width: number; color?: string };
```

`create`, `update`, and `remove` append monotonically revised mutations. `apply()` drains only those mutations; a static scene causes no JavaScript per-frame callback.

## Process protocol and isolation

`NativeOverlayRenderer` resolves `spellwire-overlay` from the npm platform directory, `SPELLWIRE_OVERLAY_EXECUTABLE`, or a workspace release/debug build. It starts the renderer with piped stdin and waits for a JSON `ready` message. Newline-delimited commands are `upsert`, `remove`, `clear`, `show`, `hide`, and `exit`.

The native process:

- owns the main-thread winit event loop and wgpu surface;
- retains nodes in a native ordered map;
- rasterizes text/rect/line nodes only after mutations;
- uploads a premultiplied RGBA frame and presents it on a transparent surface;
- requests topmost and click-through behavior;
- runs independently of the native input observer/runtime worker.

Renderer failure therefore does not stop input execution. The renderer currently covers the primary monitor; multi-monitor scene routing is not yet exposed.

## Smoke test

```bash
bun run build:native
target/release/spellwire-overlay --smoke
```

Success prints one `ready` JSON object containing surface dimensions and alpha mode, then exits. On Linux this command requires a working graphical session and must be repeated for every supported compositor.

## Performance scope

The implementation removes overlay work from the realtime worker, but no universal compositor-latency claim is made. Release profiling should report idle CPU/RSS, mutation publication time, render percentiles at representative primitive counts, and input p99 with the overlay disabled/enabled.
