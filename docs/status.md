# Implementation status

## Implemented on `main`

- normal TypeScript syntax for realtime handlers
- module-scope persistent integer/boolean state
- `if` / `else`, conditional expressions, `for`, `while`, `do`, `break`, and `continue`
- arithmetic, comparisons, boolean short-circuiting, and bit operations
- compile-time inlining of top-level helper functions with parameters
- versioned fixed-width bytecode encoder/decoder
- fixed trigger table with physical/synthetic/any filters
- allocation-free VM dispatch scratch, output batching, and instruction budgets
- absolute monotonic delay scheduling with a configurable spin tail
- C ABI for load, dispatch, state access, and output submission
- SharedArrayBuffer SPSC ring for the best-effort Bun event lane
- retained overlay scene model
- compiler, SDK, VM tests, and a percentile benchmark harness

## Deliberately not claimed yet

- arbitrary JavaScript semantics inside the realtime lane
- hard-realtime guarantees from a general-purpose desktop OS
- measured cross-platform microsecond end-to-end latency
- validated direct OS observation/injection backends on all three platforms
- a transparent native overlay renderer
- automatic source rewriting of dynamic Bun references to captured native state
- complete international/vendor-specific HID mappings
- release signing and prebuilt native binaries

Spellwire should not set a capability bit or publish a latency number before the corresponding backend is tested on actual hardware.
