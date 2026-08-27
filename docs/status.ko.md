# 구현 상태

[English](status.md)

Spellwire는 초기 alpha입니다. 이 문서는 구현된 영역과 플랫폼 acceptance 결과를 구분해 기록합니다.

## 구현 완료

- TypeScript AST compiler, source diagnostic, portable hotkey parser, paired remap, module-scope named integer/boolean state, direct state-immediate update, native 상태 gate, condition, loop, inline helper, held query, key/mouse intrinsic
- versioned `SPWR` v5 encoder, v3/v4/v5 decoder, fixed-payload effect opcode와 structural/runtime validation
- bounded native VM stack/local/output batch/instruction budget/fixed trigger table
- fixed-capacity continuation scheduler: `sleep.us/ms/seconds/minutes/hours()`가 wide/scaled delay opcode 하나로 lowering되고 observer worker를 block하지 않은 채 absolute deadline까지 yield
- compatibility C ABI와 ABI v5 owned-host lifecycle/reload/scalar·bulk state/permission/error/dispatch/shared input/event ring
- Bun FFI `NativeHost`: start/stop, `.ts` memory compile, `.bin` manifest, serialized watch reload, name/kind state preservation
- native observer에서 shared 6-word SPSC ring으로 연결되는 callback-free `DynamicInputLane`
- changed state/effect용 callback-free 20-word `RuntimeEventLane`, cached state 복구, 변경 기반 overlay refresh, 인증 local Electron/sidecar RPC
- Windows low-level keyboard/mouse hook, lock-free 원본 입력 차단, tagged batched `SendInput`
- macOS active `CGEventTap`, lock-free 원본 입력 차단, permission check, Caps Lock pulse 정규화, private tagged `CGEventPost`, tap recovery
- Linux evdev discovery/hotplug, dedicated uinput keyboard/mouse. 선택적 원본 입력 relay는 미구현
- physical/synthetic recursion classification와 USB HID translation test
- fill/stroke/radius/shadow/opacity/font style을 가진 상태 기반 Figma식 row/column/stack layout, keyed diff, 통합 lifecycle API
- configurable native overlay window policy(transparent/topmost/focusable/click-through/decorated/resizable/visible), text/rect/ellipse/line, coalesced batch protocol, dirty raster, partial GPU upload
- VM/overlay reconciliation/OS-submission percentile benchmark
- cross-platform CI, Rust 1.81, npm dry-run, checksum, optional Windows/macOS signing/notarization artifact matrix

## 검증 상태

| 영역 | macOS arm64 | Windows x64 | Linux x64 |
| --- | --- | --- | --- |
| Rust/TypeScript unit test | local 통과 | 대상 장비 통과 | CI source coverage |
| target compile + Clippy | 통과 | 대상 장비 통과 | cross-target 통과 |
| observe → VM → inject → observe loopback | 통과 | 대화형 session 통과 | 실제 장비 대기 |
| 원본 입력 차단 + 상태 gate pass-through | CoreGraphics head/tail probe 통과 | 물리 입력 smoke 대기 | 미구현 |
| Bun shared dynamic lane | 통과 | 대화형 session 통과 | 실제 장비 대기 |
| native overlay + configurable window-policy smoke | 통과 | window policy/live update 통과, 시각적 투명도 대기 | display/compositor 대기 |

macOS와 Windows loopback은 physical-source F19 test dispatch, native F20 injection, tagged synthetic re-observation, 두 번째 VM handler/state update를 사용합니다. OS backend를 검증하지만 물리 keyboard switch나 target application 수신은 측정하지 않습니다. Windows 검증은 Session 0에서 `SendInput`이 차단되므로 대화형 desktop session에서 실행합니다.

## Capability bit

Windows/macOS의 `spellwire_capabilities()`는 `0xf7`입니다.

```text
HostCallbackInjection | NativeObservation | NativeInjection |
HostLifecycle | NonBlockingDelay | NativeInputSuppression | NativeEventLane
```

Linux는 `NativeInputSuppression` 없이 `0xb7`을 반환합니다.

renderer는 별도 executable이므로 `NativeOverlay` library bit는 reserved 상태입니다. `NativeOverlayRenderer.start()`가 companion executable을 직접 검증합니다.

## 외부 release gate

source 변경만으로 완료할 수 없는 항목:

- 실제 npm publish와 registry propagation
- repository secret을 사용한 Authenticode/Developer ID signing과 Apple notarization
- Windows consuming hotkey/remap 실제 장비 smoke
- Windows overlay per-pixel transparency 시각 검증
- Linux permission/device/display 대상 실행
- suppression을 주장하기 전 Linux exclusive evdev pass-through relay 구현
- physical switch → HID → OS → target application latency
- 지원하려는 X11/Wayland compositor별 Linux overlay 동작

[플랫폼 검증](platform-verification.ko.md)으로 대상 결과를 기록하고 [라이브 네이티브 호스트](live-host.ko.md)로 애플리케이션 통합을 진행하십시오.

[Hotkey와 자동화](automation.ko.md#autohotkey-마이그레이션-상태)의 matrix에서 남은 AutoHotkey compatibility gap을 확인할 수 있습니다.
