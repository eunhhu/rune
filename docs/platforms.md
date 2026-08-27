# Platform Status

[한국어](platforms.ko.md)

The three native backends share one owned-host lifecycle and keep their event-loop details platform-specific.

## Matrix

| Feature | Windows | macOS | Linux |
| --- | --- | --- | --- |
| Observation | `WH_KEYBOARD_LL` / `WH_MOUSE_LL` | active `CGEventTap` | evdev + hotplug rescan |
| Injection | tagged batched `SendInput` | private-source tagged `CGEventPost` | dedicated uinput device |
| Original-input suppression | Hook returns nonzero | Event tap returns null | Not implemented |
| Physical/synthetic classification | injection tag | injection tag | virtual-device identity |
| Keyboard/mouse/buttons/wheel | Yes | Yes | Yes |
| Retained overlay | winit/wgpu; visual transparency check pending | accessory-policy winit/wgpu | winit/wgpu; compositor-dependent |
| Live verification | Interactive loopback, reload, dynamic lane, window policy, and live overlay update passed | Loopback + suppression passed on arm64 | Pending |

Windows x64 passes target checks in an interactive desktop session; physical suppression and visual transparency remain manual checks. Linux passes unit/mapping and cross-target source gates but still needs device/display verification.

Windows/macOS suppression uses a lock-free trigger table and paired down/repeat/up tracking. Linux cannot safely suppress selected evdev events by dropping reads: it must exclusively grab each physical device and relay every non-consumed capability through a virtual device. That full relay is not implemented, so Linux does not advertise `NativeInputSuppression` and `consume` currently leaves original input visible there.

For step-by-step setup, expected JSON, benchmark interpretation, OS-specific failure checks, and a report template, use [Platform Verification Guide](platform-verification.md).

## Permissions

### Windows

Normal desktop input usually needs no elevation. `SendInput` is subject to User Interface Privilege Isolation, so a non-elevated Spellwire process cannot inject into a higher-integrity target. Secure desktop/UAC prompts are out of scope.

### macOS

Observation needs Input Monitoring. Injection needs Accessibility. Normal `spellwire run` and `spellwire watch` startup checks and requests both automatically. The following advanced diagnostic commands print the individual status bits:

```bash
bun run inspect:runtime
bun run inspect:runtime -- --request-permissions
```

Restart the terminal/application after changing privacy settings if macOS does not update an existing process.

### Linux

The process needs read access to intended `/dev/input/event*` devices and write access to `/dev/uinput`. Reading evdev exposes global keyboard input and is security-sensitive. Review and install `packaging/linux/99-spellwire-input.rules` only for machines where that access is intended, then reload udev rules or log in again.

## Target-machine verification

From a checkout with Bun 1.4+ and Rust installed:

```bash
bun install --frozen-lockfile
bun run build:native
bun run inspect:runtime
bun run test:platform-loopback
bun run test:consume-macos # macOS only
bun run bench:platform -- 10000
target/release/spellwire-overlay --smoke
bun run test:overlay-live
```

The loopback sends a tagged synthetic F20 through the real platform injector, observes it through the global backend, and verifies that a synthetic-source VM handler updates named native state.

The macOS consume smoke first validates its tail event tap with two unblocked transitions. It then verifies state-gated pass-through and finally requires one VM hit with zero forwarded transitions.

This abbreviated command list is the release gate, not the full setup guide. Windows uses `target/release/spellwire-overlay.exe`; Linux needs evdev/uinput access and a graphical session for overlay smoke. Follow [Platform Verification](platform-verification.md) before reporting a failure.

## Key translation

Public `Key` values use USB HID keyboard-page usages. Each backend has an explicit supported map and returns `unsupported USB HID key usage` instead of silently emitting a different key. Linux covers the full currently exported set. macOS and Windows omit usages their keyboard APIs cannot represent reliably; layout/media behavior should be checked on the target keyboard. Unknown vendor-page usages are intentionally not guessed.

## Overlay window semantics

The native renderer accepts `transparent`, `alwaysOnTop`, `focusable`, `clickThrough`, `decorations`, `resizable`, and initial `visible` options; overlay-safe defaults request transparency, topmost, non-focusable, click-through, borderless, fixed-size, and visible behavior. Rendering remains isolated from the input worker.

macOS uses the prohibited activation policy when `focusable` is false and the accessory policy when true. Windows disables the native window when `focusable` is false. Windows window-policy and live-update smoke tests pass, but a ready message with `alphaMode: "Opaque"` still requires a visual per-pixel transparency check. Linux behavior depends on the active display server/compositor; Wayland does not expose one universal always-on-top layer-shell or focus contract through winit. Verify the intended GNOME/KDE/wlroots environment. Primary-monitor selection and full-monitor startup geometry are currently fixed.
