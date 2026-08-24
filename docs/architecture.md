# Architecture

[한국어](architecture.ko.md)

Spellwire separates unrestricted Bun control-plane work from bounded native event execution.

## Data flow

```text
.spellwire.ts
    │ TypeScript AST compilation
    ▼
SPWR bytecode + named-state manifest
    │ Bun FFI load/reload
    ▼
platform observer → atomic consume policy → fixed SPSC queue → runtime worker → platform injector
                                                               │
                                                               ├─ fixed continuation deadlines
                                                               └─ optional SharedArrayBuffer event ring → Bun

Bun OverlayScene mutations → pipe → separate native retained renderer
```

## TypeScript SDK and compiler

The public package supplies USB HID-style keys, realtime registration markers, output/held intrinsics, fallback test helpers, the embedded compiler, `NativeHost`, `DynamicInputLane`, named state wrappers, and the overlay client.

The compiler parses source without executing it, finds top-level `rt.hotkey`, `rt.remap`, and `rt.on*` registrations, resolves representable state/constants/helpers, validates the bounded subset, and emits:

- initial persistent integer state;
- source/device/edge/code triggers with logical modifiers, repeat/consume flags, and optional boolean state gates;
- fixed-width integer instructions;
- stack/local/instruction limits.

The companion manifest maps source state names to numeric slots and kinds. It is used only by the Bun control plane.

## Wire format and VM

`SPWR` has a versioned header followed by resource limits, state, handlers, and bytecode. `Program::decode` validates structural bounds; `Runtime::new` validates entries, jumps, slots, stack behavior, and budgets before any dispatch.

The runtime uses a direct source × device × edge × code trigger table, fixed held-input bitmaps, fixed VM stack/locals/output storage, and a fixed-capacity continuation scheduler. A zero-delay instruction run becomes one output batch. `sleepUs()` flushes that batch and yields the handler with an absolute monotonic deadline. The owned host polls ready continuations while continuing to receive new input and control commands.

The lower-level compatibility engine retains synchronous delay behavior for simple embedders.

## Owned native host

`spellwire-native` implements one lifecycle above three backends:

- Windows: low-level keyboard/mouse hook message loop and tagged `SendInput`;
- macOS: active CoreGraphics event tap with suppression and tagged `CGEventPost` from a private source;
- Linux: nonblocking evdev polling/hotplug discovery and a dedicated uinput device.

Windows/macOS observation callbacks perform bounded translation, source-aware held tracking, one atomic suppression lookup, and one fixed-capacity SPSC publish. The SPSC queue uses preallocated slots, acquire/release counters, and a worker wake token; it does not call the standard mutex-backed wake path. The callback contains no JavaScript, allocation, mutex, IPC, or overlay work. The worker owns VM state, deadlines, reloads, state commands, consume-table publication, and injection. Gate values are checked in both VM dispatch and the published suppression table. Linux uses the same queue but remains observe/inject-only until an exclusive evdev relay can preserve every grabbed device capability safely.

Stopping the host joins observer/worker threads and releases tracked held synthetic inputs. Consumed down transitions track their paired repeats/up transitions so target applications do not receive unmatched sequences.

## Dynamic JavaScript lane

`DynamicInputLane` is best-effort control-plane plumbing for events that truly require JS. `NativeHost.attachDynamicLane()` shares its fixed six-word SPSC ring with the worker. The producer never invokes JavaScript and drops/counts overflow instead of blocking realtime processing. Bun decides when and where to call `drain()`.

## Overlay isolation

The overlay is a companion executable because desktop window event loops need main-thread ownership, especially on macOS. Bun builds a Figma-style auto-layout tree only when a bound state snapshot changes, reconciles stable primitive keys, and sends one coalesced newline-JSON batch. Native code retains primitive nodes, rerasterizes the union of old/new dirty bounds, and uploads only a 256-byte-row-aligned texture region. It shares no renderer lock with input dispatch; renderer exit cannot stop the host.

## Measurement boundaries

Report these separately:

1. **Core dispatch:** lookup + VM + null/recording injector (`bun run bench`).
2. **OS submission:** native platform call return (`bun run bench:platform`).
3. **OS loopback:** injection → native observation → VM state (`bun run test:platform-loopback`).
4. **Physical end-to-end:** switch → HID → OS → Spellwire → target application.

Only the last boundary requires external hardware/target instrumentation and is not inferred from the first three.
