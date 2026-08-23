# Platform backends

All public key codes are USB HID keyboard usages. Platform translation is isolated in the backend and is done without a map allocation in the input path.

## Windows

The input thread owns a message-only window and registers keyboard and mouse Raw Input devices with `RIDEV_INPUTSINK`. `WM_INPUT` packets are translated to Rune events and dispatched directly from that thread.

Output actions are translated to scan-code `INPUT` records and submitted in one `SendInput` call per zero-delay batch. Navigation, right-side modifiers, numpad enter, and numpad divide use extended-key flags.

Current constraints:

- one active Windows Rune runtime per process
- Pause/Break and a few uncommon HID usages are not mapped yet
- physical-device identity is not surfaced in the TypeScript API yet

## macOS

The backend installs a listen-only HID event tap on a dedicated run loop. Keyboard, modifier `flagsChanged`, and mouse-button events are translated to USB HID usages. Autorepeat key-down events are ignored so a physical hold does not retrigger a down rule unless that behavior is added explicitly later.

Output uses tagged CoreGraphics events. The event tap ignores Rune's tag to prevent recursion. Input Monitoring and Accessibility permissions are required.

Current constraints:

- CoreGraphics posts the members of a zero-delay batch individually
- modifier state is reconstructed from `flagsChanged`; starting Rune while a modifier is already held can make the first transition ambiguous
- IOHIDManager is a possible later observer for device-level modes, but the MVP uses an event tap for a smaller compatibility surface

## Linux

Rune opens readable `/dev/input/event*` descriptors before creating its own virtual device, then observes `EV_KEY` records with `poll`. Output is a uinput keyboard/mouse device. A zero-delay batch is written as native events followed by one `SYN_REPORT`.

This path is below X11 and Wayland, so the input engine itself does not depend on a compositor protocol. It does require permission to read input devices and create a uinput device. Those permissions are security-sensitive: access to all keyboard events is equivalent to keylogging capability.

For a single-user desktop, the included example rule uses seat-based `uaccess`:

```bash
sudo cp packaging/linux/99-rune-input.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
sudo modprobe uinput
```

For multi-user or headless machines, create a dedicated group and use narrower administrator-managed rules instead of granting broad device access.

Current constraints:

- devices connected after Rune starts are not hot-added yet
- the MVP opens every readable event device and filters to keyboard/button events in userspace
- another pre-existing virtual device can still be observed; per-device include/exclude filters are planned

## Capability API

The native library returns bit flags for features compiled into the current target. The initial values are:

```ts
Capability.ObserveKeyboard
Capability.ObserveMouseButton
Capability.InjectKeyboard
Capability.InjectMouse
Capability.OverlayScene
```

`OverlayScene` remains unset until a native renderer is actually present.
