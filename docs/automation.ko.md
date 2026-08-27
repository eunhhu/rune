# Hotkey, remap, 상태 기반 자동화

[English](automation.md)

Spellwire realtime 입력 계층은 native latency와 작은 authoring surface를 목표로 합니다. modifier hotkey, release hotkey, 단일 키 remap, repeat 정책, 원본 입력 차단, boolean 상태 gate를 host 시작 전에 compile합니다. OS hook에서 JavaScript callback을 실행하지 않습니다.

> 아직 AutoHotkey 전체 대체재는 아닙니다. native hotstring, 임의의 비-modifier 조합, Unicode text 전송, window/control 자동화, clipboard/process helper, image/pixel search는 명시적인 미구현 영역입니다. [AutoHotkey 마이그레이션 상태](#autohotkey-마이그레이션-상태)를 먼저 확인하십시오.

## 작지만 완전한 예제

```ts
import { Key, rt, tapKey } from "spellwire";

let enabled = true;
let presses = 0;

rt.hotkey("Ctrl+Shift+K", () => {
  presses += 1;
  tapKey(Key.Enter);
}, {
  repeat: false,
  when: () => enabled,
});

rt.hotkey("F8", () => {
  enabled = !enabled;
}, { consume: false });

rt.remap("CapsLock", "Escape", { when: () => enabled });
```

이 source는 chord 1개, toggle 1개, remap down/up 2개를 합쳐 native handler 4개를 만듭니다. `enabled`와 `presses`는 영속 native state입니다. `enabled`가 false이면 `Ctrl+Shift+K`와 `CapsLock`을 차단하지 않고 현재 앱으로 통과시킵니다.

## `rt.hotkey()`

```ts
rt.hotkey(chord, handler, options?);
```

지원하는 portable 이름:

- modifier: `Ctrl`, `Control`, `Shift`, `Alt`, `Option`, `Meta`, `Cmd`, `Command`, `Win`, `Super`
- keyboard: export된 `Key` member와 `Esc`, `Return`, `PgUp`, `PgDn`, `Spacebar` 같은 alias
- mouse: `LButton`, `RButton`, `MButton`, `XButton1`, `XButton2`

대소문자, 공백, `_`, `-`는 구분하지 않습니다. chord에는 keyboard key 또는 mouse button이 정확히 하나 있고 나머지 token은 logical modifier입니다. 좌우 modifier는 같은 logical group을 충족합니다.

```ts
rt.hotkey("Cmd+Space", () => { /* macOS식 chord */ });
rt.hotkey("Ctrl+Alt+K", () => { /* portable name */ });
rt.hotkey("Shift+LButton", () => { /* mouse chord */ });
```

`A+B`처럼 modifier가 아닌 키 2개를 묶는 custom combination은 아직 지원하지 않습니다. compiler가 의미를 바꾸지 않고 명확히 거부합니다.

### Option

| Option | 기본값 | 동작 |
| --- | --- | --- |
| `source` | `InputSource.Physical` | physical, synthetic, any source 선택 |
| `consume` | `true` | 지원 backend에서 원본 down/repeat/up sequence 차단 |
| `exactModifiers` | `true` | 추가 modifier 거부; `false`이면 추가 modifier 허용 |
| `repeat` | `true` | OS repeat down 실행; `false`이면 첫 down만 실행 |
| `edge` | `"down"` | `"down"` 또는 `"up"`에서 실행 |
| `when` | 항상 활성 | native boolean state로 action과 차단을 함께 gate |

release hotkey는 대응하는 down에서 suppression과 modifier/`when` 수락 상태를 함께 latch합니다. modifier를 먼저 떼거나 key가 눌린 동안 gate가 바뀌어도, down을 정상 수락한 경우에만 paired up을 차단하고 handler를 실행합니다. 현재 앱에 down만 전달하고 up을 삼키거나 이미 수락한 release action을 잃는 문제를 피합니다.

## Native 상태 gate

`when`은 module-scope boolean `let` 하나 또는 그 부정을 반환하는 인자 없는 함수를 받습니다.

```ts
let gaming = false;

rt.hotkey("F6", () => {
  gaming = !gaming;
}, { consume: false });

rt.hotkey("Q", () => tapKey(Key.E), {
  when: () => gaming,
});

rt.remap("CapsLock", "Escape", {
  when: () => !gaming,
});
```

다음 코드는 의도적으로 compile error입니다.

```ts
const dynamicObject = { active: true };
rt.hotkey("Q", () => {}, { when: () => dynamicObject.active });
rt.hotkey("Q", () => {}, { when: () => Date.now() > 0 });
```

이 제한 덕분에 hook에서 JavaScript, window query, allocation, lock 없이 suppression을 결정합니다. handler 내부 `if`는 더 복잡한 action 조건에 사용할 수 있지만 trigger의 suppression 결정은 바꾸지 않습니다. 비활성 입력을 원래 앱으로 보내야 한다면 `when`을 사용하십시오.

## `rt.remap()`

```ts
rt.remap("CapsLock", "Escape");
rt.remap(Key.CapsLock, Key.Escape, { repeat: false });
```

source와 target은 keyboard 이름 하나 또는 `Key` 값을 받습니다. compiler는 native down/up output을 한 쌍으로 만들고, 활성 source sequence를 항상 consume합니다. `source`, `repeat`, `when` option을 지원합니다.

macOS의 물리 Caps Lock은 일반 key pair가 아니라 toggle형 `flagsChanged`로 도착합니다. Spellwire는 각 물리 Caps Lock activation을 native down/up pulse 하나로 정규화한 뒤 remap하여 target key가 눌린 채 남는 문제를 막습니다.

## 하나의 상태로 input과 overlay 함께 구동

생성 프로젝트는 `src/main.ts` authoring surface 하나를 제공합니다. AOT compiler는 realtime handler를 bounded native VM으로 추출하고 제한 없는 lifecycle/overlay code는 Bun에 둡니다. 두 execution plane은 input path에 JavaScript를 넣지 않은 채 같은 manifest 기반 native state를 사용합니다.

```ts
import { Key, Spellwire, rt, tapKey, ui } from "spellwire";

let enabled = true;
let presses = 0;

rt.hotkey("Q", () => {
  presses += 1;
  if (presses % 2 === 0) tapKey(Key.E);
}, { when: () => enabled });

rt.hotkey("F8", () => {
  enabled = !enabled;
}, { consume: false });

const app = await Spellwire.start({
  input: import.meta.file,
  watch: Bun.argv.includes("--watch"),
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

내부적으로 realtime execution과 제한 없는 application execution은 분리되지만 authoring file을 의무적으로 나누지 않습니다. `bun run start`, `watch`, `build`가 모두 같은 source를 사용합니다.

상태 update 경로:

```text
OS input
  → atomic consume lookup
  → bounded native queue
  → native VM이 enabled/presses 변경
  → overlay cadence마다 bulk state snapshot 1회
  → shallow binding 비교
  → keyed primitive diff
  → renderer batch 1회
  → dirty-region raster/upload
```

overlay는 기본 30 fps로 bound state를 확인합니다. snapshot이 같으면 이전 tree를 재사용하며 primitive mutation을 보내지 않습니다. `overlayOptions: { fps: 60 }`으로 바꾸거나, `fps: 0`과 `refreshOverlay()`로 수동 갱신하거나, overlay를 완전히 생략할 수 있습니다. 어느 선택도 OS hook 또는 native VM 경로에 작업을 추가하지 않습니다.

## 입력 차단과 성능 계약

Windows와 macOS는 `NativeInputSuppression`을 advertise합니다. Linux는 현재 observe/inject는 하지만 suppression capability를 advertise하지 않습니다. 따라서 Linux에서 `consume` handler는 실행되지만 원본 입력도 앱에 전달됩니다.

지원 backend의 hook path:

1. 고정 key/button translation
2. source-aware held/modifier bitmap update
3. atomic consume-table lookup 1회
4. 미리 할당한 고정 용량 SPSC queue publish 1회와 worker wake token

이 경로에는 JavaScript call, IPC, heap allocation, mutex, 상태식 평가, overlay 작업이 없습니다. queue slot은 host 시작 시 한 번만 할당합니다. 100,000-event test가 bounded overflow, wake-up, 순서, disconnect, ring-slot wraparound 재사용을 검증합니다. consume table은 참조 중인 `when` state가 바뀌거나 program reload가 성공할 때만 worker에서 갱신됩니다. input queue가 가득 차면 새 원본 event를 통과시키고 recovery를 atomic하게 표시한 뒤 stale backlog, pending continuation, input latch를 비우고 추적 중인 synthetic down을 전부 해제하도록 backend에 요청합니다. overload에서는 automation action이 유실될 수 있지만 수락하지 못한 event를 의도적으로 삼키지 않으며, backend release 실패는 `lastError()`에 남깁니다.

개발 macOS arm64 장비에서 warm 1,000,000-sample core run을 commit `3fe4256`의 detached baseline worktree와 각각 3회 비교했습니다. baseline과 이번 변경 모두 p50/p95 `42 ns`, p99 `42–83 ns`, p999 `84–125 ns`였습니다. benchmark clock resolution 안에서 측정 가능한 regression이 없다는 뜻이며 물리 switch-to-application latency 주장은 아닙니다.

## AutoHotkey 마이그레이션 상태

| AutoHotkey v2 영역 | Spellwire 상태 |
| --- | --- |
| Modifier keyboard/mouse hotkey | portable 문자열로 구현 |
| exact/wildcard modifier, repeat, key-up trigger | 구현 |
| 원본 입력 차단 | Windows/macOS 구현; Linux relay 미구현 |
| 단일 키 remap | 문자열 또는 `Key` 값으로 구현 |
| 상태 조건 hotkey/remap | native boolean gate 구현 |
| 영속 상태와 bounded realtime state machine | 구현 |
| modern native overlay/GUI styling | retained Figma식 layout으로 구현 |
| custom `A & B` combination | 미구현 |
| hotstring/text expansion | 미구현 |
| Unicode `SendText`/layout-aware text injection | 미구현 |
| active window/process/control predicate | 미구현 |
| clipboard, process launch, timer, dialog | 안정 public helper 미구현 |
| image/pixel search와 control 자동화 | 미구현 |

동일한 출발 예제:

```ahk
; AutoHotkey v2
^+k::Send "{Enter}"
CapsLock::Escape
```

```ts
// Spellwire
rt.hotkey("Ctrl+Shift+K", () => tapKey(Key.Enter));
rt.remap("CapsLock", "Escape");
```

표의 미구현 행과 대상 장비 검증이 끝나기 전에는 Spellwire를 AutoHotkey superset이라고 설명하면 안 됩니다. 목표 architecture는 더 넓습니다. portable TypeScript control plane, bounded native realtime plane, retained native UI를 제공하면서 input hook에 general-purpose runtime 작업을 넣지 않는 것입니다.

AutoHotkey primary reference: [Hotkeys](https://www.autohotkey.com/docs/v2/Hotkeys.htm), [Hotstrings](https://www.autohotkey.com/docs/v2/Hotstrings.htm), [#HotIf](https://www.autohotkey.com/docs/v2/lib/_HotIf.htm), [Send](https://www.autohotkey.com/docs/v2/lib/Send.htm).

## 검증

macOS 권한 설정 후:

```bash
bun run build:native
bun run test:consume-macos
bun run test:platform-loopback
bun run check
```

consume smoke는 먼저 CoreGraphics tail probe가 차단되지 않은 transition 2개를 보는지 확인합니다. 그다음 비활성 `when` gate에서도 2개가 통과하는지 검증합니다. native gate를 활성화하면 VM handler는 1회 실행되고 tail probe에는 transition 0개가 도착해야 합니다.

Windows와 Linux 대상 장비 결과는 [플랫폼 검증](platform-verification.ko.md)에 따라 기록하십시오.
