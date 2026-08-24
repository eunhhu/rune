# Native C ABI

[한국어](native-abi.ko.md)

`spellwire-native` builds as a `cdylib` and `rlib`:

```bash
cargo build -p spellwire-native --release --locked
```

```text
Windows: target/release/spellwire_native.dll
macOS:   target/release/libspellwire_native.dylib
Linux:   target/release/libspellwire_native.so
```

## Version and capabilities

```c
uint32_t spellwire_abi_version(void);       // 4
uint32_t spellwire_capabilities(void);
uint32_t spellwire_permission_status(void);
uint32_t spellwire_request_permissions(void);
```

Capability bits are:

```text
1 << 0  HostCallbackInjection
1 << 1  NativeObservation
1 << 2  NativeInjection
1 << 3  NativeOverlay       (reserved; renderer is a companion process)
1 << 4  HostLifecycle
1 << 5  NonBlockingDelay
```

The current library returns `0x37`: every bit above except `NativeOverlay`. Permission bits are `1 << 0` observe and `1 << 1` inject.

## Owned platform host

The normal live-input API owns the platform observer, runtime worker, continuation scheduler, and injector:

```c
SpellwireHost *spellwire_host_new(const uint8_t *bytes, size_t len);
void spellwire_host_free(SpellwireHost *host);
int32_t spellwire_host_start(SpellwireHost *host);
int32_t spellwire_host_stop(SpellwireHost *host);
int32_t spellwire_host_reload(
  SpellwireHost *host,
  const uint8_t *bytes,
  size_t len,
  bool preserve_positional_state
);
int32_t spellwire_host_dispatch(
  SpellwireHost *host,
  uint8_t device,
  uint16_t code,
  uint8_t edge,
  uint8_t source
);
int32_t spellwire_host_state_get(const SpellwireHost *host, size_t slot, int64_t *output);
int32_t spellwire_host_state_set(SpellwireHost *host, size_t slot, int64_t value);
int32_t spellwire_host_state_snapshot(
  const SpellwireHost *host,
  int64_t *output,
  size_t capacity
);
size_t spellwire_host_last_error(const SpellwireHost *host, char *buffer, size_t capacity);
```

`start` creates the OS injector/observer and one runtime worker. Observers publish into a bounded channel; delayed handlers yield into a fixed-capacity deadline scheduler. `stop` shuts down observation, clears pending continuations, and releases synthetic key/button downs tracked by the host. `free` also stops a running host.

Reload is synchronous. The low-level flag copies common state slots positionally. The Bun wrapper passes false and preserves compatible values by manifest name and kind, preventing state reordering from corrupting values.

`spellwire_host_last_error(host, NULL, 0)` returns the required UTF-8 buffer length including NUL. A second call copies the most recent platform/runtime message.

`spellwire_host_state_snapshot` copies all persistent slots with one synchronous worker command. `capacity` must cover the program state length. The Bun overlay binding reuses one `BigInt64Array`, then maps slots to manifest names without one FFI call per state.

## Shared dynamic input ring

```c
int32_t spellwire_host_set_input_ring(
  SpellwireHost *host,
  int32_t *words,
  size_t word_len,
  size_t capacity
);
```

The ring layout is four atomic header words followed by `capacity * 6` event words:

```text
header: write, read, dropped, closed
record: device, code, edge, source, timestamp_ns_low, timestamp_ns_high
```

Capacity must be a power of two. Native code stores a record then release-stores `write`; the Bun SPSC consumer acquire-loads it. A full ring increments `dropped` and never overwrites unread events. Passing null detaches synchronously. Attached storage must remain live until detach or host stop; `NativeHost.attachDynamicLane()` retains the `SharedArrayBuffer` view for this lifetime.

## Compatibility engine API

Hosts that already own observation/injection may use the lower-level synchronous engine:

```c
SpellwireEngine *spellwire_engine_new(const uint8_t *bytes, size_t len);
void spellwire_engine_free(SpellwireEngine *engine);
int32_t spellwire_engine_set_output_callback(
  SpellwireEngine *engine,
  SpellwireOutputCallback callback,
  void *context
);
int32_t spellwire_engine_dispatch(
  SpellwireEngine *engine,
  uint8_t device,
  uint16_t code,
  uint8_t edge,
  uint8_t source
);
int32_t spellwire_engine_state_get(const SpellwireEngine *engine, size_t slot, int64_t *output);
int32_t spellwire_engine_state_set(SpellwireEngine *engine, size_t slot, int64_t value);
```

Its callback receives contiguous zero-delay batches:

```c
typedef struct {
  uint8_t kind;
  uint8_t flags;
  uint16_t code;
  int32_t x;
  int32_t y;
} SpellwireOutputEvent;
```

Kinds are 1 key, 2 mouse button, 3 relative move, and 4 wheel. `flags & 1` is the down state for keys/buttons. The compatibility dispatch waits synchronously for delay deadlines. Engine operations are serialized; concurrent/reentrant access returns `-6`, and free must not race another thread.

## Values and errors

```text
device: 0 keyboard, 1 mouse button
edge:   0 down, 1 up
source: 0 physical, 1 synthetic
```

Zero means success. Important host statuses are `-1` null, `-2` invalid argument, `-5` runtime failure, `-7` not running, `-8` already running, `-9` platform failure, `-10` channel failure, and `-11` continuation capacity exhausted. Status values avoid allocation on the event path; read the error string on the control plane.
