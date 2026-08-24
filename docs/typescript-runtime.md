# TypeScript Runtime

Rune uses TypeScript in two different roles.

- **Control plane:** normal Bun/TypeScript with the full language and ecosystem.
- **Realtime plane:** a constrained TypeScript subset compiled ahead of time into native Rune bytecode.

The split exists so stateful macros can still use familiar TypeScript syntax without paying JavaScript callback, GC, event-loop, or native-boundary costs for every input event.

## Persistent state

Top-level mutable variables declared inside `rt.load()` become persistent native slots.

```ts
rt.load(() => {
  let phase = 0;

  on.keyDown(Key.Q, () => {
    phase = (phase + 1) % 3;
  });
});
```

The value survives after the handler returns and is reused on the next input event.

## Conditions

```ts
if (held(Key.LeftShift) && phase === 2) {
  key.tap(Key.E);
} else {
  key.tap(Key.R);
}
```

Boolean, arithmetic, comparison, and common bitwise expressions compile to native VM operations.

## Loops

```ts
for (let i = 0; i < 4; i++) {
  key.tap(Key.E);
  delay.us(40);
}
```

Runtime-dependent `while` and `do/while` loops are supported by the bytecode control-flow layer. Each handler is subject to an instruction budget so an accidental infinite loop cannot permanently occupy the realtime thread.

## Functions

```ts
function burst(keyCode: number, count: number) {
  for (let i = 0; i < count; i++) {
    key.tap(keyCode);
  }
}
```

Functions compile into native bytecode functions with fixed stack/call limits rather than JavaScript calls at event time.

## Realtime intrinsics

The realtime compiler recognizes Rune-provided intrinsics instead of arbitrary JavaScript APIs.

```ts
key.down(Key.E)
key.up(Key.E)
key.tap(Key.E)
mouse.down(MouseButton.Left)
mouse.up(MouseButton.Left)
mouse.click(MouseButton.Left)
mouse.move(5, -2)
mouse.wheel(0, 1)
delay.us(75)
held(Key.LeftShift)
```

## Deliberately unsupported on the realtime plane

The realtime compiler rejects or excludes features whose semantics require an unconstrained JavaScript runtime or unpredictable allocation:

- `async` / `await`
- `Promise`
- network or filesystem I/O
- arbitrary npm calls
- dynamic object/array construction on the hot path
- exceptions as a general control-flow mechanism
- generators
- dynamic property lookup
- runtime-created closures

Use ordinary Bun code outside `rt.load()` for those features.

## Why not just run Bun callbacks?

A conventional global-input library usually follows this path:

```text
native input thread
  → JS callback scheduling
  → JavaScript condition/state logic
  → FFI/N-API call
  → native injection
```

Rune's realtime path is instead:

```text
native input event
  → trigger lookup
  → native state/bytecode
  → native injection batch
```

That removes runtime-to-runtime round trips from latency-sensitive execution while preserving TypeScript as the authoring language.

## Resource limits

Rune intentionally uses bounded execution structures. Exact limits may evolve before a stable release, but the model is fixed-capacity rather than dynamically growing on every event:

- bounded VM value stack
- bounded local slots
- bounded function call depth
- bounded native output batch
- per-handler instruction budget

Exceeding a limit causes the handler to stop/fail rather than silently allocating an unbounded structure on the realtime thread.

## Choosing the right layer

Use normal Bun/TypeScript for:

- configuration
- persistent storage on disk
- networking
- hot reload
- logging
- overlay application state
- plugins
- profile selection

Use realtime TypeScript for:

- state transitions tied to input
- combo counters
- tap/hold state machines
- conditional sequences
- small loops
- input-sensitive functions
- timing-critical dispatch

A useful mental model is: **Bun owns the application; Rune VM owns the input interrupt path.**
