# API Reference

Rune exposes two complementary layers:

1. the regular TypeScript builder API for simple macros;
2. the realtime TypeScript runtime for persistent state and control flow without JavaScript on the input hot path.

## Regular macro builder

### `macro(name, builder)`

Creates a native macro program. The builder executes only while the macro is defined.

```ts
const example = macro("example", (m) => {
  m.on.keyDown(Key.Q).run(m.key.tap(Key.E));
});
```

### Trigger API

```ts
m.on.keyDown(Key.Q)
m.on.keyUp(Key.Q)
m.on.mouseDown(MouseButton.Left)
m.on.mouseUp(MouseButton.Left)
```

Triggers are physical-input oriented by default. Source filtering is available for cases where synthetic events intentionally trigger other macros.

### Keyboard actions

```ts
m.key.down(Key.E)
m.key.up(Key.E)
m.key.tap(Key.E)
```

### Mouse actions

```ts
m.mouse.down(MouseButton.Left)
m.mouse.up(MouseButton.Left)
m.mouse.click(MouseButton.Left)
m.mouse.move(10, -5)
m.mouse.wheel(0, 1)
```

### Delay

```ts
m.delay.us(80)
```

Delays use absolute monotonic deadlines internally. Very short waits may use a spin tail to reduce scheduler overshoot.

### Compile-time repetition

```ts
m.repeat(3, m.key.tap(Key.E), m.delay.us(40))
```

This expands while the macro is built. For runtime-dependent loops, use `rt.load()`.

### Runtime lifecycle

```ts
rune.configure({ spinThresholdUs: 100 })
rune.load(program)
rune.start()
rune.stop()
```

Multiple programs may be loaded before the runtime is started.

## Realtime TypeScript

### `rt.load(fn)`

Compiles a constrained TypeScript function into Rune native bytecode.

```ts
rt.load(() => {
  let count = 0;

  on.keyDown(Key.Q, () => {
    count++;
    if (count === 3) {
      key.tap(Key.E);
      count = 0;
    }
  });
});
```

Top-level mutable variables inside the realtime program become persistent native state.

### Input registration

```ts
on.keyDown(Key.Q, handler)
on.keyUp(Key.Q, handler)
on.mouseDown(MouseButton.Left, handler)
on.mouseUp(MouseButton.Left, handler)
```

### Realtime intrinsics

```ts
key.down(Key.E)
key.up(Key.E)
key.tap(Key.E)

mouse.down(MouseButton.Left)
mouse.up(MouseButton.Left)
mouse.click(MouseButton.Left)
mouse.move(10, -5)
mouse.wheel(0, 1)

delay.us(50)
held(Key.LeftShift)
```

### Persistent state from the control plane

The Bun side may inspect or modify named persistent slots exposed by the compiled program:

```ts
rt.state("combo")
rt.setState("combo", 0)
```

Use this for UI/configuration synchronization rather than for every input event.

## Keys

Rune public key identifiers follow the USB HID keyboard usage page. Common examples:

```ts
Key.A
Key.Q
Key.Digit1
Key.Space
Key.Enter
Key.Escape
Key.LeftShift
Key.LeftControl
Key.LeftAlt
Key.LeftMeta
Key.F1
Key.ArrowUp
```

This keeps scripts platform-independent; native backends translate USB HID-style Rune keys to Windows, macOS, or Linux representations.

## Mouse buttons

```ts
MouseButton.Left
MouseButton.Right
MouseButton.Middle
MouseButton.Back
MouseButton.Forward
```

## Performance rule of thumb

Use the regular builder when the entire sequence is known at load time. Use realtime TypeScript when execution depends on persistent state, held inputs, branches, loops, or reusable functions.

Keep unrestricted work such as network requests, filesystem access, logging pipelines, and complex UI operations in ordinary Bun code outside the realtime program.
