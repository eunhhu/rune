# 라이브 네이티브 호스트 가이드

[English](live-host.md)

이 문서는 Spellwire 매크로를 전역 키보드/마우스 입력에 연결하고, hot reload와 명명 상태를 사용하며, host를 안전하게 종료하는 방법을 설명합니다. 아직 매크로를 컴파일하고 simulator로 실행하지 않았다면 [빠른 시작](quick-start.ko.md)을 먼저 진행하십시오.

## 실행 위치와 책임

Spellwire는 실시간 작업과 일반 TypeScript를 의도적으로 분리합니다.

| 영역 | 실행 위치 | 적합한 작업 |
| --- | --- | --- |
| 실시간 handler | 제한된 네이티브 VM | 상태 갱신, 조건, 제한 반복, held 조회, 키/마우스 출력, `sleepUs()` |
| 네이티브 host | native worker와 OS backend | 전역 입력 관찰, 주입, 지연 continuation, 상태 저장 |
| Control plane | Bun | 로드, 권한, hot reload, 로그, UI, 파일, 네트워크 |
| Dynamic lane | 공유 ring 위의 Bun | 실시간 보장이 필요 없는 best-effort 입력 반응 |

`rt.onKeyDown()` 등 `rt.on*()` 내부 코드는 AOT 컴파일됩니다. `.spellwire.ts` 파일을 `bun`으로 직접 실행하면 JavaScript fallback handler만 등록되며 전역 hook은 설치되지 않습니다. 실제 입력에는 `spellwire run`, `spellwire watch`, 또는 `NativeHost`를 사용하십시오.

## 가장 빠른 사용법

```bash
bun create spellwire my-automation
cd my-automation
bun run start
```

생성된 프로젝트는 다음 세 script만 제공합니다.

| Script | 결과 |
| --- | --- |
| `bun run start` | 소스를 메모리에서 컴파일하고 네이티브 host 시작 |
| `bun run watch` | 같은 host를 시작하고 승인된 소스 변경 hot reload |
| `bun run build` | `dist/main.spellwire.bin`과 JSON manifest 생성 |

`start`와 `watch`는 host 시작 전에 전역 입력 권한을 한 번 확인합니다. 권한이 없으면 macOS에서 요청하고, 모든 플랫폼에서 해결 방법이 포함된 오류를 출력합니다. 이 작업은 native realtime dispatch에 권한 query, allocation, JavaScript callback, polling을 추가하지 않습니다.

## 소스 저장소 요구 사항

- Bun 1.4.0 이상
- Rust 1.81 이상
- [플랫폼 검증](platform-verification.ko.md)에 설명한 OS 권한

```bash
bun install --frozen-lockfile
bun run build:native
```

생성 파일:

```text
Windows: target/release/spellwire_native.dll
         target/release/spellwire-overlay.exe
macOS:   target/release/libspellwire_native.dylib
         target/release/spellwire-overlay
Linux:   target/release/libspellwire_native.so
         target/release/spellwire-overlay
```

`run`과 `watch`는 일반 사용에 필요한 권한을 자동 준비합니다. raw ABI/capability/permission 값을 확인해야 하는 플랫폼 진단에서는 다음 고급 명령을 사용할 수 있습니다.

```bash
bun packages/spellwire/src/cli.ts permissions
```

현재 소스에서는 ABI `3`, capabilities `0x37`, `observe: granted`, `inject: granted`가 정상입니다. Windows UIPI는 대상 process에 따라 달라지고, macOS는 두 privacy grant가 필요하며, Linux는 device file 접근 권한이 필요합니다.

## CLI의 세 workflow

### 1. AOT binary 만들기

```bash
bunx spellwire compile macro.spellwire.ts
```

출력:

```text
macro.spellwire.bin
macro.spellwire.bin.json
```

두 번째 JSON은 상태 manifest입니다. binary를 직접 실행할 때 함께 보관하십시오. 다른 출력 경로의 상위 디렉터리가 없어도 compiler가 생성합니다.

소스 저장소에서는 다음 명령으로 구조와 동작을 먼저 확인할 수 있습니다.

```bash
cargo run -q -p spellwire-cli --locked -- inspect macro.spellwire.bin
cargo run -q -p spellwire-cli --locked -- simulate macro.spellwire.bin key-down:Q key-up:Q
```

### 2. 바로 실행하기

설치된 패키지:

```bash
bunx spellwire run macro.spellwire.ts
```

소스 저장소:

```bash
bun packages/spellwire/src/cli.ts run macro.spellwire.ts
```

입력을 생략하면 `src/main.spellwire.ts`를 사용합니다. 정상 startup은 다음과 비슷합니다.

```text
running /absolute/path/to/macro.spellwire.ts (press Ctrl+C to stop)
```

`Ctrl+C`를 한 번 누르면 observer와 runtime worker를 중지하고, delayed continuation을 취소하며, host가 눌린 상태로 추적하던 합성 key/button을 해제하고 native library를 닫습니다.

### 3. Hot reload 실행하기

```bash
bunx spellwire watch macro.spellwire.ts
```

승인된 변경은 `reloaded`를 출력합니다. 컴파일할 수 없는 편집은 `reload failed: ...`를 출력하지만 기존 프로그램은 계속 실행되므로 수정 후 다시 저장할 수 있습니다. 여러 filesystem event가 빠르게 들어와도 reload는 직렬화됩니다.

### 컴파일된 binary 실행

기본 manifest는 `<binary>.json`입니다.

```bash
bunx spellwire run macro.spellwire.bin
```

다른 manifest를 사용하려면 다음과 같이 지정합니다.

```bash
bunx spellwire run macro.spellwire.bin --manifest configs/macro-state.json
```

### CLI 요약

| 명령 | 용도 |
| --- | --- |
| `spellwire run [source-or-binary]` | 소스를 메모리에서 컴파일하고 owned native host 즉시 시작 |
| `spellwire watch [source-or-binary]` | 같은 실행 경로에 직렬화된 hot reload 추가 |
| `spellwire compile [source] [output]` | AOT binary와 상태 manifest 생성 |

세 명령의 기본 입력은 `src/main.spellwire.ts`입니다. `--library <path>`는 native library 탐색을, `--manifest <path>`는 compiled input의 인접 manifest를 덮어씁니다.

## `NativeHost` 직접 사용

Bun process가 애플리케이션 상태, dynamic input lane, overlay도 직접 관리해야 한다면 programmatic API를 사용합니다.

```ts
import { NativeHost, NativePermission } from "spellwire";

const host = await NativeHost.load("macro.spellwire.ts");
const required = NativePermission.Observe | NativePermission.Inject;

let permissions = host.permissionStatus();
if ((permissions & required) !== required) {
  permissions = host.requestPermissions();
}
if ((permissions & required) !== required) {
  host.close();
  throw new Error("Spellwire needs observation and injection permissions");
}

const watcher = host.watch({
  debounceMs: 75,
  preserveState: true,
  onReload: () => console.log("reloaded"),
  onError: (error) => console.error("reload failed", error),
});

try {
  host.start();
  console.log("phase", host.state("phase").get());

  await new Promise<void>((resolveStop) => {
    const stop = (): void => {
      process.off("SIGINT", stop);
      process.off("SIGTERM", stop);
      resolveStop();
    };
    process.once("SIGINT", stop);
    process.once("SIGTERM", stop);
  });
} finally {
  watcher.close();
  host.close();
}
```

`close()`는 idempotent이며 필요하면 `stop()`을 호출합니다. 전역 hook이나 합성 held input이 예외 뒤에 남지 않도록 항상 `finally`에서 호출하십시오.

## 수명 주기 규칙

| 작업 | 동작 |
| --- | --- |
| `NativeHost.load(path)` | `.ts` 컴파일 또는 `.bin`/manifest 읽기, ABI 검증, native host 할당 |
| `permissionStatus()` | prompt 없이 observe/inject bit 조회 |
| `requestPermissions()` | macOS에서는 요청, Windows/Linux에서는 현재 상태 재조회 |
| `start()` | injector, observer, runtime worker, scheduler 생성 |
| `reload()` | 새 program 설치, 기존 continuation 취소, held output 해제 |
| `stop()` | native 작업 중지, wrapper는 열린 상태 유지 |
| `close()` | 필요 시 stop, host free, dynamic library close |

TypeScript wrapper를 통한 중복 `start()`/`stop()`은 이미 해당 상태이면 no-op입니다. `close()` 뒤의 다른 작업은 `Spellwire native host is closed`를 throw합니다.

## 명명 상태와 reload

실시간 handler가 참조하는 모듈 범위 정수/불리언은 native state가 됩니다.

```ts
let phase = 0;
let enabled = true;
```

```ts
host.state("enabled").set(false);
console.log(host.state("enabled").get());
console.log(host.states.phase?.get());
```

실행 중 reload에서는 source name과 state kind가 모두 같을 때만 값을 보존합니다.

| 편집 | 결과 |
| --- | --- |
| 숫자 `phase` 유지 | 현재 값 보존 |
| slot 순서만 변경 | slot이 아닌 이름으로 보존 |
| `phase`를 `step`으로 이름 변경 | source 초기값으로 시작 |
| 숫자를 boolean으로 변경 | 새 kind의 source 초기값으로 시작 |
| `reload({ preserveState: false })` | 모든 상태를 source 초기값으로 재설정 |

manifest가 바뀔 수 있다면 reload 전의 `NativeState` object를 계속 보관하지 말고 reload 후 `host.state(name)`에서 다시 가져오십시오.

## `DynamicInputLane`으로 Bun에서 이벤트 읽기

실시간 handler는 JavaScript를 호출하지 않습니다. 일반 Bun 코드에도 입력 알림이 필요할 때 shared SPSC ring을 연결합니다.

```ts
import {
  DynamicInputLane,
  InputDevice,
  InputEdge,
  Key,
  NativeHost,
} from "spellwire";

const lane = new DynamicInputLane(1024);
const host = await NativeHost.load("macro.spellwire.ts");

const unsubscribe = lane.on(
  InputDevice.Keyboard,
  Key.Q,
  InputEdge.Down,
  (event) => {
    const timestampNs =
      (BigInt(event.timestampHi >>> 0) << 32n) |
      BigInt(event.timestampLo >>> 0);
    console.log({ source: event.source, timestampNs });
  },
);

host.attachDynamicLane(lane);
let timer: ReturnType<typeof setInterval> | undefined;

try {
  host.start();
  timer = setInterval(() => {
    const drained = lane.drain(1024);
    if (drained > 0 || lane.ring.dropped > 0) {
      console.log({ drained, queued: lane.ring.size, dropped: lane.ring.dropped });
    }
  }, 8);
  await Bun.sleep(10_000);
} finally {
  if (timer !== undefined) clearInterval(timer);
  unsubscribe();
  host.close();
}
```

capacity는 2 이상 2^31 이하의 2의 거듭제곱이어야 합니다. ring이 가득 차면 `lane.ring.dropped`가 증가하고 읽지 않은 event는 덮어쓰지 않습니다. 실제 traffic을 측정해 capacity와 drain 주기를 선택하십시오. 이 lane은 best-effort control plane이며 지연에 민감한 자동화 경로가 아닙니다.

`drain()`은 소비한 record 수를 반환하며 같은 lane에서 reentrant하게 호출할 수 없습니다. dispatch 중 handler 추가/제거는 현재 snapshot이 아니라 다음 event부터 적용됩니다.

`host.dispatch(...)`는 test와 custom embedder가 VM input을 명시적으로 전달하는 API입니다. 실제 물리 device event나 global observer를 대체하지 않습니다.

## Native library 탐색 순서

`NativeHost`는 다음 순서로 탐색합니다.

1. `nativeLibraryPath` 또는 CLI `--library`
2. `SPELLWIRE_NATIVE_LIBRARY`
3. package의 `native/<platform>-<arch>/`
4. workspace `target/release/`
5. workspace `target/debug/`

overlay는 `executablePath`, `SPELLWIRE_OVERLAY_EXECUTABLE`, package directory, workspace release/debug 순서를 사용합니다. 진단할 때는 절대 경로를 사용하고 다른 OS/CPU용 library를 복사하지 마십시오.

## 합성 입력 재귀 방지

합성 출력은 다시 관찰되며 `InputSource.Synthetic`으로 표시됩니다. 출력이 자신의 trigger와 같을 수 있다면 physical-only filter를 사용하십시오.

```ts
rt.onKeyDown(
  Key.Q,
  () => {
    tapKey(Key.Q);
  },
  { source: InputSource.Physical },
);
```

`InputSource.Any`는 재귀가 의도적이고 상태 또는 다른 조건으로 확실히 제한될 때만 사용하십시오.

## 오류별 조치

| 메시지/증상 | 의미 | 조치 |
| --- | --- | --- |
| `Spellwire native library not found` | 현재 OS/CPU에 맞는 library를 찾지 못함 | package 재설치 또는 source에서 `bun run build:native`; stale override 확인 |
| `native ABI ... is incompatible` | JS wrapper와 native library가 다른 build | 같은 commit에서 다시 빌드하고 override 제거 |
| observation permission 누락 | global observer를 열 수 없음 | macOS Input Monitoring 또는 Linux evdev 접근 허용 |
| injection permission 누락 | injector를 열 수 없음 | macOS Accessibility 또는 Linux `/dev/uinput` write 허용 |
| `status -9` | platform hook, event tap, device, injection 실패 | 뒤에 붙은 native error와 [플랫폼 검증](platform-verification.ko.md) 확인 |
| `status -10` | native worker channel 실패 | host stop/close 후 로그와 함께 재현 |
| `status -11` | delayed continuation capacity 소진 | 동시에 겹치는 delayed handler 감소 또는 custom limit 조정 |
| `unsupported USB HID key usage` | 안전한 platform key translation 없음 | 지원 `Key`와 대상 keyboard layout 확인 |
| reload 후 값 reset | state name/kind 변경 또는 보존 비활성화 | 이전/새 manifest 비교 |
| Bun callback event 누락 | drain 전에 dynamic lane이 가득 참 | capacity/drain 빈도 증가, `ring.dropped` 확인 |

OS별 명령과 복사 가능한 결과 보고서는 [플랫폼 검증](platform-verification.ko.md), compiler/simulator 오류는 [문제 해결](troubleshooting.ko.md)을 참고하십시오.
