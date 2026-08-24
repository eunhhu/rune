# Troubleshooting

## `@rune/sdk` or `@rune/compiler` cannot be resolved

Install the workspace with Bun:

```bash
bun install --frozen-lockfile
```

Run commands from the repository root so Bun can resolve the workspace packages.

## TypeScript build errors mention missing declaration outputs

The SDK and compiler use TypeScript project references. Use build mode rather than checking each project independently:

```bash
bun run typecheck
```

To remove generated declaration/build metadata:

```bash
bun run clean:ts
```

## Compilation says no realtime handlers were found

Registrations must be top-level calls with an inline callback:

```ts
rt.onKeyDown(Key.Q, () => {
  tapKey(Key.E);
});
```

A registration hidden inside another runtime function is not discovered by the current compiler.

## A handler rejects ordinary TypeScript

The module may contain unrestricted control-plane TypeScript, but realtime handlers can reference only values the compiler can lower to bounded integer bytecode.

Common causes:

- capturing a string/object/array;
- calling an arbitrary Bun/npm function;
- a non-constant key or handler option;
- a helper that returns a value;
- recursion or a runtime-created closure;
- destructuring or dynamic property access.

See [TypeScript Runtime](typescript-runtime.md).

## `sleepUs()` is inaccurate

The current runtime uses an absolute deadline and a spin tail, but a general-purpose desktop OS is not a hard realtime scheduler. The simulator also executes delays synchronously, so long waits make the command appear paused.

Microsecond input is a deadline request, not a physical end-to-end guarantee.

## `bun macro.rune.ts` produces no global keyboard events

That is expected in the current branch. Executing the source directly registers JavaScript fallback handlers; it does not install a Windows/macOS/Linux observer.

Use:

```bash
bun packages/compiler/src/cli.ts macro.rune.ts
cargo run -q -p rune-cli -- simulate macro.rune.bin key-down:Q key-up:Q
```

Direct OS backends are listed as planned in [Platform Status](platforms.md).

## Native library output does nothing

`rune-native` discards output if no host callback is installed. A host must call `rune_engine_set_output_callback` and translate received batches to its platform injection API.

The ABI does not currently provide `start()` or `stop()` functions for global observation. See [Native C ABI](native-abi.md).

## Simulator rejects an event

Use one of these forms:

```text
key-down:Q
key-up:LeftShift
mouse-down:left
mouse-up:forward
key-down:0x14:synthetic
```

The optional source is `physical` or `synthetic`. Handler filters may use `InputSource.Any`, but an actual event itself must have a concrete source.

## Generated files keep appearing

Compiler outputs and TypeScript build products are ignored by Git:

```text
*.rune.bin
*.rune.bin.json
packages/*/dist/
*.tsbuildinfo
```

Remove TypeScript build outputs with `bun run clean:ts`.

## Rust tests pass but Clippy prints warnings

The workspace enables `clippy::pedantic` as warnings. During the MVP, Clippy runs in CI but pedantic findings are advisory rather than merge-blocking. Build/test failures remain blockers.

## Verify everything locally

```bash
bun run check
cargo clippy --workspace --all-targets --locked
bun run compile:example
bun run inspect:example
bun run simulate:example
```

## Reporting a compiler/VM issue

Include:

- OS and CPU;
- Bun and Rust versions;
- commit SHA;
- minimal `.rune.ts` source;
- compiler diagnostic or simulator output;
- generated manifest when state mapping matters.

Latency reports are premature for platform input until a direct backend exists. Core benchmark reports should identify that they measure VM dispatch only.
