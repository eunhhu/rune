# 실시간 TypeScript 런타임

[English](typescript-runtime.md)

Spellwire는 TypeScript를 authoring language로 사용하고 지연에 민감한 subset을 AOT compile합니다. 별도 `rt.load()` wrapper는 필요하지 않으며 `.spellwire.ts` module 자체가 compilation unit입니다.

## Compile 경계

compiler는 top-level의 다음 call을 찾습니다.

```ts
rt.onKeyDown(Key.Q, () => {})
rt.onKeyUp(Key.Q, () => {})
rt.onMouseDown(MouseButton.Left, () => {})
rt.onMouseUp(MouseButton.Left, () => {})
```

handler에 필요한 code만 native bytecode로 lowering합니다. 다른 top-level TypeScript는 control-plane code로 함께 존재할 수 있지만 handler가 compiler가 표현할 수 없는 dynamic value를 capture할 수는 없습니다.

## 영속 상태와 constant

compile-time 정수 또는 boolean으로 초기화한 module-scope mutable `let`을 realtime handler가 참조하면 persistent native state slot이 됩니다.

```ts
let count = 0;
let enabled = true;

rt.onKeyDown(Key.Q, () => {
  if (!enabled) return;
  count += 1;
});
```

값은 dispatch 사이에 유지됩니다. manifest는 source name과 native slot/kind mapping을 기록합니다. live reload는 이름과 kind가 같은 state를 보존하므로 선언 순서 변경이 값을 뒤섞지 않습니다. module-scope `const`는 가능하면 compile-time constant로 fold됩니다.

## 값과 local

realtime number는 signed 64-bit integer입니다. numeric literal은 compile 시 safe JavaScript integer여야 합니다. boolean은 native integer truth value로 표현됩니다.

handler/helper local은 고정 VM local slot으로 compile됩니다.

```ts
rt.onKeyDown(Key.Q, () => {
  let next = count + 1;
  count = next;
});
```

단순 assignment, compound assignment, prefix/postfix increment/decrement를 지원합니다. destructuring과 dynamic property assignment는 지원하지 않습니다.

## 연산과 제어 흐름

정수 산술, signed bit operation/shift, equality/ordering, boolean logic, short-circuit를 지원합니다. unsigned right shift `>>>`는 signed i64 모델과 맞지 않아 거부합니다.

```ts
if (enabled && count < 10) {
  count++;
} else {
  count = 0;
}
```

`if`/`else`, early `return`, conditional expression, `for`, `while`, `do/while`, `break`, `continue`를 지원합니다. 반복 실행은 handler instruction budget으로 제한되어 event path를 보호합니다.

## Helper 함수

handler가 호출하는 top-level helper는 inline compile됩니다.

```ts
function tapMany(key: Key, count: number): void {
  for (let index = 0; index < count; index++) {
    tapKey(key);
  }
}

rt.onKeyDown(Key.Q, () => {
  tapMany(Key.E, 3);
});
```

helper는 void여야 하고 recursion할 수 없으며 lowering 가능한 argument/body만 사용해야 합니다. inline은 runtime call overhead를 없애지만 bytecode size를 늘릴 수 있습니다.

## Realtime intrinsic

```ts
keyDown(Key.E)
keyUp(Key.E)
tapKey(Key.E)
mouseDown(MouseButton.Left)
mouseUp(MouseButton.Left)
clickMouse(MouseButton.Left)
moveMouse(4, -2)
wheelMouse(0, 1)
sleepUs(75)
keyHeld(Key.LeftShift)
mouseHeld(MouseButton.Right)
```

compiler는 이 이름을 native opcode로 변환합니다.

## Delay

`sleepUs(n)`은 pending output batch를 flush하고 absolute monotonic deadline을 진행합니다. live host는 continuation을 fixed-capacity queue에 저장했다가 deadline에 resume하므로 그동안 다른 input과 control command를 받을 수 있습니다. zero-duration yield는 같은 poll에서 resume할 수 있습니다.

compatibility engine과 simulator는 동기 waiting을 유지합니다. 비차단 live observation에는 `NativeHost`를 사용하십시오. desktop OS는 hard realtime scheduler가 아니므로 microsecond 문법은 요청 deadline이지 물리 end-to-end 보장이 아닙니다.

## 의도적으로 지원하지 않는 기능

- 별도 floating-point semantics
- realtime expression의 string
- dynamic object, array, map, set
- destructuring
- `async`, `await`, `Promise`
- 일반 제어 흐름용 exception
- generator
- 임의 npm/Bun API
- network/file I/O
- dynamic property access
- unsigned right shift `>>>`
- runtime-created closure
- non-void helper return

이 작업은 일반 Bun control plane으로 옮기고 native host와 제한된 상태/configuration만 교환하십시오.

## Resource limit

| Resource | 값 |
| --- | ---: |
| 기본 stack limit | 128 values |
| native 최대 stack | 256 values |
| native 최대 locals | 256 values |
| native output batch | 64 events |
| live pending continuations | 64 |
| 기본 instruction budget | handler당 100,000 instructions |

program은 dispatch 전에 검증됩니다. invalid jump, slot, entry, limit, empty program은 load 중 거부됩니다.

## Fallback 실행

`.spellwire.ts`를 Bun으로 직접 실행하면 `rt.on*` registration이 JavaScript fallback list에 기록됩니다. `withRealtimeActionSink()`를 사용하면 test가 handler를 호출하고 action을 관찰할 수 있습니다.

fallback은 debugging에 유용하지만 native AOT path가 아니며 realtime latency guarantee가 없습니다.
