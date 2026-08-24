# Platform Status

Rune's compiler, core VM, simulator, and C ABI build on Windows, macOS, and Linux. Direct system-wide input backends are not present in the current branch.

## Current matrix

| Feature | Windows | macOS | Linux |
| --- | --- | --- | --- |
| TypeScript compiler | Yes | Yes | Yes |
| Native VM/core | Yes | Yes | Yes |
| `rune-native` C ABI | Yes | Yes | Yes |
| `rune-sim` | Yes | Yes | Yes |
| Global input observation | No | No | No |
| Native input injection | No | No | No |
| Native overlay renderer | No | No | No |

`rune_capabilities()` currently returns only the `HostCallbackInjection` bit on every target.

## Permissions for the current Quick Start

The compiler and simulator do not observe or inject real input, so they require no special input permissions:

- no Windows elevation;
- no macOS Input Monitoring or Accessibility permission;
- no Linux evdev/uinput access.

The included Linux udev rule is a future-backend packaging artifact and is not needed to compile or simulate a program.

## Planned direct backends

The intended low-overhead direction remains:

| Platform | Observation | Injection |
| --- | --- | --- |
| Windows | Raw Input or a carefully measured low-level hook | `SendInput` |
| macOS | `CGEventTap` initially; IOHID mode only if justified | tagged `CGEventPost` |
| Linux | evdev | uinput |

These APIs are design targets, not implemented claims.

## Backend requirements

A direct backend should not be marked complete until it has:

- physical/synthetic recursion handling;
- complete key translation tests for its supported map;
- clear permission/setup diagnostics;
- zero allocation in the input-to-dispatch path after startup;
- output batching where the OS supports it;
- clean start/stop and ownership semantics;
- p50/p95/p99/p99.9 submission-latency measurements;
- compile and smoke coverage on the target OS.

## Linux security note

Future evdev access exposes global keyboard input and is equivalent to keylogging capability. A shipped udev rule must be narrowly documented and appropriate for the deployment model. Do not install `packaging/linux/99-rune-input.rules` merely to use the current simulator.

## Wayland and overlay

An evdev/uinput input backend can operate below X11/Wayland, but an always-on-top transparent overlay remains compositor-dependent. Wayland layer-shell is not universal, especially across GNOME/KDE/wlroots environments. Overlay support must therefore be reported through capabilities rather than a single unconditional “Linux supported” flag.
