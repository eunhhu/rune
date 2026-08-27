# Effect, 상태 동기화, RPC

[English](effects-rpc.md)

현재 값을 계속 보관해야 하면 state, 한 번 발생한 사건을 전달하려면 effect, realtime VM 밖의 작업을 요청하려면 RPC를 사용합니다.

## Typed effect

effect는 module scope에서 선언합니다. schema는 숫자 또는 boolean field를 최대 8개까지 가질 수 있습니다.

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

compiler는 schema 순서대로 integer 값을 push하고 숫자 channel ID와 field 수만 가진 `EmitEffect` instruction 하나를 생성합니다. VM payload는 inline `[i64; 8]`입니다. allocation, 이름 lookup, JSON 직렬화, JavaScript callback이 없습니다.

빈도가 높은 consumer는 structured object allocation도 피할 수 있습니다.

```ts
import { readRuntimeEventI64 } from "spellwire";

app.host.effects.onRaw("activated", (record, offset, length) => {
  const count = readRuntimeEventI64(record, offset);
  const enabled = readRuntimeEventI64(record, offset + 2) !== 0n;
  // 동기적으로 소비해야 합니다. 다음 drain에서 record를 재사용합니다.
});
```

## 변경된 state

state store는 이전 `i64`와 새 값을 비교합니다. 실제 변경만 record로 publish합니다.

```ts
const unsubscribeState = app.host.onStateChange((state) => {
  console.log(state.count, state.enabled);
});
```

`snapshotStates()`는 대기 중인 record를 drain한 뒤 보통 Bun 쪽 cache를 읽습니다. ring overflow나 program reload를 발견하면 native bulk snapshot 한 번으로 복구합니다. slot마다 FFI를 호출하지 않습니다.

`Spellwire.start({ overlay })`는 이 구독을 자동으로 사용합니다. 기본 overlay에는 frame polling timer가 없고 state가 바뀐 뒤에만 reconcile합니다. 외부 값도 주기적으로 읽어야 하는 UI는 `overlayOptions.fps`를 명시하면 기존 periodic refresh를 사용할 수 있습니다.

## Local RPC

server는 Bun control plane에서 실행됩니다. observer callback과 코드 경로를 공유하지 않으며 원격 client가 구독하기 전에는 VM에 추가 작업을 만들지 않습니다.

```ts
import { SpellwireRpcServer } from "spellwire/rpc/server";

const rpc = await SpellwireRpcServer.start(app.host);
rpc.expose<{ name: string }>("profile.select", ({ name }) => selectProfile(name));

console.log(rpc.endpoint);
console.log(rpc.token); // 기존의 신뢰할 수 있는 launch/IPC channel로 전달
```

Electron, Node 또는 별도 JavaScript process에서 연결할 수 있습니다. `spellwire/rpc`는 Node 호환 module만 import하며 `bun:ffi`를 load하지 않습니다.

```ts
import { SpellwireRpcClient } from "spellwire/rpc";

const client = await SpellwireRpcClient.connect({ endpoint, token });
await client.setState("enabled", true);
console.log(await client.snapshotStates());

const stopState = await client.onState((state) => render(state));
const stopEffect = await client.onEffect("activated", (payload) => notify(payload));
const profile = await client.call("profile.select", { name: "game" });
```

내장 method는 `state.get`, `state.set`, `state.snapshot`, `state.subscribe`, `state.unsubscribe`, `effect.subscribe`, `effect.unsubscribe`입니다. custom method는 예약된 `rpc.`, `state.`, `effect.` prefix를 사용할 수 없습니다.

transport는 macOS/Linux의 length-prefixed local socket과 Windows named pipe입니다. 모든 연결은 다른 method를 사용하기 전에 server의 random token과 같은 protocol version으로 인증해야 합니다. token을 직접 지정하면 16자 이상이어야 합니다. Unix socket permission은 owner-only `0600`으로 바뀝니다. endpoint와 token은 capability이므로 공용 log에 남기거나 신뢰할 수 없는 renderer에 넘기지 마십시오.

## 성능과 전달 규칙

- observer callback은 기존처럼 bounded translation, suppression lookup, input queue publish만 수행합니다. effect/RPC 경로를 건드리지 않습니다.
- state/effect는 native producer와 Bun consumer 사이의 preallocated SPSC ring 하나를 사용합니다. full이면 `dropped`만 증가시키고 realtime 실행은 Bun을 기다리지 않습니다.
- state는 durable하므로 overflow 뒤 bulk snapshot으로 복구합니다. effect는 transient라 overflow 시 유실될 수 있습니다. 모든 발생이 중요하면 lane 크기와 drain 빈도를 조정해야 합니다.
- native event lane과 pump는 state/effect subscriber가 있을 때만 존재합니다. 직접 제어할 때 `host.pollEvents()`를 호출하면 timer 없이 lane을 attach합니다.
- structured effect handler는 JavaScript payload object를 만듭니다. `onRaw`는 이 allocation을 피합니다.
- RPC framing과 JSON은 ring 이후 control plane에서 실행됩니다. VM latency에는 영향을 주지 않지만 느린 RPC client는 control-plane backpressure를 만들 수 있습니다.

native dispatch benchmark에 fixed-payload effect 하나를 포함하려면 `bun run bench -- 1000000 --effect`를 실행하십시오.

반환된 unsubscribe 함수를 모두 호출하고 RPC client/server를 닫은 뒤 Spellwire app을 닫으십시오.
