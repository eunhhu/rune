# 구현 상태

[English](status.md)

Spellwire는 README 구현 계획이 source에 반영된 초기 alpha입니다. source 구현과 대상 플랫폼 검증을 의도적으로 구분합니다.

## 구현 완료

- TypeScript AST compiler, source diagnostic, module-scope named integer/boolean state, condition, loop, update, inline helper, held query, key/mouse intrinsic
- versioned `SPWR` encoder/decoder와 structural/runtime validation
- bounded native VM stack/local/output batch/instruction budget/fixed trigger table
- fixed-capacity continuation scheduler: `sleepUs()`가 observer worker를 block하지 않고 absolute deadline까지 yield
- compatibility C ABI와 ABI v4 owned-host lifecycle/reload/scalar·bulk state/permission/error/dispatch/shared ring
- Bun FFI `NativeHost`: start/stop, `.ts` memory compile, `.bin` manifest, serialized watch reload, name/kind state preservation
- native observer에서 shared 6-word SPSC ring으로 연결되는 callback-free `DynamicInputLane`
- Windows low-level keyboard/mouse hook와 tagged batched `SendInput`
- macOS listen-only `CGEventTap`, permission check, private tagged `CGEventPost`, tap recovery
- Linux evdev discovery/hotplug와 dedicated uinput keyboard/mouse
- physical/synthetic recursion classification와 USB HID translation test
- fill/stroke/radius/shadow/opacity/font style을 가진 상태 기반 Figma식 row/column/stack layout, keyed diff, 통합 lifecycle API
- text/rect/ellipse/line, coalesced batch protocol, dirty raster, partial GPU upload를 가진 transparent/topmost/click-through retained overlay process
- VM/overlay reconciliation/OS-submission percentile benchmark
- cross-platform CI, Rust 1.81, npm dry-run, checksum, optional Windows/macOS signing/notarization artifact matrix

## 검증 상태

| 영역 | macOS arm64 | Windows x64 | Linux x64 |
| --- | --- | --- | --- |
| Rust/TypeScript unit test | local 통과 | CI source coverage | CI source coverage |
| target compile + Clippy | 통과 | cross-target 통과 | cross-target 통과 |
| observe → VM → inject → observe loopback | 통과 | 실제 장비 대기 | 실제 장비 대기 |
| Bun shared dynamic lane | 통과 | 실제 장비 대기 | 실제 장비 대기 |
| native transparent overlay smoke | 통과 | 실제 장비 대기 | display/compositor 대기 |

macOS loopback은 physical-source F19 명시 dispatch, native F20 injection, tagged synthetic re-observation, 두 번째 VM handler/state update를 사용합니다. OS loopback이며 물리 keyboard의 switch-to-application latency가 아닙니다.

## Capability bit

`spellwire_capabilities()`:

```text
HostCallbackInjection | NativeObservation | NativeInjection |
HostLifecycle | NonBlockingDelay
```

renderer는 별도 executable이므로 `NativeOverlay` library bit는 reserved 상태입니다. `NativeOverlayRenderer.start()`가 companion executable을 직접 검증합니다.

## 외부 release gate

source 변경만으로 완료할 수 없는 항목:

- 실제 npm publish와 registry propagation
- repository secret을 사용한 Authenticode/Developer ID signing과 Apple notarization
- Windows/Linux permission/setup smoke
- physical switch → HID → OS → target application latency
- 지원하려는 X11/Wayland compositor별 Linux overlay 동작

[플랫폼 검증](platform-verification.ko.md)으로 대상 결과를 기록하고 [라이브 네이티브 호스트](live-host.ko.md)로 애플리케이션 통합을 진행하십시오.
