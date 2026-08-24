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
bun packages/spellwire/src/cli.ts permissions
bun run test:platform-loopback
bun run test:consume-macos # macOS only
bun run bench:platform -- 10000
target/release/spellwire-overlay --smoke
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
- Retina resolution transparent click-through overlay creation/mutation rendering
- native OS-submission benchmark

Windows x64와 Linux x64 backend는 macOS에서 cross-target Clippy를 통과했습니다. 이는 compile 근거이지 live permission/device/display 동작 근거가 아닙니다. Windows suppression은 대상 장비 검증이 남았고 Linux suppression은 미구현이라 capability bit가 설정되지 않습니다.

이 검증은 물리 switch-to-target-application latency를 측정하지 않습니다. 해당 주장은 외부 timestamp hardware 또는 target application instrumentation이 필요합니다.
