# Native C ABI

`rune-native` is built as a `cdylib` and `rlib`. The C ABI is a small host-driven boundary around the native VM.

Build it:

```bash
cargo build -p rune-native --release
```

Artifacts are named according to the platform:

```text
Windows: target/release/rune_native.dll
macOS:   target/release/librune_native.dylib
Linux:   target/release/librune_native.so
```

## Version and capabilities

```c
uint32_t rune_abi_version(void);
uint32_t rune_capabilities(void);
```

The current ABI version is `2`.

Capability bits:

```text
1 << 0  HostCallbackInjection
1 << 1  NativeObservation
1 << 2  NativeInjection
1 << 3  NativeOverlay
```

The current implementation returns only `HostCallbackInjection`.

## Engine lifecycle

```c
RuneEngine *rune_engine_new(const uint8_t *bytes, size_t len);
void rune_engine_free(RuneEngine *engine);
```

`rune_engine_new` decodes and validates a complete `.rune.bin` buffer and copies the program into owned native memory. It returns null on invalid input.

The engine pointer is single-owner and must be freed exactly once.

## Output callback

```c
typedef int32_t (*RuneOutputCallback)(
  void *context,
  const RuneOutputEvent *events,
  size_t event_count
);

int32_t rune_engine_set_output_callback(
  RuneEngine *engine,
  RuneOutputCallback callback,
  void *context
);
```

Each callback receives one contiguous zero-delay output batch. Returning zero reports success; any non-zero status aborts the current dispatch as an injection error.

`RuneOutputEvent` is a fixed C representation:

```c
typedef struct {
  uint8_t kind;
  uint8_t flags;
  uint16_t code;
  int32_t x;
  int32_t y;
} RuneOutputEvent;
```

Kinds:

| `kind` | Meaning | Fields |
| ---: | --- | --- |
| 1 | key | `code`, `flags & 1` is down |
| 2 | mouse button | `code`, `flags & 1` is down |
| 3 | relative mouse move | `x`, `y` |
| 4 | mouse wheel | `x`, `y` |

A null callback is allowed; output batches are then discarded.

## Explicit event dispatch

```c
int32_t rune_engine_dispatch(
  RuneEngine *engine,
  uint8_t device,
  uint16_t code,
  uint8_t edge,
  uint8_t source
);
```

Values:

```text
device: 0 keyboard, 1 mouse button
edge:   0 down, 1 up
source: 0 physical, 1 synthetic
```

The function updates held-input state, matches triggers, executes bytecode, waits for delays, and invokes the output callback. The host must not call the same engine concurrently.

The ABI does **not** currently start a platform observer thread. The host is responsible for obtaining input events and translating output callback records to its platform injection API.

## Persistent state

```c
int32_t rune_engine_state_get(
  const RuneEngine *engine,
  size_t slot,
  int64_t *output
);

int32_t rune_engine_state_set(
  RuneEngine *engine,
  size_t slot,
  int64_t value
);
```

Use the compiler-generated JSON manifest to map source variable names to slots. State access must not race with dispatch on the same engine; the current ABI does not add a mutex or atomic state layer.

## Error handling

The ABI uses null pointers and negative integer status codes rather than allocating error strings on the dispatch path. Hosts should validate the binary/manifest at load time and translate status codes outside latency-sensitive code.
