# Troubleshooting

## `bun add spellwire` returns 404

The packages have not been published yet. Merge the release-ready branch, configure npm publishing, and publish `spellwire` before `create-spellwire`. See [publishing.md](publishing.md).

## `spellwire` command not found

Ensure `spellwire` is a direct dependency and run through Bun's package runner:

```bash
bunx spellwire --help
bunx spellwire compile macro.spellwire.ts
```

Inside a generated project, `bun run build` invokes the local binary automatically.

## Realtime intrinsic executed outside a handler

Output and held-state intrinsics require either compiled realtime execution or `withRealtimeActionSink()` in JavaScript tests.

## Compiler rejects an object, Promise, or dynamic property

Only the analyzable realtime subset is lowered to native bytecode. Keep dynamic objects, network work, file I/O, async functions, and UI logic in ordinary Bun control-plane code.

## The package compiles macros but does not observe global input

That is the current alpha boundary. The npm package does not yet include validated/prebuilt OS observation and injection backends. Build and embed `spellwire-native` to exercise the VM host ABI, or test the compiler and JavaScript fallback lane until native packages are released.

## macOS or Linux input permissions

See [platforms.md](platforms.md). Spellwire does not advertise a backend capability until permission behavior and latency have been verified.

## Performance results look noisy

Measure release builds, pin the workload where practical, warm up first, and report percentile distributions rather than only averages. Distinguish VM dispatch from HID polling, OS injection, and target-application polling.
