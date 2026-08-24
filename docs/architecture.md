# Architecture

```text
macro.spellwire.ts
   │
   ├─ ordinary Bun module (control plane)
   │      ├─ config / async I/O / plugins
   │      ├─ dynamic input lane
   │      └─ retained overlay mutations
   │
   └─ spellwire
          ├─ discovers rt.on* handlers
          ├─ captures representable module `let` state
          ├─ inlines helper functions
          ├─ lowers branches and loops
          └─ emits versioned SPWR bytecode
                         │
                         ▼
                    spellwire-core
          ├─ fixed trigger buckets
          ├─ persistent i64 state slots
          ├─ fixed stack / locals / output batch
          ├─ absolute monotonic deadlines
          └─ instruction budget
                         │
                         ▼
                   spellwire-native ABI
          ├─ program load/unload
          ├─ event dispatch
          ├─ state get/set
          └─ output backend boundary
```

## Hot-path invariants

After a program is loaded, dispatch avoids:

- JavaScript execution
- heap allocation
- async runtimes and promises
- locks in the VM
- hash tables for trigger lookup
- per-action native calls when adjacent outputs can be batched

The VM uses fixed-capacity scratch arrays. Program validation occurs before the runtime accepts bytecode. Every execution has an instruction budget, and every jump target/state/local index is validated.

## TypeScript compilation boundary

Spellwire compiles only code reachable from `rt.on*` callbacks. Arbitrary top-level TypeScript can coexist in the same file and remains in Bun. A value produces a compiler error only when realtime code captures it and the VM cannot represent its semantics.

This preserves a useful rule: the script is TypeScript first, while the latency-sensitive subset has an explicit static boundary.

## Persistent state

Module-scope integer and boolean `let` declarations referenced by realtime handlers become native state slots. Locals reset for each handler dispatch; state slots do not. The compiler emits a state manifest containing stable names and slot IDs so the control plane can inspect or update them through the native ABI.

## Scheduling

Delays are represented as deadlines, not chains of relative sleeps. Output accumulated before a delay is flushed as one batch. A platform backend can sleep for the coarse portion and spin only for a calibrated tail. The VM's default spin threshold is a policy value, not a universal guarantee.
