# Implementation Status

[한국어](status.ko.md)

Spellwire is an early alpha with the README implementation plan represented in source. Platform validation remains deliberately separated from implementation.

## Implemented

- TypeScript AST compiler, source diagnostics, module-scope named integer/boolean state, conditions, loops, updates, inlined helpers, held checks, and key/mouse intrinsics;
- versioned `SPWR` encoder/decoder and structural/runtime validation;
- bounded native VM stack, locals, output batch, instruction budget, and fixed trigger table;
- fixed-capacity continuation scheduler: `sleepUs()` yields until an absolute monotonic deadline without blocking the observer worker;
- compatibility engine C ABI plus ABI v4 owned-host lifecycle, reload, scalar/bulk state, permissions, error, dispatch, and shared input-ring APIs;
- Bun FFI `NativeHost` with start/stop, `.ts` in-memory compilation, `.bin` manifest loading, serialized watch reload, and state preservation by source name and kind;
- callback-free `DynamicInputLane` connection from the native observer through a shared six-word SPSC record ring;
- Windows low-level keyboard/mouse hooks and tagged batched `SendInput` injection;
- macOS listen-only `CGEventTap`, Input Monitoring/Accessibility checks, private event source, tagged `CGEventPost`, and tap recovery;
- Linux evdev discovery/hotplug observation and a dedicated uinput keyboard/mouse device;
- explicit physical/synthetic recursion classification and supported USB HID translation tests;
- state-driven Figma-style row/column/stack layout with fill/stroke/radius/shadow/opacity/font styling, keyed diff, and unified lifecycle API;
- transparent, topmost, click-through retained overlay process with text/rect/ellipse/line nodes, coalesced batch protocol, dirty raster, and partial GPU uploads;
- VM, overlay reconciliation, and native OS-submission percentile benchmark commands;
- cross-platform CI, Rust 1.81 check, npm dry-runs, and release artifact matrix with checksums and optional Windows/macOS signing plus macOS notarization.

## Validation state

| Surface | macOS arm64 | Windows x64 | Linux x64 |
| --- | --- | --- | --- |
| Rust/TypeScript unit tests | Passed locally | CI source coverage | CI source coverage |
| Target compile + Clippy | Passed | Cross-target passed | Cross-target passed |
| Global observe → VM → inject → observe loopback | Passed | Target-machine run pending | Target-machine run pending |
| Bun shared dynamic lane | Passed | Target-machine run pending | Target-machine run pending |
| Native transparent overlay smoke | Passed | Target-machine run pending | Display/compositor run pending |

The macOS loopback uses a physical-source F19 dispatch, native F20 injection, tagged synthetic re-observation, and a second VM handler/state update. This is an OS loopback test, not a physical keyboard switch-to-application latency claim.

## Capability bits

`spellwire_capabilities()` returns:

```text
HostCallbackInjection | NativeObservation | NativeInjection |
HostLifecycle | NonBlockingDelay
```

`NativeOverlay` remains a reserved library bit because the renderer is a separately resolved executable. `NativeOverlayRenderer.start()` verifies that companion executable directly.

## External release gates

These need credentials, hardware, or target machines and cannot be completed by source changes alone:

- actual npm publication and registry propagation;
- Authenticode/Developer ID signing and Apple notarization with repository secrets;
- Windows and Linux permission/setup smoke runs;
- physical switch → HID → OS → target-application latency measurements;
- Linux overlay behavior on each intended X11/Wayland compositor.

Run and report the target-machine gates with [Platform Verification Guide](platform-verification.md). Use [Live Native Host Guide](live-host.md) when integrating the implemented host APIs into an application.
