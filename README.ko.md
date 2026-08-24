# Spellwire

[English](README.md)

Spellwire는 Bun과 TypeScript를 사용하는 상태 기반 실시간 입력 자동화 런타임입니다. 분석 가능한 입력 핸들러를 입력 이벤트마다 JavaScript로 호출하지 않고, 미리 제한된 네이티브 바이트코드로 컴파일하여 실행합니다.

> 초기 알파 버전입니다. TypeScript AOT 컴파일러, 제한형 네이티브 VM, 비차단 지연 스케줄러, Bun FFI 호스트, Windows/macOS/Linux 입력 백엔드, 공유 동적 입력 lane, retained-mode 네이티브 오버레이가 구현되어 있습니다. macOS는 실제 루프백 검증을 마쳤고 Windows와 Linux는 대상 컴파일 및 단위 테스트를 통과했지만 실제 장비 smoke test가 남아 있습니다.

## 가장 빠르게 시작하기

```bash
bun create spellwire my-automation
cd my-automation
bun run start
```

생성된 프로젝트에는 다음 세 명령만 있습니다.

```bash
bun run start  # 메모리에서 컴파일한 뒤 바로 실행
bun run watch  # 실행하면서 네이티브 hot reload
bun run build  # dist/main.spellwire.bin과 manifest 생성
```

`start`와 `watch`는 네이티브 호스트를 시작하기 전에 플랫폼 권한을 자동으로 확인하고 요청합니다.

첫 npm 배포 이후 기존 프로젝트에 직접 설치하려면 다음 명령을 사용합니다.

```bash
bun add spellwire
```

## 상태 기반 실시간 TypeScript

```ts
import {
  InputSource,
  Key,
  MouseButton,
  clickMouse,
  keyHeld,
  rt,
  sleepUs,
  tapKey,
} from "spellwire";

let phase = 0;
let enabled = true;

function tapRepeated(key: Key, count: number): void {
  for (let index = 0; index < count; index++) {
    tapKey(key);
  }
}

rt.onKeyDown(
  Key.Q,
  () => {
    if (!enabled || keyHeld(Key.LeftShift)) return;

    phase = (phase + 1) % 3;
    tapRepeated(Key.E, phase + 1);

    if (phase === 2) {
      clickMouse(MouseButton.Left);
      sleepUs(80);
    }
  },
  { source: InputSource.Physical },
);

rt.onKeyDown(Key.F8, () => {
  enabled = !enabled;
});
```

실시간 핸들러가 참조하는 모듈 범위 정수/불리언 `let` 선언은 영속 네이티브 상태가 됩니다. 조건문, 제한된 반복문, 산술 연산, helper 함수, held-input 조회, 지연, 키/마우스 출력 intrinsic은 사전에 컴파일됩니다. 실시간 핸들러 밖의 일반 Bun 코드는 제한 없는 control plane으로 남습니다.

## 빌드

생성된 프로젝트에서는 다음 명령만 실행하면 됩니다.

```bash
bun run build
```

결과는 `dist/main.spellwire.bin`과 `dist/main.spellwire.bin.json`입니다. 다른 파일을 직접 컴파일하려면 다음과 같이 실행합니다.

```bash
bunx spellwire compile src/main.spellwire.ts
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
| 네이티브 VM, 버전 wire format, 고정 출력 batch | 구현 완료 |
| 네이티브 inspector/simulator | 구현 완료 |
| 명시적 dispatch 및 owned-host C ABI | 구현 완료 |
| Bun FFI 호스트, 명명 상태, watch/reload, SPSC lane | 구현 완료 |
| Windows `SendInput`, macOS `CGEventPost`, Linux evdev/uinput | 구현 완료, macOS 실제 검증 완료 |
| 투명 click-through retained overlay | 구현 완료, macOS 실제 검증 완료 |
| 플랫폼별 prebuilt/signing workflow | 구현 완료, 배포 자격 증명 필요 |
| 물리 입력부터 대상 앱까지의 마이크로초 지연 보장 | 주장하지 않음 |

## 패키지와 crate

| 이름 | 역할 |
| --- | --- |
| `spellwire` | 공개 SDK, 내장 컴파일러, TypeScript CLI |
| `create-spellwire` | `bun create spellwire` 프로젝트 생성기 |
| `spellwire-core` | 바이트코드 decoder, trigger table, 영속 상태 VM, scheduler |
| `spellwire-native` | 안정 C ABI, owned host, 전역 observer와 injector |
| `spellwire-overlay` | winit/wgpu 기반 투명 retained renderer process |
| `spellwire-cli` / `spellwire-sim` | 네이티브 inspector와 결정적 simulator |
| `spellwire-bench` | 네이티브 dispatch percentile benchmark |

## 문서

- [한국어 문서 목차](docs/index.ko.md)
- [빠른 시작](docs/quick-start.ko.md)
- [라이브 네이티브 호스트](docs/live-host.ko.md)
- [플랫폼 검증](docs/platform-verification.ko.md)
- [API 레퍼런스](docs/api.ko.md)
- [실시간 TypeScript](docs/typescript-runtime.ko.md)
- [아키텍처](docs/architecture.ko.md)
- [네이티브 C ABI](docs/native-abi.ko.md)
- [플랫폼 상태](docs/platforms.ko.md)
- [문제 해결](docs/troubleshooting.ko.md)
- [배포](docs/publishing.ko.md)
- [구현 상태](docs/status.ko.md)
- [검증 절차](docs/runtime-verification.ko.md)
- [오버레이](docs/overlay.ko.md)

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
