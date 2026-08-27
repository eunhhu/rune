# Spellwire documentation

[한국어](index.ko.md)

Normal use starts with one page. The remaining pages are task guides or implementation deep dives, not required chapters.

## Use Spellwire

1. **[One-page API reference](api.md)** — copyable complete app; project commands; hotkeys; remaps; state; keyboard/mouse output; `Spellwire.start`; every overlay constructor, property, option, lifecycle method, and current limitation.
2. **[Quick Start](quick-start.md)** — create a project, simulate safely, then perform the first live run.
3. **[Troubleshooting](troubleshooting.md)** — find setup, compiler, native host, and overlay errors by message.
4. **[Platform Verification](platform-verification.md)** — run the macOS, Windows, or Linux acceptance checklist and report exact results.

The API reference is the normal lookup surface. It intentionally combines automation and overlay APIs so a state-to-screen workflow needs no page change.

## Optional behavior and design detail

| Document | Use it when… |
| --- | --- |
| [Automation semantics](automation.md) | You need suppression rules, state-gate behavior, or the AutoHotkey migration matrix |
| [Realtime TypeScript](typescript-runtime.md) | You need compiler syntax limits, loops, helpers, or resource budgets |
| [Overlay design](overlay.md) | You are profiling reconciliation/rendering or studying renderer isolation |
| [Live native host](live-host.md) | You need low-level host lifetime, dynamic lane, reload, or library resolution details |
| [Platform status](platforms.md) | You need backend capability and compositor caveats |
| [Architecture](architecture.md) | You need compiler, wire, VM, worker, and renderer boundaries |
| [Native C ABI](native-abi.md) | You are embedding Spellwire outside Bun |

## Maintainer and release material

- [Implementation status](status.md)
- [Runtime verification](runtime-verification.md)
- [Publishing](publishing.md)
