# 플랫폼 상태

[English](platforms.md)

세 native backend는 하나의 owned-host lifecycle을 공유하고 OS별 event-loop detail만 분리합니다.

## Matrix

| 기능 | Windows | macOS | Linux |
| --- | --- | --- | --- |
| Observation | `WH_KEYBOARD_LL` / `WH_MOUSE_LL` | active `CGEventTap` | evdev + hotplug rescan |
| Injection | tagged batched `SendInput` | private-source tagged `CGEventPost` | dedicated uinput device |
| 원본 입력 차단 | hook이 nonzero 반환 | event tap이 null 반환 | 미구현 |
| physical/synthetic 분류 | injection tag | injection tag | virtual-device identity |
| keyboard/mouse/button/wheel | 지원 | 지원 | 지원 |
| transparent retained overlay | winit/wgpu | accessory-policy winit/wgpu | winit/wgpu, compositor 의존 |
| 실제 local 검증 | 대기 | arm64 loopback + suppression 통과 | 대기 |

Windows/Linux는 unit/mapping test와 macOS에서의 x86_64 cross-target Clippy를 통과했습니다. release artifact를 검증 완료로 취급하기 전에 대상 장비에서 [플랫폼 검증](platform-verification.ko.md)을 실행하십시오.

Windows/macOS suppression은 lock-free trigger table과 paired down/repeat/up tracking을 사용합니다. Linux는 evdev event를 읽고 버리는 것만으로 특정 입력을 안전하게 차단할 수 없습니다. 각 물리 device를 exclusive grab하고 consume하지 않은 모든 capability를 virtual device로 relay해야 합니다. 전체 relay가 아직 없으므로 Linux는 `NativeInputSuppression`을 advertise하지 않으며 `consume`을 사용해도 원본 입력은 앱에 전달됩니다.

## 권한

### Windows

일반 desktop input은 보통 elevation이 필요 없습니다. `SendInput`은 UIPI 적용을 받으므로 일반 Spellwire process는 높은 integrity target에 inject할 수 없습니다. secure desktop/UAC prompt는 범위 밖입니다.

### macOS

observation은 Input Monitoring, injection은 Accessibility가 필요합니다. `spellwire run`과 `spellwire watch`는 두 권한을 자동 확인/요청합니다. raw 상태 진단:

```bash
bun packages/spellwire/src/cli.ts permissions
bun packages/spellwire/src/cli.ts permissions --request
```

privacy 설정을 바꾼 뒤 기존 process가 갱신되지 않으면 terminal/application을 완전히 재시작하십시오.

### Linux

대상 `/dev/input/event*` read와 `/dev/uinput` write가 필요합니다. evdev read는 전역 keyboard input을 노출하므로 보안상 민감합니다. 의도한 장비에서만 `packaging/linux/99-spellwire-input.rules`를 검토하고 설치한 뒤 udev rule을 reload하거나 다시 login하십시오.

## 대상 장비 검증

```bash
bun install --frozen-lockfile
bun run build:native
bun packages/spellwire/src/cli.ts permissions
bun run test:platform-loopback
bun run test:consume-macos # macOS only
bun run bench:platform -- 10000
target/release/spellwire-overlay --smoke
```

loopback은 tagged synthetic F20을 실제 platform injector로 보내고 global backend로 관찰한 뒤 synthetic-source VM handler가 named state를 갱신하는지 확인합니다.

macOS consume smoke는 먼저 tail event tap이 차단되지 않은 transition 2개를 보는지 확인합니다. 이어서 상태 gate pass-through를 검증하고, gate 활성화 뒤에는 VM hit 1회와 forwarded transition 0개를 요구합니다.

Windows overlay는 `target/release/spellwire-overlay.exe`입니다. Linux는 evdev/uinput 권한과 graphical session이 필요합니다. 오류 보고 전 [플랫폼 검증](platform-verification.ko.md)의 상세 절차를 따르십시오.

## Key translation

공개 `Key`는 USB HID keyboard-page usage입니다. 각 backend는 명시적 supported map을 사용하고 안전한 mapping이 없으면 다른 key를 조용히 보내지 않고 `unsupported USB HID key usage`를 반환합니다. Linux는 현재 export set 전체를 다룹니다. macOS/Windows는 API로 신뢰성 있게 표현할 수 없는 usage를 제외합니다. layout/media 동작은 대상 keyboard에서 확인하십시오.

## Overlay 한계

renderer는 transparent, topmost, click-through window를 요청하며 input worker와 격리됩니다. Windows/macOS는 필요한 semantics를 제공합니다. Linux는 display server/compositor에 의존합니다. winit을 통한 universal Wayland always-on-top layer-shell contract가 없으므로 GNOME/KDE/wlroots 대상 환경마다 검증해야 합니다.
