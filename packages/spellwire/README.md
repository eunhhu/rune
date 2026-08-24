# spellwire

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

Compile the script:

```bash
bunx spellwire compile macro.spellwire.ts
```

The alpha package includes the SDK, AOT compiler, CLI, JavaScript fallback lane, and host-facing types. Prebuilt global-input backends are not bundled yet. See the repository documentation for the native VM/C ABI and implementation status.
