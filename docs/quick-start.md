# Quick Start

This guide gets Rune from a fresh clone to a running TypeScript macro.

## Requirements

- Bun 1.3.14+
- Rust 1.81+
- Python 3 is only required by the one-time source materializer during repository development; end users do not need it after the runtime source is committed.

## 1. Clone and install

```bash
git clone https://github.com/eunhhu/rune.git
cd rune
bun install
cargo build -p rune-native --release
```

Rune's TypeScript SDK loads the native library from `target/release` automatically. You may override it with `RUNE_NATIVE_PATH`.

## 2. Create a macro

Create `macro.ts`:

```ts
import { Key, MouseButton, macro, rune } from "@rune/sdk";

const lunge = macro("lunge", (m) => {
  m.on.keyDown(Key.Q).run(
    m.key.down(Key.E),
    m.mouse.down(MouseButton.Left),
    m.delay.us(80),
    m.mouse.up(MouseButton.Left),
    m.key.up(Key.E),
  );
});

rune.load(lunge).start();
```

Run it:

```bash
bun macro.ts
```

## 3. Stateful realtime TypeScript

Rune also has a realtime TypeScript subset for stateful macros. The source is compiled once into native bytecode; Bun does not execute a callback for every input event.

```ts
import { Key, delay, held, key, on, rt } from "@rune/sdk";

rt.load(() => {
  let combo = 0;

  function burst(count: number) {
    for (let i = 0; i < count; i++) {
      key.tap(Key.E);
      delay.us(40);
    }
  }

  on.keyDown(Key.Q, () => {
    combo++;

    if (combo >= 3 && held(Key.LeftShift)) {
      burst(2);
      combo = 0;
    }
  });
});
```

Persistent variables are stored in the native runtime and survive across input events.

Use ordinary Bun/TypeScript outside `rt.load()` for unrestricted application logic, files, networking, UI state, and configuration. Use `rt.load()` only for latency-sensitive input logic.

See [TypeScript Runtime](typescript-runtime.md) for the supported realtime language subset.

## Platform setup

### Windows

No extra device permission is normally required. Synthetic input follows normal Windows integrity/UIPI rules; Rune cannot inject into a higher-integrity process from a lower-integrity one.

### macOS

Grant Rune's terminal or executable:

- Input Monitoring
- Accessibility

Restart the process after granting permissions.

### Linux

Rune uses evdev for observation and uinput for injection. The current development setup requires access to `/dev/input/event*` and `/dev/uinput`.

A sample rule is provided at `packaging/linux/99-rune-input.rules`.

```bash
sudo cp packaging/linux/99-rune-input.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Log out/in or reconnect the device if permissions do not update immediately.

## Verify the checkout

```bash
cargo test --workspace
cargo build -p rune-native --release
bun run typecheck
bun run test:ts
```

## Benchmark the native core

```bash
bun run bench
```

The benchmark measures Rune's native trigger lookup and execution path. It does not include keyboard USB polling or the target application's own input polling.

## Next

- [API Reference](api.md)
- [TypeScript Runtime](typescript-runtime.md)
- [Platform Notes](platforms.md)
- [Troubleshooting](troubleshooting.md)
- [Architecture](architecture.md)
