# Implementation Status

Rune is an early compiler/native-VM MVP. This page separates verified implementation from design intent.

## Implemented and tested

- complete committed source tree; no post-merge source-generation workflow;
- TypeScript workspace packages for SDK and compiler;
- TypeScript AST compiler with source diagnostics;
- module-scope persistent integer/boolean state;
- conditions, loops, assignments, updates, and inlined void helper functions;
- key/mouse output and held-input intrinsics;
- physical, synthetic, and any-source handler filters;
- versioned binary encoder/decoder;
- native program validation;
- fixed trigger lookup table;
- bounded native VM stack, locals, output batch, and instruction budget;
- absolute-deadline synchronous delays;
- C ABI for engine lifecycle, explicit dispatch, state slots, and output callback batches;
- deterministic `rune-sim` inspector/simulator;
- JavaScript fallback action sink and SPSC dynamic lane;
- retained overlay scene/mutation model;
- Rust and TypeScript unit tests;
- CI across Linux, macOS, and Windows plus Rust 1.81 and Quick Start smoke tests.

## Not implemented yet

- Windows system-wide observation and `SendInput` host;
- macOS event-tap observation and CoreGraphics injection host;
- Linux evdev observation and uinput injection host;
- a Bun FFI control-plane host with start/stop, hot reload, and named state bindings;
- non-blocking continuation/deadline scheduling for delayed handlers;
- native transparent overlay windows/renderers;
- complete international/vendor-specific key translation;
- prebuilt native artifacts, signing/notarization, and package publication;
- platform submission-latency benchmarks;
- physical end-to-end latency measurements.

## Current capability flag

`rune_capabilities()` currently reports only:

```text
HostCallbackInjection
```

It does not report `NativeObservation`, `NativeInjection`, or `NativeOverlay`.

## Next practical milestone

A useful live-input milestone should add one host interface shared across platforms while preserving platform-specific implementations:

```text
start program
  → platform observer owns event thread
  → native Runtime dispatch
  → platform injector submits batches
  → bounded control channel for stop/state/profile updates
```

The host should expose capability and permission errors rather than pretending all operating systems have identical behavior.

After one backend works end-to-end, the same benchmark harness should report core dispatch and host submission separately before performance claims are published.

## Merge-readiness definition for this PR

This PR is merge-ready only when:

- the actual source is visible in the branch;
- old bootstrap/shipping workflows are removed;
- permanent CI is green;
- the documented Quick Start succeeds from a fresh checkout;
- docs do not claim direct OS input or overlay rendering that the code does not contain.
