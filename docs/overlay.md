# Native overlay model

The overlay is intentionally outside the input thread. TypeScript mutates a retained scene; a renderer consumes snapshots or mutation batches on its own thread. Rendering never calls JavaScript once per frame.

Initial primitives are deliberately small:

- text
- rectangle / rounded rectangle
- line
- image (planned)
- clip and transform (planned)

A DOM, browser layout engine, webview, accessibility tree, and general widget toolkit are out of scope.

```text
Bun scene update → inactive snapshot → atomic publish
                                           │
render thread ← acquire current snapshot ──┘
```

Target platform direction:

- Windows: transparent click-through topmost HWND with DirectComposition/Direct2D/DirectWrite
- macOS: non-activating transparent NSPanel with Core Animation/Core Text or Metal
- Linux X11: ARGB overlay window with an EGL-backed renderer
- Linux Wayland: capability-based layer-shell support where the compositor exposes it

Overlay benchmarks must be reported separately and must include input p99 with the overlay disabled and enabled. Enabling a static HUD must not measurably perturb the input thread.
