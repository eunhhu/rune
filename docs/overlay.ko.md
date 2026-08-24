# 상태 기반 네이티브 오버레이

[English](overlay.md)

Spellwire의 modern overlay API는 native state, Figma식 auto layout, retained diff, 안전한 lifecycle 관리를 하나로 합칩니다. 절대 좌표 계산이나 수동 `scene.update()` / `renderer.apply()` loop가 필요 없습니다.

## 상태 + 오버레이 전체 코드

```ts
import { fileURLToPath } from "node:url";
import { Spellwire, ui } from "spellwire";

const app = await Spellwire.start({
  input: fileURLToPath(new URL("./main.spellwire.ts", import.meta.url)),
  watch: true,
  overlay: (state) => {
    const enabled = state.enabled === true;
    return ui.column(
      {
        x: 24, y: 48, width: 300, padding: 16, gap: 12,
        fill: "#111827ee", radius: 16, stroke: "#ffffff24",
        shadow: { fill: "#00000066", y: 8, blur: 24 },
      },
      ui.text("SPELLWIRE", {
        fill: "#94a3b8ff", fontSize: 12, fontWeight: 700, letterSpacing: 1,
      }),
      ui.row(
        { width: "fill", gap: 8, align: "center" },
        ui.dot({ size: 8, fill: enabled ? "#34d399ff" : "#fb7185ff" }),
        ui.text(enabled ? "Active" : "Paused", {
          width: "fill", fontSize: 16, fontWeight: 600,
        }),
        ui.badge("F8"),
      ),
      ui.divider(),
      ui.row(
        { width: "fill", justify: "space-between" },
        ui.text("Activations", { fill: "#94a3b8ff" }),
        ui.text(String(state.activations ?? 0), { fontFamily: "monospace" }),
      ),
    );
  },
});

await app.untilSignal();
```

`Spellwire.start()`가 native host, 권한 요청, 선택적 source watcher, overlay process, state binding, 종료를 모두 소유합니다. `untilSignal()`은 `SIGINT`/`SIGTERM`을 처리하고 renderer 종료, host 정지, 눌린 합성 입력 해제까지 수행합니다.

실행 가능한 저장소 예제는 [`examples/state-overlay.ts`](../examples/state-overlay.ts)입니다.

## Native state가 화면에 반영되는 과정

1. realtime handler가 persistent native state slot을 변경합니다. input dispatch에서 JavaScript를 호출하지 않습니다.
2. overlay controller가 설정된 control-plane 주기(기본 30 Hz)에 `spellwire_host_state_snapshot` FFI command 한 번으로 모든 named state를 읽습니다.
3. shallow state snapshot이 같으면 layout과 renderer IPC를 모두 생략합니다.
4. 변경되면 `overlay(state)`가 가벼운 element tree를 반환합니다.
5. stable key/path가 retained primitive와 tree를 reconcile합니다. 같은 primitive는 mutation을 만들지 않습니다.
6. node별 pending mutation을 합치고 하나의 native batch로 전송합니다.
7. renderer는 old/new 영향 영역의 합집합만 clear/rasterize하고 정렬된 texture 영역만 GPU로 upload한 뒤 retained texture를 present합니다.

이 경로는 realtime input callback과 분리됩니다. 정적 overlay는 JavaScript timer, per-frame callback, 반복 IPC가 모두 0입니다.

알고 있는 control-plane 경계에서만 직접 갱신하려면 polling을 끌 수 있습니다.

```ts
const app = await Spellwire.start({
  overlayOptions: { fps: 0 },
  overlay: (state) => ui.text(String(state.activations ?? 0)),
});

await app.refreshOverlay();
```

## UI API 목록

| API | 용도 |
|---|---|
| `ui.row(props, ...children)` | 가로 auto layout |
| `ui.column(props, ...children)` / `ui.panel(...)` | 세로 auto layout |
| `ui.stack(props, ...children)` / `ui.frame(...)` / `ui.box(...)` | layer 또는 frame 내부 절대 배치 |
| `ui.text(value, props)` | size, weight, line height, tracking, alignment를 가진 system/monospace text |
| `ui.ellipse(props)` / `ui.dot(props)` | ellipse와 상태 dot |
| `ui.divider(props)` | fill-width separator |
| `ui.badge(label, props)` | 내용 크기에 맞는 compact label surface |
| `ui.spacer(sizeOrProps)` | 고정 또는 fill layout 공간 |
| `ui.bind(source, render, options)` | `NativeState`, `NativeHost`, getter, custom readable source binding |
| `ui.when(source, content, fallback)` | 조건부 retained subtree |

child에는 중첩 배열, `false`, `null`, `undefined`를 넣을 수 있어 일반 TypeScript 조건식을 그대로 사용할 수 있습니다.

## Layout 속성

frame은 Figma auto layout에 필요한 최소 modern layout vocabulary를 지원합니다.

```ts
ui.row({
  x: 24,
  y: 48,
  width: 320,          // number 또는 "fill"
  height: "fill",
  minWidth: 200,
  maxWidth: 480,
  padding: { x: 16, y: 12 },
  gap: 8,
  align: "center",     // start | center | end | stretch
  justify: "space-between", // start | center | end | space-between
});
```

- 숫자 dimension은 overlay logical pixel입니다. renderer가 process boundary에서 monitor scale factor를 한 번 적용하므로 Retina에서도 width 300이 150pt로 축소되지 않고 300pt를 유지합니다.
- `"fill"`은 해당 auto-layout 축의 남은 공간을 사용합니다.
- 생략한 dimension은 content를 hug합니다.
- `padding`은 숫자 하나 또는 `{ x, y, top, right, bottom, left }`이며 side-specific 값이 우선합니다.
- `row`/`column`은 flow layout입니다. `stack`은 같은 padded origin에 child를 쌓고 child `x`/`y` offset을 반영합니다.
- 조건부 삽입이나 sibling 재정렬에는 `key`를 사용해 identity를 유지합니다.

## Visual 속성

frame과 ellipse:

```ts
{
  fill: "#111827ee",
  stroke: { fill: "#ffffff30", width: 1 }, // color string이면 1 px
  radius: 16,                              // frame
  shadow: { fill: "#00000066", x: 0, y: 8, blur: 24, spread: 0 },
  opacity: 0.96,
}
```

text:

```ts
{
  fill: "#ffffffff",
  opacity: 1,
  fontFamily: "system",     // system | monospace
  fontSize: 16,
  fontWeight: 600,
  lineHeight: 20,
  letterSpacing: 0.2,
  textAlign: "left",        // left | center | right
}
```

color는 `#RRGGBB` 또는 `#RRGGBBAA`입니다. parent opacity는 descendant에 곱해집니다. 중요한 상태는 color만으로 표현하지 말고 text나 shape 변화도 함께 사용하십시오.

## `Spellwire.start()` 없이 직접 binding

`ui.bind`는 단일 `NativeState`, `NativeHost` snapshot source, getter, `get()`/`snapshotStates()` 구현 object를 받습니다.

```ts
const overlay = await Overlay.mount(
  ui.column(
    { padding: 12, fill: "#111827ee" },
    ui.bind(host.states.enabled, (enabled) =>
      ui.text(enabled ? "Enabled" : "Paused"),
    ),
  ),
);
```

같은 source의 여러 binding은 reconciliation pass당 한 번만 읽습니다. 여러 state를 표시할 때는 host 전체를 한 번 bind하는 편이 좋습니다. state마다 FFI를 호출하지 않고 native bulk snapshot 한 번을 사용합니다.

`OverlayMountOptions.fps`는 0–240이며 0은 manual refresh입니다. `executablePath`와 `readyTimeoutMs`는 native startup을 제어하고 `onError`는 asynchronous refresh failure를 처리합니다.

## Low-level retained escape hatch

다른 layout engine에서 최종 좌표를 이미 계산하는 경우 `OverlayScene`과 `NativeOverlayRenderer`를 사용할 수 있습니다. primitive node는 `text`, `rect`, `ellipse`, `line`입니다. `create`, `update`, `remove`는 node별 pending change를 합치며 `apply(scene)`는 batch 하나를 전송합니다. 같은 내용의 `update`는 no-op입니다.

일반 application UI는 `Overlay.mount()` 또는 `Spellwire.start()`를 사용하십시오.

## 성능 계약

- realtime input dispatch에서 JavaScript 호출 0
- state와 scene이 같을 때 layout, IPC, renderer 작업 0
- named-state 수와 무관하게 poll당 bulk state snapshot 1회, 값이 같으면 frozen JS snapshot 재사용
- refresh가 interval보다 오래 걸려도 polling tick을 합쳐 backlog 생성 방지
- unique source당 binding read 1회
- JSON hashing 없는 keyed primitive equality check
- update당 coalesced IPC batch와 native redraw 각 1회
- dirty-region CPU raster + 256-byte row alignment partial GPU upload
- 별도 renderer process와 main-thread winit/wgpu surface
- idle 동안 `ControlFlow::Wait`

반복 가능한 control-plane benchmark:

```bash
bun run bench:overlay
```

개발 macOS arm64에서 26 primitive state-bound panel과 20,000개 변경 snapshot을 단독으로 3회 측정한 결과 reconciliation + mutation publication은 71–72 µs p50, 90–92 µs p95, 213–220 µs p99였습니다. 이는 local baseline이며 universal compositor latency 주장이 아닙니다. target machine 검증에서는 native presentation latency, idle CPU/RSS, overlay off/on input p99도 기록해야 합니다.

## 현재 경계

- overlay는 click-through이며 interactive control은 지원하지 않습니다.
- system/monospace font family를 지원합니다. 임의 font file loading은 아직 public API가 아닙니다.
- primary monitor를 사용합니다. multi-monitor routing은 아직 public API가 아닙니다.
- image, arbitrary vector path, clipping, animation은 아직 public API가 아닙니다.
- Linux는 active graphical session과 compositor별 smoke test가 필요합니다.

## Smoke test

```bash
bun run build:native
target/release/spellwire-overlay --smoke
bun run test:overlay-live
```

executable smoke는 physical surface dimension, monitor scale factor, alpha mode를 포함한 `ready` JSON을 출력합니다. live smoke는 실제 host 시작, named state write, bulk snapshot, retained text node 두 개 update, clean shutdown까지 추가로 검증합니다.
