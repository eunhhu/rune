# Overlay

The current SDK includes a retained `OverlayScene` data model. It does not create a native window or render pixels yet.

## Current API

```ts
import { OverlayScene } from "@rune/sdk";

const scene = new OverlayScene();

const latencyId = scene.create({
  kind: "text",
  x: 24,
  y: 24,
  text: "-- µs",
  size: 16,
});

scene.update(latencyId, {
  kind: "text",
  x: 24,
  y: 24,
  text: "42 µs",
  size: 16,
});

const mutations = scene.drainMutations();
const snapshot = scene.snapshot();
scene.remove(latencyId);
```

Supported node shapes:

```ts
type OverlayNode =
  | { kind: "text"; x: number; y: number; text: string; size: number }
  | { kind: "rect"; x: number; y: number; width: number; height: number; radius: number }
  | { kind: "line"; x1: number; y1: number; x2: number; y2: number; width: number };
```

Every create/update/remove operation records a monotonically increasing revision and a mutation. A host renderer can drain mutations or request a complete snapshot.

The current implementation uses normal JavaScript `Map`, arrays, and `structuredClone`; it belongs to the Bun control plane, not to native input dispatch.

## Renderer contract

A future renderer should consume scene mutations/snapshots on a dedicated native thread:

```text
Bun scene update
    → build/publish retained scene state
        → native render thread
```

The input VM must not take an overlay lock, wait for a frame, or invoke JavaScript while dispatching an event. Overlay failure must not stop input execution.

## Planned platform direction

- Windows: transparent click-through topmost window with DirectComposition/Direct2D/DirectWrite.
- macOS: non-activating transparent panel with Core Animation/Core Text or Metal.
- Linux X11: ARGB window with an EGL-backed renderer.
- Linux Wayland: capability-based layer-shell support where the compositor provides it.

These are design directions, not current implementation claims.

## Performance gate

Before native overlay support is advertised, measurements should include:

- idle CPU and resident memory for a static text node;
- scene publication time;
- render p50/p95/p99;
- cost at 100, 1,000, and 10,000 primitives;
- native input dispatch p99 with overlay disabled and enabled.

The last comparison is mandatory: enabling a static overlay must not measurably worsen input dispatch jitter.
