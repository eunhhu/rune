# 플랫폼 검증 가이드

[English](platform-verification.md)

모든 release 대상 OS에서 이 절차를 사용하십시오. source check, native OS loopback, submission timing, overlay startup, 물리 end-to-end latency는 서로 다른 검증입니다. 한 단계의 성공이 다음 단계를 증명하지 않습니다.

생성 프로젝트의 일반 사용자는 별도 권한 workflow가 필요하지 않습니다. `bun run start`와 `bun run watch`가 권한을 자동 준비합니다. 이 release checklist는 raw ABI/capability/permission 값을 기록해야 하므로 호환용 고급 `permissions` 진단 명령을 사용합니다.

## 각 검증이 증명하는 범위

| 검증 | 증명하는 내용 | 증명하지 않는 내용 |
| --- | --- | --- |
| `bun run check` | TypeScript/Rust compile, test, format, Clippy | device permission, 실제 OS 동작 |
| `permissions` | native library load와 현재 process의 resource 접근 상태 | Windows의 모든 integrity level 대상 injection |
| `test:platform-loopback` | VM 출력 → OS injection → global observation → synthetic 분류 → VM 상태 갱신 | 물리 keyboard latency, target application 수신 |
| `bench:platform` | native OS submission call 반환 시간 | device delivery, compositor, application polling |
| overlay `--smoke` | window, GPU surface, transparency mode, event loop 초기화 | 모든 compositor와 multi-monitor 표시 품질 |

## 공통 사전 확인

저장소 root에서 실행하고 출력 전체를 보고서에 보관합니다.

```bash
git rev-parse HEAD
bun --version
rustc --version
cargo --version
```

최소 버전은 Bun 1.4.0, Rust 1.81입니다.

```bash
bun install --frozen-lockfile
bun run check
bun run build:native
```

`bun run check`가 실패하면 플랫폼 결과를 해석하기 전에 portable failure부터 해결하십시오.

루프백 test는 전역 synthetic F20 event를 주입합니다. 문자나 mouse click은 만들지 않지만 다른 애플리케이션이 F20 shortcut을 사용할 수 있습니다. F20에 민감한 애플리케이션을 닫거나 binding을 끄고 실행하십시오. 플랫폼 benchmark는 zero-delta mouse movement batch를 제출하므로 정상적으로는 pointer가 움직이지 않습니다.

## macOS 검증

### 1. 두 privacy permission 허용

```bash
bun packages/spellwire/src/cli.ts permissions --request
```

macOS에서 필요한 항목:

- **시스템 설정 → 개인정보 보호 및 보안 → 입력 모니터링**: observation
- **시스템 설정 → 개인정보 보호 및 보안 → 손쉬운 사용**: injection

Bun을 실제로 시작하는 애플리케이션에 권한을 주어야 합니다. Terminal, iTerm, IDE, Codex 중 실제 launcher를 확인하십시오. 상태가 바뀌지 않으면 해당 애플리케이션을 완전히 종료하고 다시 여십시오. 명령만 재실행하면 기존 process의 privacy 상태가 갱신되지 않을 수 있습니다.

```bash
bun packages/spellwire/src/cli.ts permissions
```

정상 형태:

```text
library: /.../target/release/libspellwire_native.dylib
ABI: 3
capabilities: 0x37
observe: granted
inject: granted
```

### 2. 네이티브 loopback

```bash
bun run test:platform-loopback
```

정상 JSON 형태:

```json
{"platform":"darwin","arch":"arm64","loopback":"ok","observed":1,"reloadReleasedHeldInput":true,"elapsedUs":123456}
```

Intel Mac은 `x64`를 출력합니다. 정확한 `elapsedUs` 값은 환경마다 다르며 latency benchmark가 아닙니다. 이 scenario는 reload cleanup을 확인하기 위해 의도적으로 sleep과 polling을 포함합니다.

### 3. OS submission benchmark

```bash
bun run bench:platform -- 10000
```

p50, p95, p99, p999, max nanosecond를 출력합니다. macOS에서는 `CGEventPost` 제출 작업이 반환될 때까지의 시간입니다. 물리 switch-to-application latency로 발표하면 안 됩니다.

### 4. Overlay smoke

```bash
target/release/spellwire-overlay --smoke
bun run test:overlay-live
```

약 350ms 뒤 종료하며 다음 형태의 한 줄을 출력합니다.

```json
{"event":"ready","width":3420,"height":2214,"alphaMode":"PostMultiplied"}
```

화면 크기와 alpha mode는 monitor/GPU에 따라 달라집니다. 양수 크기와 유효한 `alphaMode`가 중요하며 예시 값과 같을 필요는 없습니다.

## Windows 검증

일반 PowerShell에서 먼저 실행합니다.

```powershell
bun install --frozen-lockfile
bun run check
bun run build:native
bun packages/spellwire/src/cli.ts permissions
```

library는 `target\release\spellwire_native.dll`이어야 합니다. Windows는 low-level hook과 `SendInput`에 사전 permission prompt가 없으므로 현재 두 permission bit를 granted로 보고합니다.

이 값은 UIPI를 우회하지 않습니다. 일반 권한 Spellwire process는 관리자 권한 target에 입력을 주입할 수 없습니다. 먼저 일반 desktop application을 대상으로 검증하십시오. 관리자 target 검증이 필요하면 Spellwire를 같은 integrity level에서 실행하고 보고서에 명시하십시오. elevation을 일반 설치 요구 사항으로 취급하지 마십시오.

```powershell
bun run test:platform-loopback
bun run bench:platform -- 10000
.\target\release\spellwire-overlay.exe --smoke
bun run test:overlay-live
```

정상 loopback 형태:

```json
{"platform":"win32","arch":"x64","loopback":"ok","observed":1,"reloadReleasedHeldInput":true,"elapsedUs":123456}
```

Windows arm64에서는 `arch`가 `arm64`여야 합니다. 일반 앱에서는 성공하고 elevated 앱에서만 실패한다면 mapping failure보다 UIPI 동작일 가능성이 높습니다.

## Linux 검증

Linux backend는 evdev를 읽고 uinput device 하나를 만듭니다. 전역 입력을 노출하는 interface이므로 device 접근을 명시적으로 허용해야 합니다.

### 1. Device 확인

```bash
ls -l /dev/input/event* /dev/uinput
bun packages/spellwire/src/cli.ts permissions
```

`observe: granted`는 읽을 수 있는 evdev device를 하나 이상 발견했다는 뜻입니다. `inject: granted`는 `/dev/uinput`을 열었다는 뜻입니다. Linux의 `permissions --request`는 rule을 설치하거나 prompt를 표시하지 않고 같은 resource를 다시 조회합니다.

### 2. 제공 udev rule 검토 및 선택적 설치

먼저 내용을 읽습니다.

```bash
cat packaging/linux/99-spellwire-input.rules
```

이 rule은 활성 local seat에 `uaccess`를 부여하며 device를 world-readable로 만들지 않습니다. `/etc` 변경은 시스템 설정 변경이며 관리자 승인이 필요합니다.

```bash
sudo install -m 0644 packaging/linux/99-spellwire-input.rules /etc/udev/rules.d/99-spellwire-input.rules
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=input
```

접근 권한이 즉시 갱신되지 않으면 logout/login 또는 device 재연결이 필요할 수 있습니다. systemd-logind `uaccess`가 없는 headless session/distribution은 배포판에 맞는 group/service rule이 필요합니다. 영구 해결책으로 `chmod 666`을 사용하지 마십시오.

```bash
bun packages/spellwire/src/cli.ts permissions
```

### 3. Loopback과 benchmark

```bash
bun run test:platform-loopback
bun run bench:platform -- 10000
```

정상 형태:

```json
{"platform":"linux","arch":"x64","loopback":"ok","observed":1,"reloadReleasedHeldInput":true,"elapsedUs":123456}
```

backend는 uinput device registration을 기다리고, Spellwire virtual device를 이름으로 식별해 돌아오는 event를 synthetic으로 분류합니다. timeout은 udev가 새 event node를 현재 session에 아직 노출하지 못했다는 뜻일 수 있습니다.

### 4. 실제 graphical session의 overlay

```bash
printf 'XDG_SESSION_TYPE=%s DISPLAY=%s WAYLAND_DISPLAY=%s\n' \
  "${XDG_SESSION_TYPE:-}" "${DISPLAY:-}" "${WAYLAND_DISPLAY:-}"
target/release/spellwire-overlay --smoke
bun run test:overlay-live
```

지원하려는 desktop/compositor마다 반복하십시오. X11과 Wayland의 window semantics는 다르며 winit만으로 모든 Wayland compositor에 하나의 universal layer-shell guarantee를 제공할 수 없습니다. desktop 이름/버전, session type, monitor 배치, transparency/topmost/click-through 결과를 기록하십시오.

## Loopback scenario의 정확한 의미

`scripts/platform-loopback.ts`는 다음 순서로 동작합니다.

1. `examples/platform-loopback.spellwire.ts`를 ABI v4로 load
2. observe와 inject permission 확인
3. 64-record `DynamicInputLane` 연결
4. physical-source F19를 VM에 명시적으로 dispatch
5. VM이 실제 OS backend로 tagged F20 주입
6. 돌아온 synthetic F20을 observe하고 명명 상태를 `1`로 갱신
7. F18에서 delayed held F20 sequence 생성
8. release deadline 전에 program reload
9. reload가 held synthetic F20 release를 정확히 한 번 보냈는지 확인
10. `finally`에서 host close

첫 F19는 사람이 누른 물리 key가 아니라 repeatable test를 위한 명시적 dispatch입니다. F20 injection과 돌아오는 observation은 실제 platform backend를 통과합니다.

## Benchmark 해석

```text
Spellwire platform submission benchmark (10000 zero-delta mouse batches)
p50    150000 ns
p95    250000 ns
p99    600000 ns
p999   900000 ns
max   1200000 ns

Scope: native OS submission call return; device delivery and application polling excluded.
```

같은 OS, hardware, power state, background load에서 regression을 비교할 때 사용하십시오. 일반 desktop 결과를 VM, remote desktop, battery saver, 다른 backend와 직접 비교해 end-to-end latency 차이라고 부르면 안 됩니다.

물리 latency에는 외부 timestamped switch actuator 또는 target application instrumentation이 필요합니다. USB polling, OS delivery, scheduler, 필요한 경우 compositor, application polling을 모두 보고하십시오.

## 실패 matrix

| 실패 | 가능성 높은 원인 | 다음 확인 |
| --- | --- | --- |
| native library not found | build 누락 또는 architecture 불일치 | `target/release` 파일명, stale override 확인 |
| ABI mismatch | JS와 native artifact가 다른 commit | 같은 checkout에서 재빌드 |
| macOS observation missing | launcher에 Input Monitoring 없음 | 정확한 앱 entry 확인 후 앱 완전 재시작 |
| macOS injection missing | launcher에 Accessibility 없음 | 권한 허용 후 launcher 재시작 |
| Windows loopback timeout | hook/session/integrity mismatch | 일반 local desktop과 같은 integrity에서 재검증 |
| Linux observation missing | 읽을 수 있는 event device 없음 | owner/ACL과 active-seat udev 확인 |
| Linux injection missing | `/dev/uinput` 없음 또는 write 불가 | 배포판 방식으로 uinput enable, ACL 확인 |
| Linux injection 후 timeout | virtual device registration/ACL 지연 | `/sys/class/input/*/device/name`, udev event 확인 |
| overlay ready 전 종료 | GPU surface/adapter 또는 graphical session 없음 | stderr, session env, driver, headless/remote 여부 확인 |
| Wayland에서 topmost 아님 | compositor policy 차이 | compositor 기록 후 전용 layer-shell 통합 검토 |
| unsupported HID usage | 의도적으로 translation 거부 | 지원 `Key`, layout, 원하는 usage 보고 |

## 복사 가능한 검증 보고서

```text
Spellwire commit:
OS edition/version:
CPU architecture:
Physical or virtual machine:
Bun version:
Rust version:

Permission output:
  library:
  ABI:
  capabilities:
  observe:
  inject:

Loopback JSON:

Platform benchmark (sample count and full percentiles):

Overlay ready JSON:
Overlay live-state JSON:
Overlay visually transparent/topmost/click-through:
Monitor count and scaling:

Linux only:
  distribution/kernel:
  desktop/compositor:
  XDG_SESSION_TYPE:

Windows only:
  Spellwire integrity level:
  target integrity level:

Unexpected stderr/errors:
Notes:
```

현재 검증 근거와 남은 외부 gate는 [런타임 검증](runtime-verification.ko.md)과 [구현 상태](status.ko.md)를 참고하십시오.
