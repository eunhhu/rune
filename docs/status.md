# Implementation status

Rune is currently an early native runtime MVP.

## Implemented

- TypeScript macro builder and versioned binary encoder
- Bun FFI control plane
- allocation-free native trigger lookup and execution scratch
- physical, synthetic, and any-source trigger filters
- native key and mouse dispatch batches
- Windows hook and `SendInput` backend
- macOS event tap and `CGEventPost` backend
- Linux evdev and uinput backend
- renderer-independent overlay scene model
- Rust and TypeScript tests
- cross-platform build workflow

## Not yet claimed

- measured microsecond latency targets
- a realtime scheduling guarantee from general-purpose desktop operating systems
- a native transparent overlay window/render backend
- complete international and vendor-specific keyboard mappings
- prebuilt release artifacts and code signing

Benchmarks will report percentile distributions and jitter per backend before Rune publishes performance claims.
