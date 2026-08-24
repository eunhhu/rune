# Spellwire documentation

[한국어](index.ko.md)

Use these pages in order when evaluating the current alpha:

1. **[Quick Start](quick-start.md)** — install or clone, compile, inspect, and simulate a stateful macro.
2. **[Hotkeys and automation](automation.md)** — consuming chords, remaps, native state gates, overlay integration, and AutoHotkey migration status.
3. **[Live Native Host Guide](live-host.md)** — permissions, CLI use, programmatic lifecycle, hot reload, named state, dynamic input, and safe shutdown.
4. **[Platform Verification Guide](platform-verification.md)** — copyable macOS, Windows, and Linux checks with expected output and failure interpretation.
5. **[API Reference](api.md)** — exact exports available from `spellwire` and `spellwire/compiler`.
6. **[TypeScript Runtime](typescript-runtime.md)** — persistent state, control flow, helper functions, limits, and unsupported syntax.
7. **[Architecture](architecture.md)** — public package, compiler, wire format, VM, simulator, and host boundary.
8. **[Native C ABI](native-abi.md)** — owned platform host, shared input ring, and compatibility engine.
9. **[Platform Status](platforms.md)** — backend APIs, permissions, validation, and target limitations.
10. **[Overlay](overlay.md)** — state binding, Figma-style layout/styling API, retained dirty renderer.
11. **[Troubleshooting](troubleshooting.md)** — setup, compiler, simulator, and host-boundary issues.
12. **[Publishing](publishing.md)** — npm package release and verification.
13. **[Implementation Status](status.md)** — implemented features and external gates.
14. **[Verification](runtime-verification.md)** — checks that gate the source tree and pull request.

## Choose the right path

- To learn the macro language without global hooks, complete **Quick Start** through the simulator.
- To run real keyboard/mouse automation, continue with **Live Native Host Guide**.
- To certify one OS or hand target-machine results back to a maintainer, use **Platform Verification Guide**.
- To investigate an error, start with **Troubleshooting**, then follow its link to the relevant detailed guide.
