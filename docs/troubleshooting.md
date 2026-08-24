# Troubleshooting

## `@rune/sdk` cannot be resolved

From a repository checkout, run:

```bash
bun install
```

If the repository is in a development transition state before the stateful runtime source has been materialized, wait for the `Validate stateful runtime against existing SDK API` workflow on `main` to finish, then pull the latest `main`.

## Native library cannot be found

Build it first:

```bash
cargo build -p rune-native --release
```

The SDK checks common local build locations. To force a specific artifact:

```bash
RUNE_NATIVE_PATH=/absolute/path/to/librune_native.so bun macro.ts
```

Use the platform equivalent `.dll` or `.dylib` as appropriate.

## Macro starts but input is not observed

### Windows

Check whether the target process runs at a higher integrity level. Windows UIPI can prevent a normal process from injecting into an elevated one.

### macOS

Verify both permissions for the terminal/application running Rune:

- Input Monitoring
- Accessibility

After changing permissions, fully quit and restart the process.

### Linux

Check device access:

```bash
ls -l /dev/input/event* /dev/uinput
```

Install the sample udev rule if needed:

```bash
sudo cp packaging/linux/99-rune-input.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Readable keyboard event devices expose global keyboard input, so review the rule before installing it on a multi-user machine.

## Synthetic event triggers itself repeatedly

Use physical-source filtering for handlers that should react only to hardware input. Rune distinguishes physical and synthetic input where the platform backend exposes enough information to do so.

## Very short delays overshoot

Desktop operating systems are not hard realtime schedulers. Rune uses absolute deadlines and may spin for the final short part of a delay, but USB polling, thread scheduling, and the target application's own input polling remain external sources of latency.

Tune the spin tail conservatively:

```ts
rune.configure({ spinThresholdUs: 100 });
```

Higher values can reduce scheduler overshoot but consume more CPU while waiting.

## `rt.load()` rejects valid TypeScript

The realtime plane is intentionally a subset of TypeScript. Move unsupported code outside `rt.load()` and communicate through persistent state/configuration instead.

See [TypeScript Runtime](typescript-runtime.md) for the supported model.

## How to report a performance problem

Include:

- OS and version
- CPU
- keyboard/mouse polling rate if known
- Rune commit SHA
- backend in use
- p50/p95/p99 rather than only an average
- whether the overlay is enabled
- a minimal macro that reproduces the issue

For core measurements:

```bash
bun run bench
```

Do not compare Rune's internal dispatch benchmark directly with physical-switch-to-game latency; they measure different portions of the path.
