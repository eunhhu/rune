# Spellwire

[English](README.md)

Spellwire는 Bun과 TypeScript를 사용하는 상태 기반 실시간 입력 자동화 런타임입니다. 분석 가능한 입력 핸들러를 입력 이벤트마다 JavaScript로 호출하지 않고, 미리 제한된 네이티브 바이트코드로 컴파일하여 실행합니다.

> 초기 알파 버전입니다. TypeScript AOT 컴파일러, 제한형 네이티브 VM, lock-free 입력 차단 hotkey/remap, 상태 gate, 비차단 지연 스케줄러, Bun FFI 호스트, Windows/macOS/Linux 입력 백엔드, 공유 동적 입력 lane, retained-mode 네이티브 오버레이가 구현되어 있습니다. macOS는 실제 루프백과 입력 차단 검증을 마쳤습니다. Windows는 대화형 세션에서 루프백, 입력 주입, reload, overlay lifecycle을 검증했으며 물리 입력 차단과 시각적 투명도 검증이 남아 있습니다. Linux는 대상 장비 검증이 남아 있습니다.

## 가장 빠르게 시작하기

```bash
bun create spellwire my-automation
cd my-automation
bun run start
```

생성된 프로젝트는 다음 세 명령을 사용합니다.

```bash
bun run start  # 메모리에서 컴파일한 뒤 바로 실행
bun run watch  # 실행하면서 네이티브 hot reload
bun run build  # dist/main.spellwire.bin과 manifest 생성
```

`start`와 `watch`는 네이티브 호스트를 시작하기 전에 플랫폼 권한을 자동으로 확인하고 요청합니다.

생성 프로젝트는 realtime handler와 상태 기반 modern overlay를 `src/main.ts` 하나에 둡니다. compiler는 bounded handler만 native VM용으로 추출하고 제한 없는 application/overlay 코드는 Bun에 남깁니다. `Spellwire.start()`가 수동 update loop 없이 두 lifecycle을 함께 관리합니다.

## API 한눈에 보기

| 할 일 | API |
| --- | --- |
| 입력을 차단하는 hotkey | `rt.hotkey("Ctrl+Shift+K", handler)` |
| key remap | `rt.remap("CapsLock", "Escape")` |
| 영속 상태 | module-scope `let enabled = true` |
| 키보드/마우스 출력 | `tapKey`, `keyDown`, `clickMouse`, `moveMouse`, `wheelMouse` |
| 지연 | `sleep.ms(250)`, `sleep.seconds(2)` 또는 단위별 helper |
| 입력 + watch + UI 시작 | `Spellwire.start(options)` |
| Overlay layout | `ui.row`, `ui.column`, `ui.panel`, `ui.stack` |
| Overlay content | `ui.text`, `ui.ellipse`, `ui.dot`, `ui.badge`, `ui.divider` |
| 상태 기반 UI | `overlay: state => ...`, `ui.bind`, `ui.when` |
| UI style | `width`, `height`, `padding`, `gap`, `fill`, `stroke`, `shadow`, `opacity`, font prop |
| Overlay window | `overlayOptions.window` (`alwaysOnTop`, `transparent`, `focusable`, `clickThrough` 등) |

signature, 기본값, option 표, 완전한 state-to-overlay 앱, native window 정책과 플랫폼별 주의점은 **[API 레퍼런스](docs/api.ko.md)**에서 확인할 수 있습니다.

첫 npm 배포 이후 기존 프로젝트에 직접 설치하려면 다음 명령을 사용합니다.

```bash
bun add spellwire
```

## 상태 기반 실시간 TypeScript

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

portable 문자열이 modifier 보일러플레이트를 없애고, `when`이 action과 원본 입력 통과를 함께 gate하며, remap은 down/up 전환을 자동으로 한 쌍 생성합니다. 실시간 핸들러가 참조하는 모듈 범위 정수/불리언 `let` 선언은 영속 네이티브 상태가 됩니다. 더 큰 상태 머신에는 조건, 반복, 산술, helper, held 조회, delay, low-level `rt.onKey*`/`rt.onMouse*`도 그대로 사용할 수 있습니다. 실시간 핸들러 밖의 일반 Bun 코드는 제한 없는 control plane입니다.

## 빌드

생성된 프로젝트에서는 다음 명령을 실행합니다.

```bash
bun run build
```

결과는 `dist/main.spellwire.bin`과 `dist/main.spellwire.bin.json`입니다. 다른 파일을 직접 컴파일하려면 다음과 같이 실행합니다.

```bash
bunx spellwire compile src/main.ts
```

출력 경로를 생략한 직접 CLI 컴파일은 입력 파일 옆에 결과를 만듭니다.

## 저장소에서 실행하기

```bash
git clone https://github.com/eunhhu/spellwire.git
cd spellwire
bun run setup
bun run compile:example
bun run inspect:example
bun run simulate:example
```

시뮬레이터는 C ABI와 동일한 바이너리 형식을 해석하고, 실제 `spellwire-core` VM에 명명된 키/마우스 이벤트를 전달하며, 출력 batch와 이벤트 이후의 영속 상태를 보여 줍니다.

저장소에서 전역 입력과 hot reload를 실행하려면 다음 명령을 사용합니다.

```bash
bun run build:native
bun packages/spellwire/src/cli.ts watch examples/stateful.spellwire.ts
```

CLI가 시작 전에 권한을 확인하고 요청합니다. `Ctrl+C`를 누르면 observer/runtime을 중지하고 아직 눌린 상태로 추적 중인 합성 입력을 해제합니다.

## 현재 구현 상태

| 기능 | 상태 |
| --- | --- |
| TypeScript AOT 컴파일러 | 구현 완료 |
| 영속 정수/불리언 상태 | 구현 완료 |
| 조건, 반복, 대입, held 조회, helper 함수 | 구현 완료 |
| portable consuming hotkey, release trigger, 상태 gate, remap | Windows/macOS 구현, Linux suppression 미구현 |
| 네이티브 VM, 버전 wire format, 고정 출력 batch | 구현 완료 |
| 네이티브 inspector/simulator | 구현 완료 |
| 명시적 dispatch 및 owned-host C ABI | 구현 완료 |
| Bun FFI 호스트, 명명 상태, watch/reload, SPSC lane | 구현 완료 |
| Windows `SendInput`, macOS `CGEventPost`, Linux evdev/uinput | 구현 완료, macOS 실제 검증 완료, Windows 대화형 루프백 검증 완료, Windows 물리 입력 차단과 Linux 대상 실행은 미검증 |
| 상태 기반 auto-layout overlay, modern style, configurable native window 정책, retained dirty update | 구현 완료, macOS와 Windows lifecycle/window-policy smoke 검증 완료, Windows 시각적 투명도와 Linux compositor 검증은 미완료 |
| 플랫폼별 prebuilt/signing workflow | 구현 완료, 배포 자격 증명 필요 |
| 물리 입력부터 대상 앱까지의 지연 | 미측정 |

## 패키지와 crate

| 이름 | 역할 |
| --- | --- |
| `spellwire` | 공개 SDK, 내장 컴파일러, TypeScript CLI |
| `create-spellwire` | `bun create spellwire` 프로젝트 생성기 |
| `spellwire-core` | 바이트코드 decoder, trigger table, 영속 상태 VM, scheduler |
| `spellwire-native` | 안정 C ABI, owned host, 전역 observer와 injector |
| `spellwire-overlay` | winit/wgpu 기반 네이티브 retained renderer process |
| `spellwire-cli` / `spellwire-sim` | 네이티브 inspector와 결정적 simulator |
| `spellwire-bench` | 네이티브 dispatch percentile benchmark |

## 문서

여기서 시작:

- **[API 레퍼런스](docs/api.ko.md)** — 생성/실행/빌드, hotkey, 상태, 출력, 수명 주기, overlay API, 기본값과 한계
- [빠른 시작](docs/quick-start.ko.md) — 첫 프로젝트와 첫 live run
- [문제 해결](docs/troubleshooting.ko.md) — 오류와 플랫폼 설정
- [플랫폼 검증](docs/platform-verification.ko.md) — macOS, Windows, Linux 복사 가능 검증 절차

선택형 상세 문서:

- [한국어 문서 목차](docs/index.ko.md)
- [자동화 의미론과 AutoHotkey 마이그레이션](docs/automation.ko.md)
- [Overlay renderer와 성능 설계](docs/overlay.ko.md)
- [Realtime compiler subset](docs/typescript-runtime.ko.md)
- [Live native host 내부 구조](docs/live-host.ko.md)
- [아키텍처](docs/architecture.ko.md), [네이티브 C ABI](docs/native-abi.ko.md), [플랫폼 상태](docs/platforms.ko.md)
- [배포](docs/publishing.ko.md), [구현 상태](docs/status.ko.md), [검증 절차](docs/runtime-verification.ko.md)

## 개발 검증

```bash
bun install --frozen-lockfile
bun run check
cargo build --workspace --release --locked
```

`bun run check`는 TypeScript 테스트, Rust 테스트, 포맷 검사, warning을 오류로 처리하는 Clippy를 실행합니다.

```bash
bun run bench
bun run bench:platform -- 10000
```

Spellwire는 VM dispatch, OS submission, OS loopback, 물리 switch-to-application 지연을 서로 다른 측정 경계로 취급합니다. 플랫폼별 percentile과 jitter 측정 없이 물리 end-to-end 성능을 추정하지 않습니다.

## 라이선스

MIT
