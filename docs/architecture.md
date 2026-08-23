# Architecture

Rune is split into a control plane and a realtime plane. The boundary is intentional: TypeScript is an excellent language for describing automation, but a JavaScript event loop is not an appropriate latency boundary for every physical key event.

## Control plane

Bun executes the user's TypeScript file once. `macro(name, builder)` records rules and actions as plain immutable data. `@rune/sdk` validates that data and serializes it into the versioned `RUNE` wire format.

The control plane may allocate. It may read configuration, hot-reload a script, print diagnostics, or build overlay state. None of those operations occur in response to an input event.

The current bridge is a small C ABI loaded through `bun:ffi`. The bridge is replaceable: a future Node-API wrapper or another language binding can load the same binary IR without changing the core runtime.

## Realtime plane

`rune-native` decodes the IR before starting an input backend. `rune-core::ProgramSet` compiles triggers into fixed slots:

```text
source × device × edge × code → contiguous program-id bucket
```

Dispatching an event performs direct indexing and iterates a precomputed contiguous ID slice. It does not hash strings, parse script objects, allocate, or call JavaScript.

Each backend owns an `ExecutionScratch` with a fixed 64-event output array. Actions with no delay are accumulated and submitted to the platform injector as one batch when the platform API permits it.

## Delay semantics

A macro begins with a monotonic deadline equal to its start time. Every `delay.us(n)` advances that deadline by `n` microseconds. Rune waits until the absolute deadline rather than sleeping for `n` microseconds relative to the end of the previous operation.

That prevents scheduler and injection overhead from accumulating as drift across a long sequence:

```text
target += 100 us
target += 100 us
target += 100 us
```

For waits longer than `spinThresholdUs`, the input thread sleeps until the final tail. The remaining tail is actively spun. A larger spin threshold can reduce overshoot but consumes a CPU core for longer, so the native API rejects values above 5 ms.

## Batching

A sequence such as:

```ts
m.key.down(Key.E),
m.mouse.down(MouseButton.Left),
m.mouse.up(MouseButton.Left),
m.key.up(Key.E),
```

becomes one fixed native batch. On Windows that maps to one `SendInput` call. On Linux it maps to a contiguous set of `input_event` records followed by one `SYN_REPORT`. CoreGraphics exposes event posting individually, so the macOS backend avoids JS and allocation but cannot offer the same single-call batch primitive.

A delay flushes the current batch before advancing the deadline.

## Source handling

The IR distinguishes `physical`, `synthetic`, and `any` trigger sources. The initial direct backends expose physical observations and prevent their own generated events from recursively triggering macros where the OS API provides source identity.

Synthetic-trigger programs are reserved in the format so future hook/virtual-device modes do not require a wire-format break.

## Failure model

Platform errors are written into a native error slot and returned to the control plane through numeric error codes. The hot path does not log or format successful events. If a native injection fails during dispatch, that backend stops rather than silently continuing with a partially executed program.

## Performance measurement

Rune separates three measurements:

1. **Core dispatch:** trigger lookup + VM + null injector.
2. **Runtime submission:** OS-visible event → native injection API submission.
3. **Physical end-to-end:** switch → HID report → OS → Rune → target application.

Only the first is portable and deterministic enough for an automated repository benchmark. Platform submission latency needs a loopback device or platform-specific instrumentation. Physical end-to-end measurements need external hardware. Numbers from these scopes must never be mixed in documentation or marketing.
