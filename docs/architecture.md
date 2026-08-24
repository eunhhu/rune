# Architecture

Rune is currently composed of five explicit layers: TypeScript SDK, AOT compiler, versioned wire format, native VM, and host boundary.

## 1. TypeScript SDK

`@rune/sdk` supplies:

- USB HID-style `Key` identifiers and mouse buttons;
- top-level `rt.on*` handler markers;
- output and held-input intrinsics;
- JavaScript fallback registrations/action sinks;
- a SharedArrayBuffer SPSC dynamic input lane;
- native state wrappers;
- a retained overlay scene model.

The handler markers are ordinary TypeScript functions when executed by Bun, which makes source files testable. The compiler recognizes the same call shapes and does not depend on executing the module to discover handlers.

## 2. AOT compiler

`@rune/compiler` parses TypeScript with the TypeScript compiler API, collects representable module state, finds top-level realtime registrations, resolves constants, validates the supported subset, and lowers handlers to an integer bytecode instruction stream.

```text
module-scope let/const
handler callback
helper functions
control flow + expressions
          │
          ▼
 states + handler table + bytecode + resource limits
```

Compilation happens once. No AST, source string, or TypeScript runtime is needed during native dispatch.

## 3. Wire format

The encoder writes a versioned `RUNE` binary containing:

- header/version and resource limits;
- initial persistent-state values;
- triggers and bytecode entry points;
- fixed-width instructions.

`rune-core::Program::decode` validates structural bounds before `Runtime::new` validates entries, jumps, state/local slots, stack limits, and instruction budgets.

The companion JSON manifest is control-plane metadata; it is not read on the native input path.

## 4. Native VM

The runtime builds a fixed trigger table indexed by source, device, edge, and code. Dispatch performs direct indexing into contiguous handler-ID buckets.

```text
explicit InputEvent
      │
      ▼
update held-input bitmap
      │
      ▼
source × device × edge × code lookup
      │
      ▼
prevalidated integer VM
      │
      ▼
fixed 64-event output batches
```

`VmScratch` owns fixed stack, local, and output arrays. Successful dispatch does not parse strings, create a JavaScript callback, or allocate per instruction. Persistent state is owned by `Runtime` and survives between dispatches.

## 5. Host boundary

`rune-native` exposes a C ABI. A host passes a compiled binary, dispatches explicit events, reads/writes state slots, and receives output batches through a callback.

The current ABI is deliberately host-driven:

```text
host observes input → rune_engine_dispatch(...)
                    → VM output callback → host injects output
```

The repository does not yet contain a host that owns Windows Raw Input/hooks, a macOS event tap, or Linux evdev/uinput. Consequently, only host-callback injection is advertised by `rune_capabilities()`.

## Native simulator

`rune-sim` is a deterministic development host built directly on `rune-core`. It loads the same encoded binary, dispatches named input events, records native output batches, and prints persistent state after each event.

It is useful for API/compiler/VM feedback, but it intentionally does not pretend to measure platform input latency.

## Two TypeScript lanes

Rune's intended application split is:

```text
ordinary Bun/TypeScript
  configuration, files, networking, logging, hot reload, overlay state
                     │
                     │ compile/load/state control
                     ▼
native realtime VM
  bounded state machines and latency-sensitive input/output logic
```

`DynamicInputLane` is a best-effort bridge for events that genuinely need JavaScript. It uses an SPSC shared ring so a native producer does not have to invoke JS directly for every event.

## Delay and batching semantics

Output instructions accumulate until a delay, batch capacity, or handler halt. A delay flushes the batch and advances an absolute monotonic deadline, avoiding relative-sleep drift across a sequence.

The current delay waits synchronously. A continuation scheduler is a future runtime component and should use fixed-capacity queues so yielding does not add heap allocation to the hot path.

## Performance measurement scopes

Measurements must identify their boundary:

1. **Core dispatch:** trigger lookup + VM + null/recording injector.
2. **Host submission:** host-observed event → platform injection API submission.
3. **Physical end-to-end:** switch → HID → OS → host → injection → target application.

Only the first exists in this branch. The other two cannot be claimed until direct platform hosts and appropriate instrumentation exist.
