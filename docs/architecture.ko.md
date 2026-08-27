# 아키텍처

[English](architecture.md)

Spellwire는 제한 없는 Bun control plane과 제한된 native event execution을 분리합니다.

## Data flow

```text
.spellwire.ts
    │ TypeScript AST compilation
    ▼
SPWR bytecode + named-state manifest
    │ Bun FFI load/reload
    ▼
platform observer → atomic consume policy → fixed SPSC queue → runtime worker → platform injector
                                                               │
                                                               ├─ fixed continuation deadlines
                                                               └─ optional SharedArrayBuffer event ring → Bun

Bun OverlayScene mutations → pipe → separate native retained renderer
```

## TypeScript SDK와 compiler

공개 package는 USB HID key, realtime registration marker, output/held intrinsic, fallback test helper, compiler, `NativeHost`, `DynamicInputLane`, named state wrapper, overlay client를 제공합니다.

compiler는 source를 실행하지 않고 parse하며 top-level `rt.hotkey`, `rt.remap`, `rt.on*` registration을 찾습니다. 표현 가능한 state/constant/helper를 해석하고 제한 subset을 검증한 뒤 다음을 생성합니다.

- 초기 persistent integer state
- logical modifier, repeat/consume flag, optional boolean state gate를 포함한 source/device/edge/code trigger
- fixed-width integer instruction
- stack/local/instruction limit

별도 manifest는 source state name을 native slot과 kind에 연결하며 Bun control plane에서만 사용합니다.

## Wire format과 VM

`SPWR`은 version header 뒤에 resource limit, state, handler, bytecode를 저장합니다. `Program::decode`는 구조 경계를 검증하고 `Runtime::new`는 dispatch 전에 entry, jump, slot, stack behavior, budget을 확인합니다.

runtime은 direct source × device × edge × code trigger table, fixed held-input bitmap, fixed VM stack/local/output storage, fixed-capacity continuation scheduler를 사용합니다. 흔한 state/immediate update는 stack traffic을 우회합니다. zero-delay instruction은 한 output batch가 됩니다. 모든 `sleep.*()` unit helper는 wide/scaled delay opcode 하나로 lowering되어 batch를 flush하고 absolute monotonic deadline과 함께 handler를 yield합니다. owned host는 새 input/control command를 계속 받으면서 ready continuation을 poll합니다.

간단한 embedder를 위한 lower-level compatibility engine은 synchronous delay를 유지합니다.

## Owned native host

`spellwire-native`는 하나의 lifecycle 위에 세 backend를 구현합니다.

- Windows: low-level keyboard/mouse hook message loop, tagged `SendInput`
- macOS: suppression 가능한 active CoreGraphics event tap, private source의 tagged `CGEventPost`
- Linux: nonblocking evdev poll/hotplug discovery, dedicated uinput device

Windows/macOS observation callback은 제한된 translation, source-aware held tracking, atomic suppression lookup 1회, 고정 용량 SPSC publish 1회만 수행합니다. SPSC queue는 미리 할당한 slot, acquire/release counter, worker wake token을 사용하며 표준 channel의 mutex 기반 wake 경로를 호출하지 않습니다. callback에는 JavaScript, allocation, mutex, IPC, overlay 작업이 없습니다. worker가 VM state, deadline, reload, state command, consume-table publish, injection을 독점합니다. gate 값은 VM dispatch와 publish된 suppression table 양쪽에서 확인합니다. Linux도 같은 queue를 사용하지만 grab한 모든 device capability를 안전하게 보존하는 exclusive evdev relay가 구현될 때까지 observe/inject-only입니다.

stop은 observer/worker thread를 join하고 추적 중인 synthetic held input을 해제합니다. consume한 down은 paired repeat/up을 추적하여 대상 앱에 불완전한 sequence가 전달되지 않게 합니다.

## Dynamic JavaScript lane

`DynamicInputLane`은 JavaScript가 꼭 필요한 event를 위한 best-effort control plane입니다. `NativeHost.attachDynamicLane()`은 고정 6-word SPSC ring을 worker와 공유합니다. producer는 JavaScript를 호출하지 않고 full이면 block 대신 drop/count합니다. drain 시점과 thread는 Bun이 선택합니다.

## Overlay 격리

desktop window event loop는 특히 macOS에서 main-thread ownership이 필요하므로 overlay는 companion executable입니다. Bun은 bound state snapshot이 바뀔 때만 Figma식 auto-layout tree를 만들고 stable primitive key를 reconcile한 뒤 newline JSON batch 하나를 보냅니다. native는 primitive를 retained하고 old/new dirty bounds 합집합만 rerasterize하며 256-byte row에 정렬된 texture 영역만 upload합니다. input dispatch와 renderer lock을 공유하지 않아 renderer 종료가 host를 멈추지 않습니다.

## 측정 경계

1. **Core dispatch:** lookup + VM + null/recording injector (`bun run bench`)
2. **OS submission:** native platform call return (`bun run bench:platform`)
3. **OS loopback:** injection → native observation → VM state (`bun run test:platform-loopback`)
4. **Physical end-to-end:** switch → HID → OS → Spellwire → target application

마지막 경계만 외부 hardware/target instrumentation이 필요합니다. 앞 세 결과로 마지막 값을 추정하지 않습니다.
