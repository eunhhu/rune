# 런타임 검증

[English](runtime-verification.md)

## Portable source gate

```bash
bun install --frozen-lockfile
bun run typecheck
bun run test:ts
bun run test:docs
bun run pack:dry-run
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --release --locked
```

GitHub Actions는 Linux, macOS, Windows에서 Rust test, Clippy, release build를 반복하고 Rust 1.81을 확인합니다. 두 npm tarball과 compiler → wire format → simulator 빠른 시작도 검증합니다.

## Native target gate

모든 release OS에서 권한 설정 후 실행합니다.

```bash
bun run build:native
bun run inspect:runtime
bun run test:platform-loopback
bun run test:consume-macos # macOS only
bun run bench:platform -- 10000
target/release/spellwire-overlay --smoke
bun run test:overlay-live
```

Windows overlay는 `target/release/spellwire-overlay.exe`입니다. Linux overlay는 graphical session이 필요합니다. loopback은 실제 native injection, global observation, synthetic classification, 2단계 VM execution, named state access를 확인합니다.

OS별 setup, 예상 출력, 보고서 형식은 [플랫폼 검증](platform-verification.ko.md)을 사용하십시오.

## 현재 local 근거

macOS arm64에서 다음 항목을 통과했습니다.

- 전체 Rust workspace test/Clippy/release build
- TypeScript build와 test
- Bun FFI를 통한 ABI v4 load, bulk state snapshot, permission read
- tagged F20 `CGEventPost` injection → `CGEventTap` observation → synthetic VM trigger
- CoreGraphics suppression probe: baseline/inactive-gate transition `2/2`, active native handler hit `1`, forwarded transition `0`
- `DynamicInputLane` publication과 smoke scenario drop 0
- Retina resolution의 기본 transparent/topmost/non-focusable/click-through overlay와 hidden opaque/focusable/decorated/resizable non-default 정책 생성
- Bun에 resolved window 정책을 반환하는 live overlay mutation rendering
- direct state-immediate VM workload 200,000회 local sample: trigger lookup + VM + null injection 기준 p50 42ns, p95 84ns, p99 84ns
- native OS-submission benchmark

Windows 10 x64의 대화형 desktop session에서 다음 항목을 통과했습니다.

- `bun run check`와 locked release workspace build
- native observe → VM → `SendInput` → observe loopback, synthetic 분류, reload 중 held-input release
- dynamic-lane publication
- 기본·custom overlay window-policy smoke와 live mutation rendering
- package dry-run과 두 native benchmark
- 해당 실행의 VM benchmark p50 100ns, p99 200ns와 platform submission p50 14.5µs, p99 30.1µs

같은 injection 검증을 SSH service의 Windows Session 0에서 실행하면 `ACCESS_DENIED`가 발생하므로 Windows live 검증은 로그인된 대화형 session에서 실행해야 합니다. 물리 consuming-hotkey suppression과 시각적 per-pixel transparency는 수동 acceptance가 남아 있으며 smoke renderer는 `alphaMode: "Opaque"`를 보고했습니다. Linux backend는 cross-target source gate를 통과했지만 실제 device/display 실행은 남아 있습니다. Linux suppression은 미구현이라 capability bit가 설정되지 않습니다.

이 검증은 물리 switch-to-target-application latency를 측정하지 않습니다. 해당 주장은 외부 timestamp hardware 또는 target application instrumentation이 필요합니다.
