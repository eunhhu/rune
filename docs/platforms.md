# Platform backends

Spellwire uses USB HID usages as its portable key identity. Every backend translates once at its boundary rather than leaking platform virtual-key values into scripts.

Intended native paths:

| Platform | Observation | Injection | Notes |
| --- | --- | --- | --- |
| Windows | Raw Input, with a low-level hook mode when suppression is needed | `SendInput` batches | injected-event tagging and recursion filtering required |
| macOS | `CGEventTap`; optional IOHID observation profile | `CGEventPost` / tap posting | Input Monitoring and Accessibility consent required |
| Linux | `evdev` | `uinput` virtual device | device permissions/udev setup required |

Wayland compositor protocols do not provide one universal global overlay contract. Input through evdev/uinput and desktop-portal/libei modes therefore remain separate capabilities.

The current native crate exposes a stable host ABI and capability bits. A backend is advertised only after it passes correctness, permission, recursion, and latency tests on that OS; unsupported capability bits remain clear instead of silently falling back to a slower path.
