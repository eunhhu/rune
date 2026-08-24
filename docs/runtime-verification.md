# Runtime Verification

The source tree was materialized and committed only after these checks passed on Linux:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets
cargo build -p rune-native --release
bun run typecheck
bun run test:ts
```

The permanent `CI` workflow now gates pull requests with:

- Rust formatting, workspace tests, Clippy, and release builds on Linux, macOS, and Windows;
- `cargo check --workspace --locked` on Rust 1.81;
- frozen Bun installation, TypeScript project build, and TypeScript tests;
- a Quick Start smoke test that compiles `examples/stateful.rune.ts`, inspects the binary, and dispatches events through `rune-sim`.

Pedantic Clippy findings are currently advisory during the MVP. Formatting, compilation, tests, binary decoding, and Quick Start execution are merge blockers.
