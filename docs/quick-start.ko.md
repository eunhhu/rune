# 빠른 시작

[English](quick-start.md)

Spellwire는 기존 Bun 프로젝트에 설치하거나 소스 저장소에서 직접 시험할 수 있습니다. 현재 알파는 결정적 simulator와 실제 전역 입력 native host를 모두 제공합니다.

전역 입력을 켜기 전에 compiler와 simulator로 예제를 확인합니다. lifecycle option은 [라이브 네이티브 호스트](live-host.ko.md), OS별 권한과 검증은 [플랫폼 검증](platform-verification.ko.md)에서 설명합니다.

## 가장 간단한 설치

첫 npm 배포 이후 다음 명령으로 프로젝트를 생성합니다.

```bash
bun create spellwire my-automation
cd my-automation
bun run start
```

생성된 프로젝트에는 정확히 세 가지 workflow만 있습니다.

```bash
bun run start  # 메모리에서 컴파일하고 한 번 실행
bun run watch  # 실행하면서 소스 변경 hot reload
bun run build  # dist에 binary와 JSON manifest 생성
```

`start`와 `watch`는 전역 입력 권한을 자동으로 확인하고 요청합니다. 일반 사용에 별도 권한 명령은 필요하지 않습니다.

scaffold는 realtime 로직과 상태 기반 overlay를 `src/main.ts` 하나에 둡니다. compiler는 realtime handler만 native bytecode로 추출하고 `Spellwire.start()`는 제한 없는 application/UI code를 Bun에 둔 채 같은 lifecycle을 관리합니다. layout과 window option은 [오버레이](overlay.ko.md)를 참고하십시오.

기존 프로젝트에는 다음처럼 설치합니다.

```bash
bun add spellwire
```

배포 패키지는 지원 플랫폼에 맞는 native library와 overlay executable을 포함하도록 조립됩니다.

## 소스 저장소에서 개발하기

### 요구 사항

- Bun 1.4.0 이상
- Rust 1.81 이상
- Git

### 1. clone 및 전체 빌드

```bash
git clone https://github.com/eunhhu/spellwire.git
cd spellwire
bun run setup
```

`bun run setup`은 frozen Bun 설치, TypeScript project reference 빌드, 전체 Rust workspace release 빌드를 순서대로 실행합니다.

### 2. 포함된 매크로 컴파일

```bash
bun run compile:example
```

정상 결과:

```text
examples/stateful.spellwire.bin
examples/stateful.spellwire.bin.json
```

네이티브 프로그램 구조를 확인합니다.

```bash
bun run inspect:example
```

inspector는 handler 수, persistent state 수, instruction 수, resource limit, 초기 상태, trigger source, bytecode entry point를 출력합니다.

### 3. 실제 Rust VM simulator 실행

```bash
bun run simulate:example
```

이 명령은 실제 Rust VM에 `Q` press/release 세 쌍을 전달합니다. 각 입력마다 다음 내용을 출력합니다.

- 일치한 handler 수
- 실행한 instruction 수
- output event 수
- zero-delay output batch
- dispatch 이후 persistent state

예제는 이벤트 사이에 `phase`를 유지하고, `E` tap 횟수를 바꾸며, 조건이 맞으면 왼쪽 click과 80µs VM delay를 생성합니다.

### 4. 새 매크로 작성

`macro.spellwire.ts`를 만듭니다.

```ts
import {
  Key,
  MouseButton,
  clickMouse,
  keyDown,
  keyUp,
  rt,
  sleep,
} from "spellwire";

let combo = 0;
let enabled = true;

function tap(key: Key, count: number): void {
  for (let index = 0; index < count; index++) {
    keyDown(key);
    keyUp(key);
    sleep.us(40);
  }
}

rt.hotkey(
  "Q",
  () => {
    combo++;
    if (combo >= 3) {
      tap(Key.E, 2);
      clickMouse(MouseButton.Left);
      combo = 0;
    }
  },
  { repeat: false, when: () => enabled },
);

rt.hotkey("F8", () => {
  enabled = !enabled;
}, { consume: false, repeat: false });
```

직접 컴파일합니다.

```bash
bunx spellwire compile macro.spellwire.ts
```

inspect와 simulator를 실행합니다.

```bash
cargo run -q -p spellwire-cli --locked -- inspect macro.spellwire.bin
cargo run -q -p spellwire-cli --locked -- simulate macro.spellwire.bin \
  key-down:Q key-up:Q \
  key-down:Q key-up:Q \
  key-down:Q key-up:Q
```

## Simulator 이벤트 문법

```text
key-down:Q
key-up:Q
key-down:LeftShift
key-up:0xe1
mouse-down:left
mouse-up:left
key-down:Q:synthetic
```

event kind는 `key-down`, `key-up`, `mouse-down`, `mouse-up`입니다. 마지막 source는 선택 사항이며 `physical` 또는 `synthetic`을 사용합니다. 기본값은 `physical`입니다.

키 이름은 대소문자를 구분하지 않고 하이픈/밑줄을 무시합니다. 일반 USB HID 이름, 문자, 숫자, function key, modifier, arrow, `0xNN` 코드가 지원됩니다.

## 생성되는 상태 manifest

compiler는 binary 옆에 `<program>.spellwire.bin.json`을 생성합니다.

```json
{
  "states": {
    "combo": { "slot": 0, "kind": "number" },
    "enabled": { "slot": 1, "kind": "boolean" }
  }
}
```

`NativeHost`는 이 manifest를 사용해 `host.states.combo`와 `host.state("enabled")`를 노출하고, hot reload 중 이름과 kind가 같은 값을 보존합니다.

## 실제 전역 입력 실행

생성된 프로젝트에서는 `start` 또는 `watch`만 사용하면 됩니다. 소스 저장소에서는 먼저 네이티브 파일을 빌드합니다.

```bash
bun run build:native
```

한 번 실행하거나 watch mode를 시작합니다.

```bash
bun packages/spellwire/src/cli.ts run macro.spellwire.ts
bun packages/spellwire/src/cli.ts watch macro.spellwire.ts
```

두 명령 모두 `.ts`를 메모리에서 컴파일하고 권한을 준비한 뒤 플랫폼 observer/injector를 시작합니다. `watch`는 직렬화된 filesystem reload만 추가합니다. `.spellwire.bin`과 인접 JSON manifest도 입력으로 받을 수 있습니다. `Ctrl+C`로 안전하게 종료합니다.

## 현재 장비 검증

```bash
bun run test:platform-loopback
bun run bench:platform -- 10000
target/release/spellwire-overlay --smoke
bun run test:overlay-live
```

Windows overlay 파일은 `target/release/spellwire-overlay.exe`입니다. Linux overlay는 활성 graphical session이 필요합니다. `bench:platform`은 OS submission call의 반환 시간만 측정하며 물리 입력부터 target application까지의 지연이 아닙니다.

루프백 성공 출력에는 최소한 다음 값이 포함됩니다.

```json
{"loopback":"ok","observed":1,"reloadReleasedHeldInput":true}
```

실제 출력에는 `platform`, `arch`, `elapsedUs`도 포함됩니다. 자세한 해석과 OS별 설정은 [플랫폼 검증](platform-verification.ko.md)을 참고하십시오.

## 저장소 전체 검증

```bash
bun run check
cargo clippy --workspace --all-targets --locked
```

영구 GitHub workflow는 Linux, macOS, Windows에서 Rust test/build를 반복하고 Rust 1.81, npm tarball, compiler → wire format → simulator smoke path를 확인합니다.

## 플랫폼 검증 상태

macOS arm64는 permission, loopback, suppression, dynamic lane, overlay, submission benchmark를 검증했습니다. Windows x64는 대화형 세션에서 source, build, loopback/reload, dynamic lane, overlay lifecycle/window policy, package, benchmark를 검증했으며 물리 suppression과 시각적 transparency 확인이 남아 있습니다. Linux는 source coverage가 있고 실제 device와 graphical session 검증이 남아 있습니다. 자세한 상태는 [플랫폼 상태](platforms.ko.md)를 참고하십시오.

## 다음 문서

- 실제 애플리케이션 host: [라이브 네이티브 호스트](live-host.ko.md)
- 지원 문법과 제한: [실시간 TypeScript](typescript-runtime.ko.md)
- export와 signature: [API 레퍼런스](api.ko.md)
- OS별 검증: [플랫폼 검증](platform-verification.ko.md)
- 오류 진단: [문제 해결](troubleshooting.ko.md)
