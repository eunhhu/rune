# Platform Status

[한국어](platforms.ko.md)

The three native backends share one owned-host lifecycle and keep their event-loop details platform-specific.

## Matrix

| Feature | Windows | macOS | Linux |
| --- | --- | --- | --- |
| Observation | `WH_KEYBOARD_LL` / `WH_MOUSE_LL` | listen-only `CGEventTap` | evdev + hotplug rescan |
| Injection | tagged batched `SendInput` | private-source tagged `CGEventPost` | dedicated uinput device |
| Physical/synthetic classification | injection tag | injection tag | virtual-device identity |
| Keyboard/mouse/buttons/wheel | Yes | Yes | Yes |
| Transparent retained overlay | winit/wgpu | accessory-policy winit/wgpu | winit/wgpu; compositor-dependent |
| Local live verification | Pending | Passed on arm64 | Pending |

Windows and Linux currently pass unit/mapping tests and x86_64 cross-target Clippy from macOS. Run the target-machine checks below before treating a release artifact as verified.

For step-by-step setup, expected JSON, benchmark interpretation, OS-specific failure checks, and a report template, use [Platform Verification Guide](platform-verification.md).

## Permissions

### Windows

Normal desktop input usually needs no elevation. `SendInput` is subject to User Interface Privilege Isolation, so a non-elevated Spellwire process cannot inject into a higher-integrity target. Secure desktop/UAC prompts are out of scope.

### macOS

Observation needs Input Monitoring. Injection needs Accessibility. Normal `spellwire run` and `spellwire watch` startup checks and requests both automatically. The following advanced diagnostic commands print the individual status bits:

```bash
bun packages/spellwire/src/cli.ts permissions
bun packages/spellwire/src/cli.ts permissions --request
```

Restart the terminal/application after changing privacy settings if macOS does not update an existing process.

### Linux

The process needs read access to intended `/dev/input/event*` devices and write access to `/dev/uinput`. Reading evdev exposes global keyboard input and is security-sensitive. Review and install `packaging/linux/99-spellwire-input.rules` only for machines where that access is intended, then reload udev rules or log in again.

## Target-machine verification

From a checkout with Bun 1.4+ and Rust installed:

```bash
bun install --frozen-lockfile
bun run build:native
bun packages/spellwire/src/cli.ts permissions
bun run test:platform-loopback
bun run bench:platform -- 10000
target/release/spellwire-overlay --smoke
```

The loopback sends a tagged synthetic F20 through the real platform injector, observes it through the global backend, and verifies that a synthetic-source VM handler updates named native state.

This abbreviated command list is the release gate, not the full setup guide. Windows uses `target/release/spellwire-overlay.exe`; Linux needs evdev/uinput access and a graphical session for overlay smoke. Follow [Platform Verification](platform-verification.md) before reporting a failure.

## Key translation

Public `Key` values use USB HID keyboard-page usages. Each backend has an explicit supported map and returns `unsupported USB HID key usage` instead of silently emitting a different key. Linux covers the full currently exported set. macOS and Windows omit usages their keyboard APIs cannot represent reliably; layout/media behavior should be checked on the target keyboard. Unknown vendor-page usages are intentionally not guessed.

## Overlay limitations

The renderer requests a transparent, topmost, click-through window and keeps rendering isolated from the input worker. Windows and macOS provide the needed window semantics. Linux support depends on the active display server/compositor; Wayland does not expose one universal always-on-top layer-shell contract through winit, so verify the intended GNOME/KDE/wlroots environment.
