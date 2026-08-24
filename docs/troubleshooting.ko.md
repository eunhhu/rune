# 문제 해결

[English](troubleshooting.md)

## `spellwire` 또는 `spellwire/compiler`를 찾을 수 없음

```bash
bun install --frozen-lockfile
```

workspace package를 resolve할 수 있도록 저장소 root에서 명령을 실행하십시오. 생성 프로젝트에서는 `bun install`이 성공했는지 확인하십시오.

## TypeScript build가 declaration output 누락을 보고함

SDK/compiler는 TypeScript project reference를 사용합니다. 개별 file check 대신 build mode를 사용합니다.

```bash
bun run typecheck
```

생성물을 지우려면:

```bash
bun run clean:ts
```

## Realtime handler가 없다고 나옴

registration은 inline callback을 가진 top-level call이어야 합니다.

```ts
rt.onKeyDown(Key.Q, () => {
  tapKey(Key.E);
});
```

다른 runtime function 안에 숨긴 registration은 현재 compiler가 발견하지 않습니다.

## 일반 TypeScript 문법을 handler가 거부함

module에는 제한 없는 control-plane TypeScript가 존재할 수 있지만 realtime handler는 bounded integer bytecode로 lowering 가능한 값만 참조할 수 있습니다.

주요 원인:

- string/object/array capture
- 임의 Bun/npm function call
- non-constant key 또는 handler option
- 값을 반환하는 helper
- recursion/runtime-created closure
- destructuring/dynamic property access

[실시간 TypeScript](typescript-runtime.ko.md)를 참고하십시오.

## `sleepUs()`가 정확하지 않음

live host는 absolute deadline과 non-blocking continuation을 사용하지만 desktop OS는 hard realtime scheduler가 아닙니다. compatibility engine/simulator는 delay를 동기 실행하므로 긴 wait에서 멈춘 것처럼 보일 수 있습니다.

microsecond 값은 deadline request이며 물리 end-to-end guarantee가 아닙니다.

## `bun macro.spellwire.ts`가 전역 입력을 만들지 않음

source module 직접 실행은 fallback handler만 등록합니다. native host를 사용하십시오.

```bash
bun run build:native
bun packages/spellwire/src/cli.ts run macro.spellwire.ts
```

`run`은 startup 전 권한을 확인/요청합니다. source change를 자동 적용하려면 `spellwire watch macro.spellwire.ts`를 사용하십시오.

Linux는 evdev/uinput 권한이 필요합니다. Windows는 UIPI 때문에 높은 integrity process에 inject할 수 없습니다. [플랫폼 상태](platforms.ko.md)와 [플랫폼 검증](platform-verification.ko.md)을 참고하십시오.

## Native library 출력이 아무 동작도 하지 않음

low-level `SpellwireEngine`은 output callback이 없으면 출력을 버립니다. built-in OS injection에는 owned `NativeHost`/`spellwire_host_*`를 사용하고 custom embedder라면 engine callback을 설치하십시오.

일반 CLI는 missing permission을 자동 보고합니다. embedder는 `host.permissionStatus()`와 `spellwire_host_last_error`를 확인할 수 있습니다. unsupported HID usage는 명시적 platform error입니다.

## Native overlay가 시작되지 않음

```bash
bun run build:native
```

비표준 위치는 `SPELLWIRE_OVERLAY_EXECUTABLE`을 설정합니다. Linux는 active graphical session이 필요하고 topmost transparency는 compositor에 의존합니다. [플랫폼 검증](platform-verification.ko.md)의 smoke command와 failure matrix를 확인하십시오.

## Simulator가 event를 거부함

```text
key-down:Q
key-up:LeftShift
mouse-down:left
mouse-up:forward
key-down:0x14:synthetic
```

선택 source는 `physical` 또는 `synthetic`입니다. handler filter는 `InputSource.Any`를 사용할 수 있지만 실제 event 자체는 concrete source여야 합니다.

## 생성 파일이 계속 보임

Git ignore 대상:

```text
*.spellwire.bin
*.spellwire.bin.json
packages/*/dist/
*.tsbuildinfo
```

TypeScript output 제거:

```bash
bun run clean:ts
```

## Rust test는 통과하지만 Clippy warning이 나옴

workspace는 `clippy::pedantic`을 사용하고 CI/`bun run check`는 `-D warnings`를 전달하므로 warning도 실패입니다. 다만 `block 0.1.6` 같은 transitive future-incompatibility notice는 현재 crate source warning과 별도일 수 있습니다.

## 전체 local 검증

```bash
bun run check
cargo clippy --workspace --all-targets --locked
bun run compile:example
bun run inspect:example
bun run simulate:example
bun run test:platform-loopback
target/release/spellwire-overlay --smoke
```

Windows overlay는 `target/release/spellwire-overlay.exe --smoke`입니다. OS loopback과 platform benchmark는 portable source check와 별도로 기록하십시오.

## Issue 보고에 포함할 내용

- OS와 CPU
- Bun/Rust version
- commit SHA
- 최소 `.spellwire.ts` source
- compiler diagnostic 또는 simulator output
- state mapping이 관련되면 generated manifest

latency report는 core dispatch, OS submission, OS loopback, physical switch-to-application 중 어느 경계를 측정했는지 반드시 구분해야 합니다.
