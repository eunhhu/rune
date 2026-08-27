# Troubleshooting

[한국어](troubleshooting.ko.md)

## `spellwire` or `spellwire/compiler` cannot be resolved

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

## A `sleep.*()` delay is inaccurate

The live host uses absolute deadlines and non-blocking continuations, but a general-purpose desktop OS is not a hard realtime scheduler. The compatibility engine/simulator executes delays synchronously, so long waits make simulation appear paused.

Microsecond input is a deadline request, not a physical end-to-end guarantee.

## `bun macro.spellwire.ts` produces no global keyboard events

Executing a source module directly registers fallback handlers only. Start the native host through the CLI:

Use:

```bash
bun run build:native
bun packages/spellwire/src/cli.ts run macro.spellwire.ts
```

`run` checks and requests permissions before host startup. Use `spellwire watch macro.spellwire.ts` when source changes should reload automatically.

On Linux, configure evdev/uinput access. On Windows, injection into a higher-integrity process is blocked by UIPI. See [Platform Status](platforms.md).

Follow [Live Native Host Guide](live-host.md) for the exact host lifecycle and [Platform Verification](platform-verification.md) for OS-specific permission checks.

## Native library output does nothing

The low-level `SpellwireEngine` discards output if no callback is installed. Use the owned `NativeHost`/`spellwire_host_*` lifecycle for built-in OS injection, or install an engine output callback in a custom embedder.

Normal CLI startup reports missing permissions automatically. Advanced embedders can check `host.permissionStatus()` and `spellwire_host_last_error`. Unsupported HID usages return an explicit platform error.

## Native overlay does not start

Build both native targets with `bun run build:native`. Set `SPELLWIRE_OVERLAY_EXECUTABLE` for a nonstandard location. Linux needs an active graphical session; always-on-top transparency remains compositor-dependent.

Run the target-specific smoke command and compare its stderr with the failure matrix in [Platform Verification](platform-verification.md).

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
*.spellwire.bin
*.spellwire.bin.json
packages/*/dist/
*.tsbuildinfo
```

Remove TypeScript build outputs with `bun run clean:ts`.

## Rust tests pass but Clippy prints warnings

The workspace enables `clippy::pedantic`. CI and `bun run check` pass `-D warnings`, so warnings are blocking.

## Verify everything locally

```bash
bun run check
cargo clippy --workspace --all-targets --locked
bun run compile:example
bun run inspect:example
bun run simulate:example
bun run test:platform-loopback
target/release/spellwire-overlay --smoke
```

Use `target/release/spellwire-overlay.exe --smoke` on Windows. The OS loopback and platform benchmark need permissions and should be recorded separately from portable source checks. [Platform Verification](platform-verification.md) gives exact expected output and interpretation.

## Reporting a compiler/VM issue

Include:

- OS and CPU;
- Bun and Rust versions;
- commit SHA;
- minimal `.spellwire.ts` source;
- compiler diagnostic or simulator output;
- generated manifest when state mapping matters.

Latency reports must identify whether they measure core dispatch, OS submission, OS loopback, or physical switch-to-application behavior.
