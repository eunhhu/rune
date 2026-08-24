# TypeScript execution model

Spellwire is not a callback wrapper around a generic automation package. It has two TypeScript execution lanes because full JavaScript semantics and predictable microsecond-scale dispatch are different constraints.

## Realtime AOT lane

Handlers registered with `rt.onKeyDown`, `rt.onKeyUp`, `rt.onMouseDown`, and `rt.onMouseUp` are parsed from TypeScript and lowered to Spellwire bytecode before the input runtime starts.

Inside those handlers Spellwire currently supports ordinary TypeScript syntax for:

- persistent module-scope integer and boolean variables
- local variables and assignments
- `if` / `else` and conditional expressions
- `for`, `while`, and `do` loops with `break` / `continue`
- integer arithmetic, comparisons, bit operations, and boolean short-circuiting
- top-level helper functions with parameters, inlined at compile time
- key/mouse output, held-key queries, and microsecond deadlines

The native VM owns captured state. An input dispatch therefore performs no native-to-JavaScript callback, JS allocation, promise scheduling, or property lookup.

```ts
let phase = 0;

function burst(count: number): void {
  for (let i = 0; i < count; i++) tapKey(Key.E);
}

rt.onKeyDown(Key.Q, () => {
  phase = (phase + 1) % 3;
  if (phase !== 0) burst(phase);
});
```

This is TypeScript syntax, but not every JavaScript value can be represented in the realtime VM. Objects, strings, closures over dynamic objects, exceptions, promises, recursion, and unbounded platform APIs stay out of this lane. The compiler reports a source-positioned error when realtime code captures an unsupported value. Every handler also has a native instruction budget so a bad loop cannot permanently occupy the input thread.

## Dynamic Bun lane

The rest of the module remains ordinary TypeScript running in Bun. This lane is for configuration, files, networking, UI orchestration, plugins, complex objects, async work, and observability.

Dynamic input subscriptions use a fixed-record `SharedArrayBuffer` SPSC ring. A dedicated worker drains the ring instead of invoking a JavaScript callback directly from the native input thread. This lowers overhead and isolates the native producer, but it is still best-effort JavaScript and does not carry Spellwire's realtime latency contract.

## State boundary

Realtime state is persistent across handler invocations and is addressable from the control plane through generated state metadata and the native `state_get` / `state_set` ABI. A future source transform can rewrite dynamic references to captured variables automatically; the initial ABI exposes explicit state handles so synchronization is never implicit or racy.

## Why not execute arbitrary Bun callbacks on the hot path?

Bun is fast, but event-loop scheduling, garbage collection, deoptimisation, and native-to-JS transitions introduce tail latency. They are acceptable for control work and unacceptable as the only path for a macro that advertises microsecond-scale dispatch. Spellwire therefore optimizes TypeScript by moving analyzable hot code into a small native machine while retaining Bun for the language features that genuinely need JavaScript.
