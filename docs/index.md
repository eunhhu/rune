# Spellwire documentation

[한국어](index.ko.md)

Choose a guide by task.

## Start

- **[Quick Start](quick-start.md)** — create a project, simulate it safely, and perform the first live run.
- **[API reference](api.md)** — find project commands, realtime input, state, output, lifecycle, and overlay APIs.
- **[Troubleshooting](troubleshooting.md)** — resolve setup, compiler, native host, and overlay errors by message.
- **[Platform Verification](platform-verification.md)** — run the macOS, Windows, or Linux acceptance checklist.

## Topics

| Document | Contents |
| --- | --- |
| [Automation semantics](automation.md) | Suppression rules, state gates, output helpers, timing, and the AutoHotkey migration matrix |
| [Realtime TypeScript](typescript-runtime.md) | Compiler syntax, loops, helpers, diagnostics, and resource budgets |
| [Effects and RPC](effects-rpc.md) | Typed transient events, changed-state subscriptions, Electron/sidecar IPC, and performance boundaries |
| [Overlay design](overlay.md) | State binding, layout, styling, window policy, reconciliation, and renderer isolation |
| [Live native host](live-host.md) | Host lifecycle, dynamic input lane, reload, shutdown, and library resolution |
| [Platform status](platforms.md) | Backend capabilities, permissions, verification status, and compositor notes |
| [Architecture](architecture.md) | Compiler, wire format, VM, worker, native host, and renderer boundaries |
| [Native C ABI](native-abi.md) | Stable ABI for embedding Spellwire outside Bun |

## Maintainer and release material

- [Implementation status](status.md)
- [Runtime verification](runtime-verification.md)
- [Publishing](publishing.md)
