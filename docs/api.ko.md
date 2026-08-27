# Spellwire API — 한 페이지 레퍼런스

[English](api.md)

일반 Spellwire 앱에 필요한 명령, 실시간 자동화, 영속 상태, 앱 수명 주기, overlay UI를 이 페이지 하나에서 찾을 수 있습니다. 다른 문서는 내부 구조, 플랫폼 검증, 설계 이유를 설명하는 선택 자료입니다. 일상 API를 찾기 위해 페이지를 오갈 필요가 없습니다.

현재 source tree에 실제로 존재하는 API만 적습니다. 예전 설계 초안의 `macro(...)`, `spellwire.start()`, `rt.load(...)`, `on.keyDown(...)`, `key.tap(...)`은 export하지 않습니다.

## 이 페이지에서 바로 찾기

| 하고 싶은 일 | 사용할 API | 위치 |
| --- | --- | --- |
| 프로젝트 생성 | `bun create spellwire my-automation` | [생성, 실행, 감시, 빌드](#생성-실행-감시-빌드) |
| 입력을 차단하는 chord 등록 | `rt.hotkey("Ctrl+K", handler)` | [실시간 handler 등록](#실시간-handler-등록) |
| 키 하나 remap | `rt.remap("CapsLock", "Escape")` | [실시간 handler 등록](#실시간-handler-등록) |
| native 상태 유지 | module-scope `let enabled = true` | [영속 realtime 상태](#영속-realtime-상태) |
| 키보드/마우스 출력 | `tapKey`, `clickMouse`, `moveMouse` | [출력 및 held intrinsic](#출력-및-held-intrinsic) |
| handler 지연 | `sleep.ms(250)` 또는 `sleep.seconds(2)` | [출력 및 held intrinsic](#출력-및-held-intrinsic) |
| 입력, watch, UI 함께 시작 | `Spellwire.start(options)` | [통합 앱 수명 주기](#통합-application-lifecycle) |
| 상태를 overlay에 표시 | `overlay: state => ui.text(...)` | [Modern overlay](#modern-overlay) |
| row, column, panel, stack 구성 | `ui.row`, `ui.column`, `ui.panel` | [UI 생성 함수](#ui-생성-함수) |
| 크기, padding, gap, fill, border, shadow, font 설정 | element prop | [Layout과 visual 속성](#layout과-visual-속성) |
| UI 일부만 state binding | `ui.bind(state, render)` | [Binding과 refresh](#binding과-refresh) |
| UI 표시, 숨김, 수동 갱신 | `overlay.show()`, `hide()`, `app.refreshOverlay()` | [Overlay 수명 주기](#overlay-수명-주기) |
| topmost/transparency/focus 설정 | `overlayOptions.window` | [Window 동작](#window-동작) |
| native host 직접 제어 | `NativeHost` | [Native host와 권한](#native-host와-권한) |

## 바로 실행 가능한 전체 앱

생성 프로젝트는 `src/main.ts` 하나를 제공합니다. compiler는 realtime handler만 native bytecode로 추출하고 같은 파일의 제한 없는 application/overlay 코드는 Bun에 둡니다. authoring은 하나로 합치면서 input event path에는 JavaScript를 넣지 않습니다.

```ts
import { Key, Spellwire, rt, tapKey, ui } from "spellwire";

let enabled = true;
let presses = 0;

rt.hotkey("Q", () => {
  presses += 1;
  tapKey(Key.E);
}, { when: () => enabled });

rt.hotkey("F8", () => {
  enabled = !enabled;
}, { consume: false });

rt.remap("CapsLock", "Escape", { when: () => enabled });

const app = await Spellwire.start({
  input: import.meta.file,
  watch: Bun.argv.includes("--watch"),
  overlayOptions: { window: { title: "Spellwire Status" } },
  overlay: (state) => {
    const enabled = state.enabled === true;
    return ui.column(
      {
        x: 24,
        y: 48,
        width: 280,
        padding: 16,
        gap: 12,
        fill: "#111827ee",
        radius: 16,
        stroke: "#ffffff24",
        shadow: { fill: "#00000066", y: 8, blur: 24 },
      },
      ui.row(
        { width: "fill", gap: 8, align: "center" },
        ui.dot({ size: 8, fill: enabled ? "#34d399ff" : "#fb7185ff" }),
        ui.text(enabled ? "Active" : "Paused", {
          width: "fill",
          fill: "#ffffffff",
          fontSize: 16,
          fontWeight: 600,
        }),
        ui.badge("F8"),
      ),
      ui.text(`Q presses: ${String(state.presses ?? 0)}`, {
        fill: "#cbd5e1ff",
        fontFamily: "monospace",
        fontSize: 13,
      }),
    );
  },
});

await app.untilSignal();
```

## 생성, 실행, 감시, 빌드

```bash
bun create spellwire my-automation
cd my-automation
bun run start
bun run watch
bun run build
```

| 명령 | 동작 |
| --- | --- |
| `bun run start` | source를 메모리에서 compile하고 권한을 준비한 뒤 바로 실행 |
| `bun run watch` | 실행하면서 source 변경 후 native program reload |
| `bun run build` | `dist/main.spellwire.bin`과 named-state manifest 생성 |

직접 CLI를 사용할 때의 같은 명령:

```bash
bunx spellwire run [macro.spellwire.ts]
bunx spellwire watch [macro.spellwire.ts]
bunx spellwire compile macro.spellwire.ts [output.spellwire.bin]
```

입력을 생략하면 `src/main.ts`, 없으면 legacy `src/main.spellwire.ts`를 사용합니다. CLI `run`/`watch`는 realtime native host만 실행하고 생성 프로젝트의 `bun run start`/`watch`는 overlay를 포함한 통합 application을 실행합니다. 둘 다 realtime handler를 메모리에서 compile하고 권한을 한 번 준비합니다. watch mode는 control-plane file reload만 추가합니다.

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
  sleep,
  sleepHours,
  sleepMinutes,
  sleepMs,
  sleepSeconds,
  sleepUs,
  tapKey,
  ui,
  wheelMouse,
} from "spellwire";
```

일반 앱에는 `Spellwire`, `ui`, `rt`, key/button, 출력 intrinsic이면 충분합니다. `NativeHost`, low-level overlay class, compiler helper, fallback helper는 고급 escape hatch입니다.

compiler API:

```ts
import { compileSource, encodeModule } from "spellwire/compiler";
```

## 영속 realtime 상태

safe integer 또는 boolean으로 초기화한 module-scope `let`을 realtime handler가 참조하면 영속 native `i64` state slot이 됩니다.

```ts
let enabled = true;
let count = 0;

rt.hotkey("F8", () => {
  enabled = !enabled;
  count += 1;
});
```

대입, compound 대입, `++`/`--`, 정수 산술, 비교, boolean logic, bit 연산, 조건, 제한 반복은 native VM opcode로 compile됩니다. realtime dispatch에는 state 이름 lookup, JavaScript object, FFI가 없습니다. `count++`, `count += 4`, `mask ^= 1`, `enabled = !enabled`, constant store 같은 흔한 discarded update는 VM stack 왕복 없이 slot/immediate opcode 하나로 바로 compile됩니다. module-scope `const`는 정적으로 표현할 수 있으면 fold됩니다.

상태는 dispatch 사이에 유지됩니다. source reload는 `preserveState: false`가 아니면 같은 이름과 kind의 값을 보존합니다. `when`은 native suppression table도 dispatch 전에 같은 gate를 평가해야 하므로 module-scope boolean 하나 또는 그 부정만 받습니다.

realtime handler 밖에서는 `app.host.state("name")`, `app.host.states.name`, 또는 bulk `app.host.snapshotStates()`를 사용합니다. 이 경로는 control-plane FFI이며 realtime opcode가 아닙니다.

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

Chord 문법은 `+`로 구분한 trigger 하나와 logical modifier 0개 이상입니다.

- modifier: `Ctrl`/`Control`, `Shift`, `Alt`/`Option`, `Meta`/`Cmd`/`Command`/`Win`/`Super`
- trigger: export된 모든 `Key` member 이름, `A`–`Z`, `0`–`9`, `Esc`, `Return`, `PgUp`, `PgDn`, `Spacebar` 같은 alias, 또는 `LButton`, `RButton`, `MButton`, `XButton1`, `XButton2`
- 이름은 대소문자, 공백, `_`, `-`를 무시
- modifier가 아닌 trigger는 정확히 하나여야 하며 `A+B` 같은 조합은 거부

```ts
rt.hotkey("Cmd+Space", handler);
rt.hotkey("Ctrl+Alt+K", handler);
rt.hotkey("Shift+LButton", handler);
```

### `rt.remap(from, to, options?)`

key down/up handler를 한 쌍으로 compile하고 활성 source sequence를 consume합니다.

```ts
rt.remap("CapsLock", "Escape", { when: () => enabled });
rt.remap(Key.CapsLock, Key.Escape, { repeat: false });
```

두 key는 portable 문자열 이름 하나 또는 `Key` 값입니다. option은 `source`, `repeat`, `when`입니다.

### Low-level 등록

```text
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

특수 release/suppression case와 AutoHotkey migration matrix가 필요할 때만 선택형 [자동화 의미론](automation.ko.md)을 참고하십시오.

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
sleep.us(80)
sleep.ms(250)
sleep.seconds(2)
sleep.minutes(1)
sleep.hours(1)
keyHeld(Key.LeftShift)
mouseHeld(MouseButton.Right)
```

zero-delay 출력은 고정 native output batch로 모입니다. live host에서 모든 `sleep.*()` helper는 batch를 flush하고 fixed-capacity absolute-deadline scheduler에 continuation을 양보합니다. compatibility engine과 simulator는 동기 대기합니다.

| API | Native 동작 |
| --- | --- |
| `keyDown(key)` / `keyUp(key)` | keyboard transition 하나 출력 |
| `tapKey(key)` | down/up transition 한 쌍 출력 |
| `mouseDown(button)` / `mouseUp(button)` | mouse button transition 하나 출력 |
| `clickMouse(button)` | mouse down/up 한 쌍 출력 |
| `moveMouse(dx, dy)` | 상대 pointer 이동 출력 |
| `wheelMouse(x, y)` | 수평/수직 wheel 이동 출력 |
| `sleep.us(duration)` / `sleepUs(duration)` | microsecond |
| `sleep.ms(duration)` / `sleepMs(duration)` | millisecond |
| `sleep.seconds(duration)` / `sleepSeconds(duration)` | second |
| `sleep.minutes(duration)` / `sleepMinutes(duration)` | minute |
| `sleep.hours(duration)` / `sleepHours(duration)` | hour |

namespace 형태가 가장 짧고 named helper는 직접 import할 때 편합니다. constant duration은 compile time에 scale합니다. dynamic duration도 같은 `DelayUs` instruction immediate에 unit scale을 담으므로 helper call은 native delay opcode 하나입니다. realtime path에 conversion callback, 추가 timer, JavaScript 작업이 없습니다. encoded microsecond는 non-negative signed 64-bit이고 platform이 표현할 수 없는 deadline은 wrap하지 않고 오류가 됩니다. JavaScript fallback은 input과 변환 결과에 safe integer도 요구합니다.

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

일반 application은 이 API를 사용하십시오.

```ts
const app = await Spellwire.start({
  input: import.meta.file,
  watch: true,
  overlay: (state) => ui.text(String(state.count ?? 0)),
});

await app.untilSignal();
```

`Spellwire.start(options)`가 host load, 권한 준비, native input start, 선택적 file watch, 상태 기반 overlay, 안전 종료를 모두 소유합니다.

| `SpellwireStartOptions` | 기본값 | 계약 |
| --- | --- | --- |
| `input` | `"src/main.ts"`, 이후 legacy source | realtime TypeScript 또는 compiled `.bin` 경로 |
| `watch` | `false` | input source를 감시하고 native program reload |
| `debounceMs` | host 기본값 | file reload debounce |
| `preserveState` | `true` | reload 때 compatible named state 보존 |
| `requestPermissions` | `true` | 시작 전에 observe/inject 권한 확인·요청 |
| `onReload` | — | watch reload 성공 후 호출 |
| `onError` | console/기본 전파 | watch 또는 asynchronous overlay failure 수신 |
| `overlay(state)` | — | shallow named-state snapshot으로 overlay 생성 |
| `overlayOptions` | — | 아래 polling, renderer startup, native window option |
| `nativeLibraryPath` | 자동 탐색 | native library 직접 지정 |
| `manifestPath` | 인접 manifest | compiled binary의 manifest 직접 지정 |

| App member | 계약 |
| --- | --- |
| `app.host` | 시작된 `NativeHost`; `states`, `reload`, snapshot 포함 |
| `app.overlay` | mount된 `Overlay`; callback이 없으면 `undefined` |
| `app.refreshOverlay()` | binding read/reconciliation 한 번 강제; `fps: 0`에서 사용 |
| `app.untilSignal()` | `SIGINT`/`SIGTERM`을 기다린 뒤 안전 종료 |
| `app.close()` | watcher, renderer, host를 닫고 추적 중인 synthetic input 해제 |

## Modern overlay

`Spellwire.start({ overlay })`가 가장 짧은 state-to-screen 경로입니다. `Overlay.mount(tree, options?)`는 standalone API입니다. 둘 다 native retained renderer를 사용하며 DOM, WebView, React, per-frame JavaScript drawing callback이 없습니다.

### UI 생성 함수

| API | 결과 |
| --- | --- |
| `ui.row(props, ...children)` | 수평 auto layout |
| `ui.column(props, ...children)` | 수직 auto layout |
| `ui.panel(props, ...children)` | 수직 frame의 semantic alias |
| `ui.stack(props, ...children)` | 같은 padded origin에 child layer |
| `ui.box(...)` / `ui.frame(...)` | `ui.stack(...)` alias |
| `ui.text(value, props?)` | text primitive |
| `ui.ellipse(props?)` | ellipse primitive |
| `ui.dot({ size, ...props })` | 같은 width/height ellipse 편의 함수 |
| `ui.divider(props?)` | 기본 1 px, fill-width divider |
| `ui.badge(label, props?)` | style된 frame+text 편의 함수 |
| `ui.spacer(sizeOrProps?)` | 빈 layout 공간 |
| `ui.bind(source, render, options?)` | cache되는 state-bound subtree |
| `ui.when(source, content, fallback?)` | 조건부 state-bound subtree |

child에는 중첩 배열, `false`, `null`, `undefined`를 넣을 수 있습니다. 조건부 삽입이나 sibling 재정렬에서 identity를 유지하려면 `key`를 사용하십시오.

### Layout과 visual 속성

frame, text, ellipse, spacer에 공통으로 적용되는 layout prop:

| Prop | Type | 의미 |
| --- | --- | --- |
| `key` | `string` | 안정 reconciliation identity |
| `x`, `y` | `number` | logical-pixel offset |
| `width`, `height` | `number \| "fill"` | 고정 크기 또는 parent 남은 공간; 생략하면 content hug |
| `minWidth`, `minHeight` | `number` | 최소 measured size |
| `maxWidth`, `maxHeight` | `number` | 최대 measured size |
| `opacity` | `number` | element opacity; parent 값은 descendant에 곱해짐 |

frame 전용 prop:

| Prop | Type / 값 |
| --- | --- |
| `padding` | `number` 또는 `{ x?, y?, top?, right?, bottom?, left? }`; side 값 우선 |
| `gap` | flow child 사이 logical pixel |
| `align` | `"start" \| "center" \| "end" \| "stretch"` |
| `justify` | `"start" \| "center" \| "end" \| "space-between"` |
| `fill` | `#RRGGBB` 또는 `#RRGGBBAA` |
| `radius` | logical-pixel corner radius |
| `stroke` | 1 px color string 또는 `{ fill, width }` |
| `shadow` | `{ fill, x?, y?, blur?, spread? }` |

text 전용 prop:

| Prop | Type / 값 |
| --- | --- |
| `fill` | text color, `#RRGGBB` 또는 `#RRGGBBAA` |
| `fontFamily` | `"system" \| "monospace"` |
| `fontSize`, `fontWeight`, `lineHeight`, `letterSpacing` | `number` |
| `textAlign` | `"left" \| "center" \| "right"` |

ellipse는 `fill`, `stroke`, `shadow`를 지원합니다. `ui.dot`은 `size`를 추가합니다. `ui.badge`는 모든 frame prop과 함께 `textFill`, `fontFamily`, `fontSize`, `fontWeight`를 받습니다.

modern style vocabulary 전체 예제:

```ts
ui.row(
  {
    key: "status",
    x: 24,
    y: 48,
    width: 320,
    minHeight: 56,
    padding: { x: 16, y: 12 },
    gap: 10,
    align: "center",
    justify: "space-between",
    fill: "#111827ee",
    radius: 16,
    stroke: { fill: "#ffffff30", width: 1 },
    shadow: { fill: "#00000066", y: 8, blur: 24 },
    opacity: 0.96,
  },
  ui.text("Active", {
    width: "fill",
    fill: "#ffffffff",
    fontFamily: "system",
    fontSize: 16,
    fontWeight: 600,
    lineHeight: 20,
    letterSpacing: 0.2,
  }),
  ui.badge("F8"),
);
```

### Binding과 refresh

`ui.bind`는 `NativeState`, `NativeHost`, getter 함수, `get()`/`snapshotStates()` 구현 object를 받습니다.

```ts
const overlay = await Overlay.mount(
  ui.column(
    { padding: 12, gap: 8, fill: "#111827ee" },
    ui.bind(host.states.enabled, (enabled) =>
      ui.text(enabled ? "Enabled" : "Paused"),
    ),
    ui.bind(host.states.count, (count) => ui.text(`Count: ${count}`)),
  ),
);
```

`ui.bind`는 기본 shallow equality를 사용하며 `options.equals(left, right)`로 교체할 수 있습니다. reconciliation pass마다 unique source를 한 번 읽습니다. 값이 같으면 layout과 renderer IPC가 모두 없습니다. binding이 바뀌면 해당 render callback만 다시 실행하고, resolved tree를 layout한 뒤 keyed primitive를 diff합니다. 바뀐 primitive만 process boundary를 넘고 native renderer는 영향 영역만 다시 그립니다.

`Spellwire.start({ overlay })`는 bulk host snapshot 하나를 root callback에 bind합니다. named state 하나라도 바뀌면 root callback을 다시 실행합니다. React식 render/reconcile에 가깝고 자동 signal dependency tracking은 아닙니다. callback 단위 granularity가 필요하면 state별 `ui.bind`를 사용하십시오.

### Overlay 수명 주기

```ts
const overlay = await Overlay.mount(tree, {
  fps: 30,
  executablePath: "/optional/spellwire-overlay",
  readyTimeoutMs: 5_000,
  window: { title: "Status", alwaysOnTop: true, clickThrough: true },
  onError: console.error,
});

await overlay.set(nextTree);
await overlay.refresh();
await overlay.hide();
await overlay.show();
await overlay.close();
```

| `OverlayMountOptions` | 기본값 | 계약 |
| --- | --- | --- |
| `fps` | `30` | 초당 binding poll, `0`은 manual refresh; 0–240 |
| `executablePath` | 자동 탐색 | native renderer 직접 지정 |
| `readyTimeoutMs` | `5_000` | renderer startup timeout |
| `window` | overlay용 안전 기본값 | native window 정책; 아래 표 참고 |
| `onError` | console | asynchronous refresh error callback |
| `renderer` | 새로 생성 | 기존 `NativeOverlayRenderer` 재사용; `SpellwireStartOptions.overlayOptions`에서는 사용 불가 |

정적 tree는 timer를 만들지 않습니다. refresh 진행 중 poll tick은 합쳐지므로 느린 update가 backlog를 만들지 않습니다.

### Window 동작

`OverlayMountOptions.window`, `SpellwireStartOptions.overlayOptions.window`, low-level `NativeOverlayRenderer.start({ window })`가 같은 정책을 사용합니다.

| `OverlayWindowOptions` | 기본값 | Native 요청 |
| --- | --- | --- |
| `title` | `"Spellwire Overlay"` | 1–256자 window title |
| `transparent` | `true` | alpha surface와 transparent window |
| `alwaysOnTop` | `true` | always-on-top window level |
| `focusable` | `false` | overlay activation/focus 허용 여부 |
| `clickThrough` | `true` | true이면 pointer hit testing 비활성 |
| `decorations` | `false` | native title bar/border |
| `resizable` | `false` | user resize 허용 |
| `visible` | `true` | 초기 표시; 이후 `show()` / `hide()` 가능 |

```ts
const app = await Spellwire.start({
  input: import.meta.file,
  overlayOptions: {
    window: {
      title: "Macro status",
      transparent: true,
      alwaysOnTop: true,
      focusable: false,
      clickThrough: true,
      decorations: false,
      resizable: false,
      visible: true,
    },
  },
  overlay: (state) => ui.text(String(state.enabled ?? false)),
});
```

ready message의 `overlay.renderer.ready.window`에서 validate 및 default resolve가 끝난 요청 정책을 확인할 수 있습니다. renderer는 native winit/wgpu process이며 WebView compatibility layer가 없습니다. `focusable`과 `clickThrough`는 별도입니다. focusable window도 pointer hit을 무시할 수 있고 non-focusable window도 pointer hit을 받을 수 있습니다. macOS는 non-focusable일 때 prohibited activation policy, focusable일 때 accessory policy를 사용하고 Windows는 non-focusable일 때 native window를 disable하며 Linux는 가능한 winit/compositor hint를 사용합니다. 모든 X11/Wayland compositor에서 focus/topmost가 완전히 같다고 보장할 수 없으므로 대상 Linux desktop에서 검증해야 합니다.

초기 monitor와 size는 아직 primary monitor 전체 영역입니다. monitor routing과 명시적 window geometry는 public API가 아닙니다. Windows에서는 `focusable: false`이면 window도 non-interactive입니다. interactive decorated tool window에는 `focusable: true`를 사용하십시오.

### Low-level retained overlay

`OverlayScene`과 `NativeOverlayRenderer`는 최종 좌표를 이미 계산하는 caller용 escape hatch입니다. primitive kind는 `text`, `rect`, `ellipse`, `line`입니다.

```ts
const renderer = await NativeOverlayRenderer.start();
const scene = new OverlayScene();
const id = scene.create({ kind: "text", x: 20, y: 20, text: "Ready", size: 16 });
await renderer.apply(scene);

scene.update(id, { kind: "text", x: 20, y: 20, text: "Running", size: 16 });
await renderer.apply(scene);

scene.remove(id);
await renderer.apply(scene);
await renderer.close();
```

pending change는 node별로 합쳐집니다. 같은 내용의 update는 no-op이며 `apply(scene)`는 mutation batch를 최대 하나 전송합니다.

## ABI

C ABI는 owned platform host와 compatibility callback engine을 모두 포함합니다. [네이티브 C ABI](native-abi.ko.md)를 참고하십시오.
