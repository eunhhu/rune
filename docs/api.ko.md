# API 레퍼런스

[English](api.md)

이 문서는 현재 source tree에 실제로 존재하는 API만 설명합니다. 예전 설계 초안의 `macro(...)`, `spellwire.start()`, `rt.load(...)`, `on.keyDown(...)`, `key.tap(...)`은 현재 SDK에서 export하지 않습니다.

## Package import

```ts
import {
  InputSource,
  NativeCapability,
  NativeHost,
  NativeOverlayRenderer,
  Overlay,
  OverlayScene,
  Spellwire,
  Key,
  Modifier,
  MouseButton,
  clickMouse,
  keyDown,
  keyHeld,
  keyUp,
  mouseDown,
  mouseHeld,
  mouseUp,
  moveMouse,
  parseHotkey,
  rt,
  sleepUs,
  tapKey,
  ui,
  wheelMouse,
} from "spellwire";
```

compiler API와 CLI workflow:

```ts
import { compileSource, encodeModule } from "spellwire/compiler";
```

```bash
bunx spellwire run [macro.spellwire.ts]
bunx spellwire watch [macro.spellwire.ts]
bunx spellwire compile macro.spellwire.ts [output.spellwire.bin]
```

입력을 생략하면 `src/main.spellwire.ts`를 사용합니다. `run`/`watch`는 source를 메모리에서 compile하고 권한을 한 번 준비한 뒤 같은 owned native host를 시작합니다. `watch`가 추가하는 것은 control-plane filesystem reload뿐입니다.

## 실시간 handler 등록

### `rt.hotkey(chord, handler, options?)`

portable modifier chord 또는 mouse chord를 등록합니다. 기본적으로 원본 입력을 consume하고 exact modifier를 요구합니다.

```ts
let enabled = true;

rt.hotkey("Ctrl+Shift+K", () => {
  tapKey(Key.Enter);
}, {
  source: InputSource.Physical,
  consume: true,
  exactModifiers: true,
  repeat: false,
  edge: "down",
  when: () => enabled,
});
```

`edge`는 `"down"` 또는 `"up"`입니다. `when`은 module-scope boolean native state 하나 또는 그 부정을 반환해야 합니다. gate가 VM dispatch와 원본 입력 차단을 함께 제어합니다. `parseHotkey(chord)`는 validation/tooling에서 같은 parser를 사용할 수 있게 `{ device, code, modifiers }`를 반환합니다. logical modifier bit는 `Modifier`로 export합니다.

### `rt.remap(from, to, options?)`

key down/up handler를 한 쌍으로 compile하고 활성 source sequence를 consume합니다.

```ts
rt.remap("CapsLock", "Escape", { when: () => enabled });
rt.remap(Key.CapsLock, Key.Escape, { repeat: false });
```

두 key는 portable 문자열 이름 하나 또는 `Key` 값입니다. option은 `source`, `repeat`, `when`입니다.

### Low-level 등록

```ts
rt.onKeyDown(key, handler, options?)
rt.onKeyUp(key, handler, options?)
rt.onMouseDown(button, handler, options?)
rt.onMouseUp(button, handler, options?)
```

AOT compile을 위해 등록 call은 top-level이어야 하고 callback은 inline arrow function 또는 function expression이어야 합니다.

```ts
rt.onKeyDown(
  Key.Q,
  () => {
    tapKey(Key.E);
  },
  { source: InputSource.Physical },
);
```

Low-level option:

- `source`: `InputSource.Physical`(기본), `Synthetic`, `Any`
- `consume`: 원본 입력 차단, low-level 기본값 `false`
- `modifiers`: `Modifier` bitmask
- `exactModifiers`: 추가 modifier 거부, 기본값 `false`
- `repeat`: repeat down 허용, 기본값 `true`
- `when`: module-scope native boolean state gate

compiler는 trigger와 option을 정적으로 해석합니다. identifier, quoted name, computed string name, shorthand constant를 지원합니다. spread, 중복 property, dynamic value, 알 수 없는 option은 compile error입니다.

문법, suppression 동작, overlay state 흐름, AutoHotkey 마이그레이션 표는 [Hotkey, remap, 상태 기반 자동화](automation.ko.md)를 참고하십시오.

## 출력 및 held intrinsic

```ts
keyDown(Key.E)
keyUp(Key.E)
tapKey(Key.E)
mouseDown(MouseButton.Left)
mouseUp(MouseButton.Left)
clickMouse(MouseButton.Left)
moveMouse(12, -4)
wheelMouse(0, 1)
sleepUs(80)
keyHeld(Key.LeftShift)
mouseHeld(MouseButton.Right)
```

zero-delay 출력은 고정 native output batch로 모입니다. live host에서 `sleepUs()`는 batch를 flush하고 fixed-capacity absolute-deadline scheduler에 continuation을 양보합니다. compatibility engine과 simulator는 동기 대기합니다.

held 함수는 VM이 handler 실행 전에 갱신한 input-state bitmap을 읽으므로 platform query가 없습니다. 일반 Bun 코드에서 output intrinsic을 직접 호출하면 `withRealtimeActionSink()`가 없는 경우 오류가 발생합니다.

## Key와 mouse button

`Key`는 USB HID keyboard usage ID를 사용합니다. 예: `Key.A`, `Key.Q`, `Key.Digit1`, `Key.Enter`, `Key.Escape`, `Key.Space`, `Key.F8`, `Key.ArrowUp`, `Key.LeftControl`, `Key.LeftShift`, `Key.LeftAlt`, `Key.LeftMeta`.

mouse button은 `MouseButton.Left`, `Right`, `Middle`, `Back`, `Forward`입니다.

## Compiler API

### `compileSource(source, options?)`

```ts
const result = compileSource(source, {
  fileName: "macro.spellwire.ts",
  stackLimit: 128,
  instructionBudget: 100_000,
});
```

`module`에는 compiled state, handler, instruction, local count, stack limit, instruction budget이 있고 `sourceFile`에는 parse한 TypeScript source file이 있습니다. 기본 stack limit은 128, handler별 instruction budget은 100,000입니다. native stack/local 최대값은 각각 256입니다.

compile 실패는 파일, line, column, message를 가진 `SpellwireCompileError`를 throw합니다.

### `encodeModule(module)`

compiled module을 `spellwire-core::Program::decode`와 `spellwire_engine_new`가 읽는 versioned native binary로 serialize합니다.

## JavaScript fallback/debug API

일반 Bun으로 source를 실행하면 `rt.on*`은 fallback 등록도 남깁니다. test/debug용이며 realtime guarantee와 global observer가 없습니다.

`getFallbackRealtimeRegistrations()`는 현재 process에 수집된 registration을 반환합니다. `withRealtimeActionSink(sink, body)`는 handler action을 관찰할 임시 sink를 설치합니다.

```ts
interface RealtimeActionSink {
  key(code: number, down: boolean): void;
  mouseButton(button: number, down: boolean): void;
  mouseMove(dx: number, dy: number): void;
  mouseWheel(x: number, y: number): void;
  delayUs(duration: number): void;
  held(device: "keyboard" | "mouse", code: number): boolean;
}
```

## Dynamic control-plane lane

`DynamicInputLane`은 SPSC `SharedArrayBuffer` ring을 사용하는 best-effort JavaScript lane입니다. native producer가 6-word event record를 쓰고 Bun이 event별 native-to-JS callback 없이 drain합니다.

```ts
const lane = new DynamicInputLane(1024);
const unsubscribe = lane.on(
  InputDevice.Keyboard,
  Key.Q,
  InputEdge.Down,
  (event) => console.log(event),
);
lane.drain();
unsubscribe();
```

```ts
const host = await NativeHost.load("macro.spellwire.ts");
host.attachDynamicLane(lane);
host.start();
lane.drain(1024);
host.detachDynamicLane();
```

각 event는 readonly snapshot입니다. dispatch 중 등록/해제는 다음 event부터 적용됩니다. 같은 lane에서 `drain()`을 reentrant 호출할 수 없습니다. 이 lane은 control plane이며 native realtime handler path가 아닙니다.

## Native state

```ts
const enabled = new NativeState<boolean>(1, "boolean", bridge);
enabled.set(false);
console.log(enabled.get());
```

`NativeHost`는 compiler manifest로 wrapper를 생성합니다.

```ts
const host = await NativeHost.load("macro.spellwire.ts");
host.start();
host.state("enabled").set(false);
console.log(host.states.phase?.get());
host.close();
```

`reload()`는 source를 다시 compile/load하고 이름과 kind가 같은 state를 보존합니다. `watch()`는 filesystem reload를 직렬화합니다. `.bin` input은 인접 `.json` 또는 `manifestPath`를 사용합니다.

## Native host와 권한

```ts
const host = await NativeHost.load("macro.spellwire.ts", {
  nativeLibraryPath: "/optional/explicit/library",
});
host.permissionStatus();
host.requestPermissions();
host.start();
await host.reload();
host.stop();
host.close();
```

| Member | Contract |
| --- | --- |
| `NativeHost.load(input, options?)` | `.ts` compile 또는 `.bin`+manifest load, ABI 검증, host 할당 |
| `permissionStatus()` | prompt 없이 observe/inject bitmask 반환 |
| `requestPermissions()` | macOS 요청, Windows/Linux 상태 재조회 |
| `start()` / `stop()` | observer, injector, worker, scheduler 시작/중지 |
| `reload({ preserveState? })` | reload 직렬화, 기본적으로 compatible name/kind state 보존 |
| `watch(options?)` | debounce와 callback을 지정해 file 감시 |
| `state(name)` / `states[name]` | 현재 manifest의 `NativeState` 접근 |
| `snapshotStates()` | native worker command 한 번으로 모든 named state 조회 |
| `attachDynamicLane(lane)` | observed input을 shared record로 publish |
| `dispatch(...)` | test/custom embedder용 명시 VM input |
| `close()` | 필요 시 stop, native host free, library close |

host는 package library, `SPELLWIRE_NATIVE_LIBRARY`, workspace release/debug build 순서로 탐색합니다. `close()`는 idempotent이고 stop은 추적 중인 synthetic held input을 해제합니다. 자세한 예제는 [라이브 네이티브 호스트](live-host.ko.md)를 참고하십시오.

`host.capabilities & NativeCapability.NativeInputSuppression`은 Windows/macOS에서 nonzero이고 현재 Linux backend에서는 0입니다.

## 통합 application lifecycle

`Spellwire.start(options)`는 host load, 자동 권한 준비, start, 선택적 watch, 상태 기반 overlay, 안전한 종료를 소유합니다. `options.overlay(state)`는 shallow named-state snapshot을 받습니다. `refreshOverlay()`는 수동 update boundary를 지원하고 `untilSignal()`은 host stop 전에 overlay를 종료합니다.

## Modern overlay

`Overlay.mount(tree, options?)`는 retained declarative tree를 mount합니다. `ui`는 `row`, `column`, `panel`, `stack`, `frame`, `box`, `text`, `ellipse`, `dot`, `divider`, `badge`, `spacer`, `bind`, `when`을 export합니다.

layout prop은 숫자/`"fill"` width·height, min/max dimension, padding, gap, alignment, justification, offset을 지원합니다. visual prop은 fill, stroke, radius, shadow, 상속 opacity, system/monospace family, font size/weight, line height, letter spacing, text alignment를 지원합니다.

`OverlayScene`과 `NativeOverlayRenderer`는 text/rect/ellipse/line primitive용 low-level retained API로 유지됩니다. pending change는 node별로 합쳐지고 `apply(scene)`는 batch 하나를 전송합니다. 상태 결합 예제와 속성 계약은 [상태 기반 네이티브 오버레이](overlay.ko.md)를 참고하십시오.

## ABI

C ABI는 owned platform host와 compatibility callback engine을 모두 포함합니다. [네이티브 C ABI](native-abi.ko.md)를 참고하십시오.
