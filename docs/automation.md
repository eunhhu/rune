# Hotkeys, remaps, and state-driven automation

[한국어](automation.ko.md)

Spellwire's realtime input plane is designed for native latency and a small authoring surface. Modifier hotkeys, release hotkeys, one-key remaps, repeat policy, original-input suppression, and boolean state gates compile before the host starts. No JavaScript callback runs in an OS hook.

> Spellwire is not yet a complete AutoHotkey replacement. Native hotstrings, arbitrary non-modifier combinations, Unicode text sending, window/control automation, clipboard/process helpers, and image/pixel search remain explicit gaps. See [AutoHotkey migration status](#autohotkey-migration-status).

## Small complete example

```ts
import { Key, rt, tapKey } from "spellwire";

let enabled = true;
let presses = 0;

rt.hotkey("Ctrl+Shift+K", () => {
  presses += 1;
  tapKey(Key.Enter);
}, {
  repeat: false,
  when: () => enabled,
});

rt.hotkey("F8", () => {
  enabled = !enabled;
}, { consume: false });

rt.remap("CapsLock", "Escape", { when: () => enabled });
```

This source produces four native handlers: one chord, one toggle, and paired down/up remap handlers. `enabled` and `presses` are persistent native states. While `enabled` is false, the `Ctrl+Shift+K` and `CapsLock` input is not suppressed and passes to the focused application.

## `rt.hotkey()`

```ts
rt.hotkey(chord, handler, options?);
```

Accepted portable names include:

- modifiers: `Ctrl`, `Control`, `Shift`, `Alt`, `Option`, `Meta`, `Cmd`, `Command`, `Win`, and `Super`;
- keyboard names: exported `Key` member names plus common aliases such as `Esc`, `Return`, `PgUp`, `PgDn`, and `Spacebar`;
- mouse names: `LButton`, `RButton`, `MButton`, `XButton1`, and `XButton2`.

Names ignore case, spaces, `_`, and `-`. A chord contains exactly one keyboard key or mouse button; every other token is a logical modifier. Left and right variants satisfy the same modifier group.

```ts
rt.hotkey("Cmd+Space", () => { /* macOS-style chord */ });
rt.hotkey("Ctrl+Alt+K", () => { /* portable names */ });
rt.hotkey("Shift+LButton", () => { /* mouse chord */ });
```

Custom combinations containing two non-modifier keys, such as `A+B`, are not implemented yet. The compiler rejects them instead of silently changing their meaning.

### Options

| Option | Default | Contract |
| --- | --- | --- |
| `source` | `InputSource.Physical` | Match physical, synthetic, or any source |
| `consume` | `true` | Suppress original down/repeat/up sequence when backend supports suppression |
| `exactModifiers` | `true` | Reject extra logical modifiers; `false` allows extras |
| `repeat` | `true` | Run on OS repeat downs; `false` handles first down only |
| `edge` | `"down"` | Run on `"down"` or `"up"` |
| `when` | always active | Gate action and suppression with native boolean state |

Release hotkeys arm suppression and latch their modifier/`when` acceptance on the matching down transition. The paired up transition triggers the handler and is suppressed only when the down was accepted, even if the modifier is released first or the gate changes while the key is held. This avoids delivering a down without its matching up or silently losing an accepted release action.

## Native state gates

`when` accepts a zero-argument function returning one module-scope boolean `let`, or its negation:

```ts
let gaming = false;

rt.hotkey("F6", () => {
  gaming = !gaming;
}, { consume: false });

rt.hotkey("Q", () => tapKey(Key.E), {
  when: () => gaming,
});

rt.remap("CapsLock", "Escape", {
  when: () => !gaming,
});
```

The following forms intentionally fail compilation:

```ts
const dynamicObject = { active: true };
rt.hotkey("Q", () => {}, { when: () => dynamicObject.active });
rt.hotkey("Q", () => {}, { when: () => Date.now() > 0 });
```

The restriction makes suppression decidable without JavaScript, a window query, allocation, or lock in the hook. Handler-internal conditions remain available for richer action logic, but they do not change a trigger's suppression decision; use `when` when inactive input must pass through.

## `rt.remap()`

```ts
rt.remap("CapsLock", "Escape");
rt.remap(Key.CapsLock, Key.Escape, { repeat: false });
```

Source and target accept a single keyboard name or `Key` value. The compiler emits paired native down/up outputs and always consumes the accepted source sequence. `source`, `repeat`, and `when` are supported.

On macOS, physical Caps Lock arrives as a toggle-style `flagsChanged` event rather than an ordinary key pair. Spellwire normalizes each physical Caps Lock activation to one native down/up pulse before remap dispatch, preventing the target key from remaining held.

## One state drives input and overlay

Generated projects separate realtime source from unrestricted application code. Both sides use the same manifest-backed native states.

`src/main.spellwire.ts`:

```ts
import { Key, rt, tapKey } from "spellwire";

let enabled = true;
let presses = 0;

rt.hotkey("Q", () => {
  presses += 1;
  if (presses % 2 === 0) tapKey(Key.E);
}, { when: () => enabled });

rt.hotkey("F8", () => {
  enabled = !enabled;
}, { consume: false });
```

`src/app.ts`:

```ts
import { fileURLToPath } from "node:url";
import { Spellwire, ui } from "spellwire";

const app = await Spellwire.start({
  input: fileURLToPath(new URL("./main.spellwire.ts", import.meta.url)),
  watch: Bun.argv.includes("--watch"),
  overlay: (state) => {
    const enabled = state.enabled === true;
    return ui.column(
      {
        x: 24,
        y: 48,
        width: 280,
        padding: 16,
        gap: 12,
        fill: "#111827ee",
        radius: 16,
        stroke: "#ffffff24",
        shadow: { fill: "#00000066", y: 8, blur: 24 },
      },
      ui.row(
        { width: "fill", gap: 8, align: "center" },
        ui.dot({ size: 8, fill: enabled ? "#34d399ff" : "#fb7185ff" }),
        ui.text(enabled ? "Active" : "Paused", {
          width: "fill",
          fill: "#ffffffff",
          fontSize: 16,
          fontWeight: 600,
        }),
        ui.badge("F8"),
      ),
      ui.text(`Q presses: ${String(state.presses ?? 0)}`, {
        fill: "#cbd5e1ff",
        fontFamily: "monospace",
        fontSize: 13,
      }),
    );
  },
});

await app.untilSignal();
```

Update path:

```text
OS input
  → atomic consume lookup
  → bounded native queue
  → native VM changes enabled/presses
  → one bulk state snapshot at overlay cadence
  → shallow binding comparison
  → keyed primitive diff
  → one coalesced renderer batch
  → dirty-region raster/upload
```

The overlay checks bound state at 30 fps by default; unchanged snapshots reuse the previous tree and send no primitive mutations. Change `overlayOptions: { fps: 60 }`, set `fps: 0` and call `refreshOverlay()` manually, or omit the overlay entirely. None of these choices adds work to the OS hook or native VM path.

## Suppression and performance contract

Windows and macOS advertise `NativeInputSuppression`. Linux currently observes and injects but does not advertise suppression, so `consume` handlers run there while original input still reaches applications.

On supported backends, the hook path contains:

1. fixed key/button translation;
2. source-aware held/modifier bitmap update;
3. one atomic consume-table lookup;
4. one publish to a preallocated, fixed-capacity SPSC queue and a worker wake token.

There is no JavaScript call, IPC, heap allocation, mutex, state-expression evaluation, or overlay work in that path. Queue slots are allocated once at host startup; a 100,000-event test exercises bounded overflow, wake-up, ordering, disconnection, and ring-slot wraparound reuse. A consume table is rebuilt on the worker only when a referenced `when` state changes or a program reload succeeds. If the input queue is full, Spellwire passes new original events, atomically marks recovery, drops the stale backlog, clears pending continuations/input latches, and asks the backend to release every tracked synthetic down. Overload can lose automation actions, but Spellwire does not intentionally swallow an event it could not admit; any backend release failure is retained in `lastError()`.

On the development macOS arm64 machine, three warm 1,000,000-sample core runs were compared with a detached baseline worktree at commit `3fe4256`. Both baseline and this change measured p50/p95 `42 ns`, p99 `42–83 ns`, and p999 `84–125 ns`. This shows no measurable regression at the benchmark's clock resolution; it is not a physical switch-to-application latency claim.

## AutoHotkey migration status

| AutoHotkey v2 area | Spellwire status |
| --- | --- |
| Modifier keyboard/mouse hotkeys | Implemented with portable strings |
| Exact/wildcard modifiers, key repeat, key-up triggers | Implemented |
| Original-input suppression | Implemented on Windows/macOS; Linux relay pending |
| Single-key remapping | Implemented; strings or `Key` values |
| State-conditional hotkeys/remaps | Implemented for native boolean gates |
| Persistent state and bounded realtime state machines | Implemented |
| Modern native overlay/GUI styling | Implemented with retained Figma-style layout |
| Custom `A & B` combinations | Not implemented |
| Hotstrings/text expansion | Not implemented |
| Unicode `SendText`/layout-aware text injection | Not implemented |
| Active-window/process/control predicates | Not implemented |
| Clipboard, process launch, timers, dialogs | Not implemented as stable public helpers |
| Image/pixel search and control automation | Not implemented |

Equivalent starting points:

```ahk
; AutoHotkey v2
^+k::Send "{Enter}"
CapsLock::Escape
```

```ts
// Spellwire
rt.hotkey("Ctrl+Shift+K", () => tapKey(Key.Enter));
rt.remap("CapsLock", "Escape");
```

Do not describe Spellwire as an AutoHotkey superset until the missing rows are implemented and target-machine behavior is verified. The architectural target is broader: portable TypeScript control plane, bounded native realtime plane, and retained native UI without putting general-purpose runtime work in the input hook.

Primary AutoHotkey references: [Hotkeys](https://www.autohotkey.com/docs/v2/Hotkeys.htm), [Hotstrings](https://www.autohotkey.com/docs/v2/Hotstrings.htm), [#HotIf](https://www.autohotkey.com/docs/v2/lib/_HotIf.htm), and [Send](https://www.autohotkey.com/docs/v2/lib/Send.htm).

## Verification

On macOS after permissions are granted:

```bash
bun run build:native
bun run test:consume-macos
bun run test:platform-loopback
bun run check
```

The consume smoke first proves its CoreGraphics tail probe sees two unblocked transitions, then verifies an inactive `when` gate still forwards both. After enabling the native gate, the VM handler runs once while zero transitions reach the tail probe.

Use [Platform Verification](platform-verification.md) for Windows and Linux target-machine reports.
