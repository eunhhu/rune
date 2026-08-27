# spellwire

[한국어](README.ko.md)

Stateful realtime input automation for Bun and TypeScript.

```bash
bun add spellwire
```

```ts
import { Key, rt, tapKey } from "spellwire";

let count = 0;
let enabled = true;

rt.hotkey("Ctrl+Q", () => {
  count += 1;
  if (count % 2 === 0) tapKey(Key.E);
}, { repeat: false, when: () => enabled });

rt.remap("CapsLock", "Escape", { when: () => enabled });
```

Run, watch, or compile the script:

```bash
bunx spellwire run macro.spellwire.ts
bunx spellwire watch macro.spellwire.ts
bunx spellwire compile macro.spellwire.ts
```

The package includes the SDK, AOT compiler, CLI, lock-free consuming hotkeys/remaps on Windows/macOS, native state gates, Bun FFI native host, named state/hot reload, fixed-payload effects, authenticated local RPC, shared event lanes, and retained native overlay client. Release tarballs are assembled with platform-native runtime and overlay artifacts by the publish workflow.

State and overlay can share one lifecycle without update boilerplate:

```ts
import { Spellwire, ui } from "spellwire";

const app = await Spellwire.start({
  input: import.meta.file,
  watch: true,
  overlayOptions: { window: { alwaysOnTop: true, focusable: false, clickThrough: true } },
  overlay: (state) => ui.column(
    { width: 280, padding: 16, gap: 8, fill: "#111827ee", radius: 16 },
    ui.text(state.enabled === true ? "Active" : "Paused"),
  ),
});
await app.untilSignal();
```

Both snippets can live in the same `src/main.ts`; only the `rt.*` handlers enter the native VM.

## State changes, effects, and app integration

Use state for a durable current value and an effect for a transient occurrence:

```ts
import { Spellwire, effect, rt } from "spellwire";

let count = 0;
let enabled = true;
const activated = effect("activated", { count: "number", enabled: "boolean" });

rt.hotkey("F6", () => {
  count += 1;
  activated.emit({ count, enabled });
});

const app = await Spellwire.start({ input: import.meta.file });
const stop = app.host.effects.on("activated", ({ count, enabled }) => {
  console.log({ count, enabled });
});
```

The compiler lowers `emit` to one fixed-width native opcode with an inline `[i64; 8]` payload. Changed state and effects cross a preallocated SPSC lane after realtime execution; no JavaScript, socket, or JSON runs on the input callback. `app.host.onStateChange()` provides fine-grained state updates, and the default `Spellwire.start({ overlay })` path refreshes only after a change.

Use `SpellwireRpcServer` from `spellwire/rpc/server` and `SpellwireRpcClient` from `spellwire/rpc` to connect an Electron/Node build or sidecar over an authenticated local socket/named pipe. The client subpath has no `bun:ffi` dependency. See [Effects, state synchronization, and RPC](https://github.com/eunhhu/spellwire/blob/main/docs/effects-rpc.md) for raw allocation-free subscriptions, complete server/client code, security, and overflow behavior.

`run` and `watch` compile source in memory and prepare platform permissions before native startup. `watch` only adds control-plane filesystem reload; native realtime dispatch stays callback-free. For deterministic compiler/VM integration testing, the repository also contains `spellwire-sim`.

Detailed guides:

- [Live Native Host Guide](https://github.com/eunhhu/spellwire/blob/main/docs/live-host.md)
- [Platform Verification Guide](https://github.com/eunhhu/spellwire/blob/main/docs/platform-verification.md)
- [API Reference](https://github.com/eunhhu/spellwire/blob/main/docs/api.md)
- [Hotkeys and AutoHotkey migration](https://github.com/eunhhu/spellwire/blob/main/docs/automation.md)
- [State-driven Overlay](https://github.com/eunhhu/spellwire/blob/main/docs/overlay.md)
- [Effects, state synchronization, and RPC](https://github.com/eunhhu/spellwire/blob/main/docs/effects-rpc.md)
