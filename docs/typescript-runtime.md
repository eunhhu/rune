# TypeScript Runtime

Spellwire uses TypeScript as an authoring language and compiles the latency-sensitive subset ahead of time. A source file does not need a wrapper such as `rt.load()`; the `.spellwire.ts` module itself is the compilation unit.

## Compilation boundary

The compiler scans the module for top-level calls to:

```ts
rt.onKeyDown(...)
rt.onKeyUp(...)
rt.onMouseDown(...)
rt.onMouseUp(...)
```

Only code needed by those handlers is lowered to native bytecode. Other top-level TypeScript may coexist as control-plane code, but a handler cannot capture a dynamic value that the compiler cannot represent.

## Persistent state

A module-scope mutable `let` initialized to a compile-time integer or boolean becomes a persistent native state slot when a realtime handler references it.

```ts
let phase = 0;
let enabled = true;

rt.onKeyDown(Key.Q, () => {
  if (enabled) phase = (phase + 1) % 3;
});
```

The state survives after the handler returns. The generated `.spellwire.bin.json` manifest maps each source name to its native slot and kind.

Module-scope `const` declarations are folded as compile-time constants when possible.

Spellwire numbers on the realtime plane are signed 64-bit integers. Numeric literals must be safe JavaScript integers at compile time. Booleans are represented as native integer truth values.

## Handler-local variables

Handler and helper-function locals compile to fixed VM local slots:

```ts
rt.onKeyDown(Key.Q, () => {
  let count = phase + 1;
  count *= 2;
  tapKey(Key.E);
});
```

Destructuring and dynamically sized local containers are not supported.

## Conditions and expressions

The compiler supports the integer/boolean operations needed for state machines, including:

- arithmetic and remainder;
- comparison and equality;
- logical short-circuit expressions;
- bitwise operations and shifts;
- prefix unary operations;
- assignment and compound assignment;
- prefix/postfix increment and decrement.

```ts
if (enabled && phase >= 2 && !keyHeld(Key.LeftShift)) {
  tapKey(Key.E);
} else {
  tapKey(Key.R);
}
```

Unsupported expressions fail compilation with a source-position diagnostic instead of falling back silently to JavaScript.

## Loops

Supported loop forms:

```ts
for (let index = 0; index < count; index++) {
  tapKey(Key.E);
}

while (enabled) {
  break;
}

do {
  phase++;
} while (phase < 3);
```

`break` and `continue` are supported. Every handler has an instruction budget, so an accidental infinite loop fails rather than running without a bound.

## Helper functions

Top-level helper functions called by handlers are compiled inline:

```ts
function tapRepeated(key: Key, count: number): void {
  for (let index = 0; index < count; index++) {
    keyDown(key);
    keyUp(key);
  }
}
```

Current restrictions:

- helper functions return `void` only;
- arguments must lower to integer values;
- functions are inlined rather than dynamically dispatched;
- recursion is rejected;
- runtime-created closures are not supported.

Inlining removes a per-event JavaScript or VM function-call boundary, but it can increase bytecode size.

## Realtime intrinsics

```ts
keyDown(Key.E)
keyUp(Key.E)
tapKey(Key.E)
mouseDown(MouseButton.Left)
mouseUp(MouseButton.Left)
clickMouse(MouseButton.Left)
moveMouse(4, -2)
wheelMouse(0, 1)
sleepUs(75)
keyHeld(Key.LeftShift)
mouseHeld(MouseButton.Right)
```

The compiler recognizes these functions by name and emits native opcodes.

## Delay behavior

`sleepUs(n)` flushes the pending output batch and advances an absolute monotonic deadline. The runtime sleeps until the configured spin tail, then actively spins for the remaining interval.

The current VM executes delays synchronously on the dispatching thread. A preallocated continuation/deadline scheduler is planned so long macro waits can yield without blocking input observation.

Desktop operating systems are not hard realtime schedulers; microsecond syntax is a requested deadline, not a guarantee that physical end-to-end latency has the same precision.

## Deliberately unsupported

The realtime compiler rejects or excludes features that require an unconstrained JavaScript runtime or unpredictable allocation:

- floating-point semantics as a separate type;
- strings in realtime expressions;
- dynamic objects, arrays, maps, and sets;
- destructuring;
- `async`, `await`, and `Promise`;
- exceptions as general control flow;
- generators;
- arbitrary npm/Bun APIs;
- network or filesystem I/O;
- dynamic property access;
- runtime-created closures;
- non-void helper returns.

Move such work to ordinary Bun code and exchange only bounded state/configuration with the native host.

## Resource limits

Current defaults and caps:

| Resource | Value |
| --- | ---: |
| Default stack limit | 128 values |
| Native maximum stack | 256 values |
| Native maximum locals | 256 values |
| Native output batch | 64 events |
| Default instruction budget | 100,000 instructions/handler |

Programs are validated before dispatch. Invalid jumps, slots, entries, limits, and empty programs are rejected during loading.

## Fallback execution

Executing a `.spellwire.ts` module directly with Bun records the `rt.on*` registrations in a JavaScript fallback list. With `withRealtimeActionSink()`, tests can call those handlers and observe their actions.

That fallback preserves useful semantics for debugging, but it is not the native AOT path and carries no realtime latency guarantee.
