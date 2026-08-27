# Implementation Status

[한국어](status.ko.md)

Spellwire is an early alpha. This page tracks implemented surfaces separately from platform acceptance results.

## Implemented

- TypeScript AST compiler, source diagnostics, portable hotkey parser, paired remaps, module-scope named integer/boolean state, direct state-immediate updates, native state gates, conditions, loops, inlined helpers, held checks, and key/mouse intrinsics;
- versioned `SPWR` v5 encoder, v3/v4/v5 decoder, fixed-payload effect opcode, and structural/runtime validation;
- bounded native VM stack, locals, output batch, instruction budget, and fixed trigger table;
- fixed-capacity continuation scheduler: `sleep.us/ms/seconds/minutes/hours()` lowers to one wide/scaled delay opcode and yields until an absolute monotonic deadline without blocking the observer worker;
- compatibility engine C ABI plus ABI v5 owned-host lifecycle, reload, scalar/bulk state, permissions, error, dispatch, shared input-ring, and native event-ring APIs;
- Bun FFI `NativeHost` with start/stop, `.ts` in-memory compilation, `.bin` manifest loading, serialized watch reload, and state preservation by source name and kind;
- callback-free `DynamicInputLane` connection from the native observer through a shared six-word SPSC record ring;
- callback-free 20-word `RuntimeEventLane` for changed state/effects, cached state recovery, change-driven overlay refresh, and authenticated local Electron/sidecar RPC;
- Windows low-level keyboard/mouse hooks, lock-free original-input suppression, and tagged batched `SendInput` injection;
- macOS active `CGEventTap`, lock-free original-input suppression, Input Monitoring/Accessibility checks, Caps Lock pulse normalization, private event source, tagged `CGEventPost`, and tap recovery;
- Linux evdev discovery/hotplug observation and a dedicated uinput keyboard/mouse device; selective original-input relay remains pending;
- explicit physical/synthetic recursion classification and supported USB HID translation tests;
- state-driven Figma-style row/column/stack layout with fill/stroke/radius/shadow/opacity/font styling, keyed diff, and unified lifecycle API;
- configurable native overlay window policy (transparent/topmost/focusable/click-through/decorated/resizable/visible) plus text/rect/ellipse/line nodes, coalesced batch protocol, dirty raster, and partial GPU uploads;
- VM, overlay reconciliation, and native OS-submission percentile benchmark commands;
- cross-platform CI, Rust 1.81 check, npm dry-runs, and release artifact matrix with checksums and optional Windows/macOS signing plus macOS notarization.

## Validation state

| Surface | macOS arm64 | Windows x64 | Linux x64 |
| --- | --- | --- | --- |
| Rust/TypeScript unit tests | Passed locally | Passed on target | CI source coverage |
| Target compile + Clippy | Passed | Passed on target | Cross-target passed |
| Global observe → VM → inject → observe loopback | Passed | Passed in interactive session | Target-machine run pending |
| Original input suppression + state-gated pass-through | Passed with CoreGraphics head/tail probe | Physical-input smoke pending | Not implemented |
| Bun shared dynamic lane | Passed | Passed in interactive session | Target-machine run pending |
| Native overlay + configurable window-policy smoke | Passed | Window policy/live update passed; visual transparency pending | Display/compositor run pending |

The macOS and Windows loopbacks use a physical-source F19 test dispatch, native F20 injection, tagged synthetic re-observation, and a second VM handler/state update. They test the OS backend without measuring a physical keyboard switch or target-application receipt. Windows checks run in an interactive desktop session because Session 0 blocks `SendInput`.

## Capability bits

`spellwire_capabilities()` returns `0xf7` on Windows/macOS:

```text
HostCallbackInjection | NativeObservation | NativeInjection |
HostLifecycle | NonBlockingDelay | NativeInputSuppression | NativeEventLane
```

Linux returns `0xb7` without `NativeInputSuppression`.

`NativeOverlay` remains a reserved library bit because the renderer is a separately resolved executable. `NativeOverlayRenderer.start()` verifies that companion executable directly.

## External release gates

These need credentials, hardware, or target machines and cannot be completed by source changes alone:

- actual npm publication and registry propagation;
- Authenticode/Developer ID signing and Apple notarization with repository secrets;
- Windows consuming hotkey/remap smoke run;
- Windows per-pixel overlay transparency visual check;
- Linux permission/device/display target run;
- Linux exclusive evdev pass-through relay before suppression can be claimed;
- physical switch → HID → OS → target-application latency measurements;
- Linux overlay behavior on each intended X11/Wayland compositor.

Run and report the target-machine gates with [Platform Verification Guide](platform-verification.md). Use [Live Native Host Guide](live-host.md) when integrating the implemented host APIs into an application.

The [Hotkeys and automation](automation.md#autohotkey-migration-status) matrix tracks remaining AutoHotkey compatibility gaps.
