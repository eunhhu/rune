# spellwire

[한국어](README.ko.md)

Stateful realtime input automation for Bun and TypeScript.

```bash
bun add spellwire
```

```ts
import { Key, rt, tapKey } from "spellwire";

let count = 0;

rt.onKeyDown(Key.Q, () => {
  count += 1;
  if (count % 2 === 0) tapKey(Key.E);
});
```

Run, watch, or compile the script:

```bash
bunx spellwire run macro.spellwire.ts
bunx spellwire watch macro.spellwire.ts
bunx spellwire compile macro.spellwire.ts
```

The package includes the SDK, AOT compiler, CLI, Bun FFI native host, named state/hot reload, shared dynamic lane, and retained native overlay client. Release tarballs are assembled with platform-native runtime and overlay artifacts by the publish workflow.

State and overlay can share one lifecycle without update boilerplate:

```ts
import { Spellwire, ui } from "spellwire";

const app = await Spellwire.start({
  input: "src/main.spellwire.ts",
  watch: true,
  overlay: (state) => ui.column(
    { width: 280, padding: 16, gap: 8, fill: "#111827ee", radius: 16 },
    ui.text(state.enabled === true ? "Active" : "Paused"),
  ),
});
await app.untilSignal();
```

`run` and `watch` compile source in memory and prepare platform permissions before native startup. `watch` only adds control-plane filesystem reload; native realtime dispatch stays callback-free. For deterministic compiler/VM integration testing, the repository also contains `spellwire-sim`.

Detailed guides:

- [Live Native Host Guide](https://github.com/eunhhu/spellwire/blob/main/docs/live-host.md)
- [Platform Verification Guide](https://github.com/eunhhu/spellwire/blob/main/docs/platform-verification.md)
- [API Reference](https://github.com/eunhhu/spellwire/blob/main/docs/api.md)
- [State-driven Overlay](https://github.com/eunhhu/spellwire/blob/main/docs/overlay.md)
