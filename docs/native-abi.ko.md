# 네이티브 C ABI

[English](native-abi.md)

`spellwire-native`는 `cdylib`와 `rlib`으로 빌드됩니다.

```bash
cargo build -p spellwire-native --release --locked
```

```text
Windows: target/release/spellwire_native.dll
macOS:   target/release/libspellwire_native.dylib
Linux:   target/release/libspellwire_native.so
```

## Version과 capability

```c
uint32_t spellwire_abi_version(void);       // 3
uint32_t spellwire_capabilities(void);
uint32_t spellwire_permission_status(void);
uint32_t spellwire_request_permissions(void);
```

```text
1 << 0  HostCallbackInjection
1 << 1  NativeObservation
1 << 2  NativeInjection
1 << 3  NativeOverlay       (reserved; renderer는 companion process)
1 << 4  HostLifecycle
1 << 5  NonBlockingDelay
```

현재 library는 `NativeOverlay`를 제외한 `0x37`을 반환합니다. permission bit는 `1 << 0` observe, `1 << 1` inject입니다.

## Owned platform host

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
size_t spellwire_host_last_error(const SpellwireHost *host, char *buffer, size_t capacity);
```

`start`는 OS injector/observer와 runtime worker 하나를 만듭니다. observer는 bounded channel에 publish하고 delayed handler는 fixed-capacity deadline scheduler로 yield합니다. `stop`은 observation과 pending continuation을 종료하고 host가 추적하는 synthetic key/button down을 해제합니다. `free`도 running host를 stop합니다.

reload는 synchronous입니다. low-level flag는 공통 slot을 positional하게 복사합니다. Bun wrapper는 false를 전달하고 manifest name/kind로 compatible value를 보존해 state 순서 변경에 의한 corruption을 막습니다.

`spellwire_host_last_error(host, NULL, 0)`은 NUL을 포함한 UTF-8 buffer length를 반환합니다. 두 번째 call로 최근 platform/runtime message를 복사합니다.

## Shared dynamic input ring

```c
int32_t spellwire_host_set_input_ring(
  SpellwireHost *host,
  int32_t *words,
  size_t word_len,
  size_t capacity
);
```

layout:

```text
header: write, read, dropped, closed
record: device, code, edge, source, timestamp_ns_low, timestamp_ns_high
```

header 4 word 뒤에 `capacity * 6` event word가 옵니다. capacity는 2의 거듭제곱이어야 합니다. native는 record를 저장한 뒤 `write`를 release-store하고 Bun consumer는 acquire-load합니다. full ring은 `dropped`를 증가시키며 unread event를 덮어쓰지 않습니다.

null pointer는 synchronous detach입니다. attached storage는 detach 또는 host stop까지 살아 있어야 하며 `NativeHost.attachDynamicLane()`은 이 lifetime 동안 `SharedArrayBuffer` view를 보관합니다.

## Compatibility engine

자체 observation/injection을 가진 host는 lower-level synchronous engine을 사용할 수 있습니다.

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

callback record:

```c
typedef struct {
  uint8_t kind;
  uint8_t flags;
  uint16_t code;
  int32_t x;
  int32_t y;
} SpellwireOutputEvent;
```

kind 1은 key, 2는 mouse button, 3은 relative move, 4는 wheel입니다. key/button에서 `flags & 1`은 down입니다. compatibility dispatch는 delay deadline까지 동기 대기합니다. engine operation은 serialized되며 concurrent/reentrant access는 `-6`입니다. 다른 thread 작업과 free를 race시키면 안 됩니다.

## 값과 error

```text
device: 0 keyboard, 1 mouse button
edge:   0 down, 1 up
source: 0 physical, 1 synthetic
```

0은 성공입니다. 주요 host status는 `-1` null, `-2` invalid argument, `-5` runtime failure, `-7` not running, `-8` already running, `-9` platform failure, `-10` channel failure, `-11` continuation capacity exhausted입니다. event path에서 allocation을 피하기 위해 status는 숫자로 반환하고 상세 error string은 control plane에서 읽습니다.
