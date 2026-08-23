# Native overlay design

The overlay must not share the input thread and must not depend on per-frame JavaScript callbacks.

## Retained scene

TypeScript will create stable node IDs and update properties:

```ts
const hud = overlay.create({ clickThrough: true });
const latency = hud.text({ x: 24, y: 24, text: "-- us" });
latency.set({ text: "42 us" });
```

Those operations produce scene mutations on the control plane. A native renderer consumes immutable snapshots. Rendering continues when Bun is busy, and a frame never calls into JavaScript.

Initial primitives should stay deliberately small:

- rectangle and rounded rectangle
- line
- text
- image
- clip and transform

A browser layout engine, DOM, webview, accessibility tree, and general widget toolkit are explicitly out of scope.

## Snapshot handoff

The intended handoff is double- or triple-buffered:

```text
Bun update → build inactive scene snapshot → atomic publish
                                             │
render thread ← acquire current snapshot ─────┘
```

Input execution has no reference to the scene lock or renderer. Overlay failure must never stop a macro.

## Platform direction

- **Windows:** transparent click-through topmost HWND, DirectComposition/Direct2D/DirectWrite.
- **macOS:** transparent non-activating NSPanel, Core Animation and Core Text or Metal for larger scenes.
- **Linux X11:** ARGB override-redirect window with an EGL-backed renderer.
- **Linux Wayland:** capability-based support. Layer-shell works on wlroots-family compositors; no universal protocol guarantees a global always-on-top overlay on every compositor.

## Why the MVP does not use winit + webview

A cross-platform window crate can be a useful prototype, but it does not solve compositor policy and often pulls a large dependency surface into the same binary. Rune's first milestone keeps the realtime runtime dependency-free and marks overlay support unavailable until a renderer meets the isolation and capability requirements above.

## Performance contract

Overlay performance should be reported separately from input latency. Candidate metrics:

- idle CPU and resident memory with one static text node
- scene-update publication time
- render-thread frame time at p50/p95/p99
- GPU/CPU cost for 100, 1,000, and 10,000 primitives
- input dispatch latency with overlay disabled vs enabled

The final comparison is mandatory: enabling the overlay must not measurably change the input thread's p99 latency.
