# 네이티브 오버레이

[English](overlay.md)

Spellwire는 retained scene model과 별도 native renderer process를 제공합니다. input dispatch는 renderer를 기다리지 않고 JavaScript를 호출하지 않습니다.

## 사용 예

```ts
import { NativeOverlayRenderer, OverlayScene } from "spellwire";

const scene = new OverlayScene();
const panel = scene.create({
  kind: "rect",
  x: 20,
  y: 20,
  width: 260,
  height: 72,
  radius: 14,
  color: "#121216cc",
});
const label = scene.create({
  kind: "text",
  x: 42,
  y: 42,
  text: "42 µs",
  size: 20,
  color: "#ffffffff",
});

const renderer = await NativeOverlayRenderer.start();
await renderer.apply(scene);

scene.update(label, { kind: "text", x: 42, y: 42, text: "38 µs", size: 20 });
await renderer.apply(scene);

scene.remove(panel);
await renderer.apply(scene);
await renderer.close();
```

node는 선택적 `#RRGGBB` 또는 `#RRGGBBAA` color를 지원합니다.

```ts
type OverlayNode =
  | { kind: "text"; x: number; y: number; text: string; size: number; color?: string }
  | { kind: "rect"; x: number; y: number; width: number; height: number; radius: number; color?: string }
  | { kind: "line"; x1: number; y1: number; x2: number; y2: number; width: number; color?: string };
```

`create`, `update`, `remove`는 단조 증가 revision의 mutation을 추가합니다. `apply()`는 pending mutation만 drain하므로 정적 scene에는 JavaScript per-frame callback이 없습니다.

## Process protocol과 격리

`NativeOverlayRenderer`는 npm platform directory, `SPELLWIRE_OVERLAY_EXECUTABLE`, workspace release/debug build에서 `spellwire-overlay`를 찾습니다. piped stdin으로 시작하고 JSON `ready` message를 기다립니다. newline command는 `upsert`, `remove`, `clear`, `show`, `hide`, `exit`입니다.

native process는 다음을 소유합니다.

- main-thread winit event loop와 wgpu surface
- native ordered map의 retained node
- mutation 뒤에만 수행하는 text/rect/line rasterization
- premultiplied RGBA upload와 transparent surface present
- topmost/click-through window request

input observer/runtime worker와 독립적이므로 renderer failure가 input execution을 중지하지 않습니다. 현재 primary monitor 하나를 덮으며 multi-monitor scene routing은 아직 공개하지 않습니다.

## Smoke test

```bash
bun run build:native
target/release/spellwire-overlay --smoke
```

성공하면 surface dimension과 alpha mode를 담은 `ready` JSON 하나를 출력하고 종료합니다. Linux는 graphical session이 필요하며 지원 compositor마다 반복해야 합니다.

## 성능 범위

overlay 작업은 realtime worker에서 분리되어 있지만 universal compositor latency를 주장하지 않습니다. release profiling은 idle CPU/RSS, mutation publish time, 대표 primitive 수의 render percentile, overlay on/off input p99를 각각 보고해야 합니다.
