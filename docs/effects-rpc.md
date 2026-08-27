# Effects, state synchronization, and RPC

[한국어](effects-rpc.ko.md)

Use state for a durable current value, an effect for a transient occurrence, and RPC for work that belongs outside the realtime VM.

## Typed effects

Declare an effect at module scope. A schema may contain up to eight numeric or boolean fields.

```ts
import { Key, Spellwire, effect, rt } from "spellwire";

let count = 0;
const activated = effect("activated", {
  count: "number",
  enabled: "boolean",
});

rt.hotkey("F6", () => {
  count += 1;
  activated.emit({ count, enabled: true });
});

const app = await Spellwire.start({ input: import.meta.file });
const unsubscribe = app.host.effects.on("activated", ({ count, enabled }) => {
  console.log({ count, enabled });
});
```

The compiler orders fields by the schema, pushes their integer values, and emits one `EmitEffect` instruction containing only a numeric channel ID and field count. The VM payload is an inline `[i64; 8]`; it does not allocate, resolve names, serialize JSON, or invoke JavaScript.

For high-rate consumers, avoid the structured object allocation:

```ts
import { readRuntimeEventI64 } from "spellwire";

app.host.effects.onRaw("activated", (record, offset, length) => {
  const count = readRuntimeEventI64(record, offset);
  const enabled = readRuntimeEventI64(record, offset + 2) !== 0n;
  // Consume synchronously. `record` is reused by the next drain.
});
```

## Changed state

Every state store compares the old and new `i64`. Only a real change publishes a record:

```ts
const unsubscribeState = app.host.onStateChange((state) => {
  console.log(state.count, state.enabled);
});
```

`snapshotStates()` drains pending records and normally reads the maintained Bun-side cache. If the ring reports overflow or a program reload, it performs one bulk native snapshot to recover. It never repairs state with one FFI call per slot.

`Spellwire.start({ overlay })` uses this subscription automatically. The default overlay has no frame polling timer; it reconciles after a changed-state record. Setting `overlayOptions.fps` explicitly restores periodic refresh for UIs that also read external values.

## Local RPC

The server runs in the Bun control plane. It has no code path into the observer callback and adds no VM work until a remote client subscribes.

```ts
import { SpellwireRpcServer } from "spellwire/rpc/server";

const rpc = await SpellwireRpcServer.start(app.host);
rpc.expose<{ name: string }>("profile.select", ({ name }) => selectProfile(name));

console.log(rpc.endpoint);
console.log(rpc.token); // deliver through an existing trusted launch/IPC channel
```

Connect from Electron, Node, or another bundled JavaScript process. `spellwire/rpc` imports only Node-compatible modules and does not load `bun:ffi`.

```ts
import { SpellwireRpcClient } from "spellwire/rpc";

const client = await SpellwireRpcClient.connect({ endpoint, token });
await client.setState("enabled", true);
console.log(await client.snapshotStates());

const stopState = await client.onState((state) => render(state));
const stopEffect = await client.onEffect("activated", (payload) => notify(payload));
const profile = await client.call("profile.select", { name: "game" });
```

Built-in methods are `state.get`, `state.set`, `state.snapshot`, `state.subscribe`, `state.unsubscribe`, `effect.subscribe`, and `effect.unsubscribe`. Custom methods cannot use the reserved `rpc.`, `state.`, or `effect.` prefixes.

The transport is a length-prefixed local socket on macOS/Linux and a named pipe on Windows. Each connection must authenticate with the server's random token and matching protocol version before using another method. Explicit tokens must contain at least 16 characters. Unix socket permissions are changed to owner-only (`0600`). Treat endpoint and token as capabilities: do not print them into shared logs or pass them to untrusted renderers.

## Performance and delivery rules

- Observer callbacks still perform bounded translation, suppression lookup, and one input-queue publish only. They do not touch the effect/RPC path.
- State/effect records use one native-producer/Bun-consumer preallocated SPSC ring. A full ring increments `dropped`; realtime execution never waits for Bun.
- Durable state recovers from overflow by bulk snapshot. Effects are transient and may be lost on overflow; size the lane or drain more frequently when every occurrence matters.
- The native event lane and pump exist only while state/effect subscribers are registered. Manual `host.pollEvents()` attaches the lane without a timer.
- Structured effect handlers allocate a JavaScript payload object. `onRaw` avoids that allocation.
- RPC framing and JSON run after the ring on the control plane. They never change VM latency, but a slow RPC client can still increase control-plane memory/backpressure.

Run `bun run bench -- 1000000 --effect` to include one fixed-payload effect in the native dispatch benchmark.

Call every returned unsubscribe function, close RPC clients/servers, then close the Spellwire app.
