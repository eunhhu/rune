# Runtime verification

This source tree was committed only after these checks passed:

- `cargo test --workspace`
- `cargo clippy --workspace --all-targets`
- `cargo build -p rune-native --release`
- `bun run typecheck`
- `bun run test:ts`

Pull requests and pushes to `main` repeat the Rust checks on Linux, macOS, and Windows and run the Bun/TypeScript checks on Linux. Pedantic Clippy findings are currently advisory during the MVP phase.
