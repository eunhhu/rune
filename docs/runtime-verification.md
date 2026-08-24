# Runtime verification

The merged Spellwire source tree is checked with:

```bash
bun install --frozen-lockfile
bun run typecheck
bun run test:ts
bun run pack:dry-run
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked
cargo build --workspace --release --locked
```

GitHub Actions additionally verifies:

- Rust tests, Clippy, and release builds on Linux, macOS, and Windows;
- Rust 1.81 as the declared MSRV;
- both npm package tarballs;
- a Quick Start smoke path that compiles `examples/stateful.spellwire.ts`, inspects the resulting module, and dispatches events through `spellwire-sim` while checking persistent state.

The simulator validates the compiler → wire format → native VM path. It does not claim physical switch-to-application latency or global OS hook coverage.
