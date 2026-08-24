# Rune documentation

Use these pages in order when evaluating the current MVP:

1. **[Quick Start](quick-start.md)** — clone, build, compile, inspect, and simulate a stateful macro.
2. **[API Reference](api.md)** — exact exports currently available from `@rune/sdk` and `@rune/compiler`.
3. **[TypeScript Runtime](typescript-runtime.md)** — persistent state, control flow, helper functions, limits, and unsupported syntax.
4. **[Architecture](architecture.md)** — compiler, wire format, VM, simulator, and host boundary.
5. **[Native C ABI](native-abi.md)** — embedding the VM and receiving native output batches.
6. **[Platform Status](platforms.md)** — what builds on Windows/macOS/Linux and what is not implemented yet.
7. **[Overlay](overlay.md)** — the retained scene API and renderer roadmap.
8. **[Troubleshooting](troubleshooting.md)** — common setup and compiler issues.
9. **[Implementation Status](status.md)** — implemented features and next milestones.
10. **[Verification](runtime-verification.md)** — checks that gate the pull request.

## Naming note

The GitHub repository has been renamed to `eunhhu/spellwire`. This branch still exposes `@rune/*`, `rune-*`, `RUNE` wire-format, and `rune_*` ABI identifiers. A separate rebrand change can rename those symbols without changing the runtime model documented here.
