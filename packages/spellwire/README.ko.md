# spellwire

[English](README.md)

Bun과 TypeScript를 위한 상태 기반 실시간 입력 자동화 패키지입니다.

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

CLI 명령은 다음 세 가지입니다.

```bash
bunx spellwire run macro.spellwire.ts
bunx spellwire watch macro.spellwire.ts
bunx spellwire compile macro.spellwire.ts
```

`run`과 `watch`는 소스를 메모리에서 AOT 컴파일하고 플랫폼 권한을 준비한 뒤 동일한 네이티브 호스트를 시작합니다. `watch`가 추가하는 작업은 control-plane 파일 감시와 직렬화된 reload뿐이며 네이티브 실시간 dispatch 경로는 JavaScript callback 없이 유지됩니다.

상태와 overlay를 boilerplate 없이 하나의 lifecycle로 연결할 수 있습니다.

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

두 code block은 같은 `src/main.ts`에 둘 수 있고 `rt.*` handler만 native VM으로 들어갑니다.

## 상태 변경, effect, app 연동

현재 값을 유지해야 하면 state를, 한 번 발생한 사건을 전달하려면 effect를 사용합니다.

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

compiler는 `emit`을 inline `[i64; 8]` payload를 가진 fixed-width native opcode 하나로 lowering합니다. 실제로 바뀐 state와 effect는 realtime 실행 뒤 preallocated SPSC lane으로 전달됩니다. input callback에서는 JavaScript, socket, JSON을 실행하지 않습니다. `app.host.onStateChange()`로 fine-grained state update를 받을 수 있고 기본 `Spellwire.start({ overlay })` 경로는 변경이 생긴 뒤에만 refresh합니다.

Electron/Node build나 sidecar는 `spellwire/rpc/server`의 `SpellwireRpcServer`와 `spellwire/rpc`의 `SpellwireRpcClient`로 인증된 local socket/named pipe에 연결합니다. client subpath에는 `bun:ffi` dependency가 없습니다. allocation-free raw 구독, 완전한 server/client code, 보안과 overflow 규칙은 [Effect, 상태 동기화, RPC](https://github.com/eunhhu/spellwire/blob/main/docs/effects-rpc.ko.md)에 있습니다.

자세한 내용:

- [라이브 네이티브 호스트](https://github.com/eunhhu/spellwire/blob/main/docs/live-host.ko.md)
- [플랫폼 검증](https://github.com/eunhhu/spellwire/blob/main/docs/platform-verification.ko.md)
- [API 레퍼런스](https://github.com/eunhhu/spellwire/blob/main/docs/api.ko.md)
- [Hotkey와 AutoHotkey 마이그레이션](https://github.com/eunhhu/spellwire/blob/main/docs/automation.ko.md)
- [상태 기반 오버레이](https://github.com/eunhhu/spellwire/blob/main/docs/overlay.ko.md)
- [Effect, 상태 동기화, RPC](https://github.com/eunhhu/spellwire/blob/main/docs/effects-rpc.ko.md)
