import type { Key, MouseButton } from "./keys";
import { InputSource } from "./keys";

export interface RealtimeOptions {
  source?: InputSource;
}

export interface RealtimeRegistration {
  readonly device: "keyboard" | "mouse";
  readonly edge: "down" | "up";
  readonly code: number;
  readonly source: InputSource;
  readonly handler: () => void;
}

const fallbackRegistrations: RealtimeRegistration[] = [];

function register(
  device: RealtimeRegistration["device"],
  edge: RealtimeRegistration["edge"],
  code: number,
  handler: () => void,
  options?: RealtimeOptions,
): void {
  fallbackRegistrations.push({
    device,
    edge,
    code,
    source: options?.source ?? InputSource.Physical,
    handler,
  });
}

/**
 * Realtime handler markers.
 *
 * `rune compile` recognizes these calls and lowers their callback bodies to native
 * bytecode. Calling them without the compiler registers a JavaScript fallback; that
 * fallback preserves semantics but does not carry a realtime latency guarantee.
 */
export const rt = Object.freeze({
  onKeyDown(key: Key, handler: () => void, options?: RealtimeOptions): void {
    register("keyboard", "down", key, handler, options);
  },
  onKeyUp(key: Key, handler: () => void, options?: RealtimeOptions): void {
    register("keyboard", "up", key, handler, options);
  },
  onMouseDown(button: MouseButton, handler: () => void, options?: RealtimeOptions): void {
    register("mouse", "down", button, handler, options);
  },
  onMouseUp(button: MouseButton, handler: () => void, options?: RealtimeOptions): void {
    register("mouse", "up", button, handler, options);
  },
});

export function getFallbackRealtimeRegistrations(): readonly RealtimeRegistration[] {
  return fallbackRegistrations;
}

export interface RealtimeActionSink {
  key(code: number, down: boolean): void;
  mouseButton(button: number, down: boolean): void;
  mouseMove(dx: number, dy: number): void;
  mouseWheel(x: number, y: number): void;
  delayUs(duration: number): void;
  held(device: "keyboard" | "mouse", code: number): boolean;
}

let currentSink: RealtimeActionSink | undefined;

export function withRealtimeActionSink<T>(sink: RealtimeActionSink, body: () => T): T {
  const previous = currentSink;
  currentSink = sink;
  try {
    return body();
  } finally {
    currentSink = previous;
  }
}

function sink(): RealtimeActionSink {
  if (!currentSink) {
    throw new Error(
      "Rune realtime intrinsic executed outside a handler. Compile the script or run it through RuneRuntime.",
    );
  }
  return currentSink;
}

// These are ordinary TypeScript functions. The AOT compiler recognizes and replaces
// their calls; the fallback implementations make the same script debuggable in JS.
export function keyDown(key: Key): void {
  sink().key(key, true);
}

export function keyUp(key: Key): void {
  sink().key(key, false);
}

export function tapKey(key: Key): void {
  const target = sink();
  target.key(key, true);
  target.key(key, false);
}

export function mouseDown(button: MouseButton): void {
  sink().mouseButton(button, true);
}

export function mouseUp(button: MouseButton): void {
  sink().mouseButton(button, false);
}

export function clickMouse(button: MouseButton): void {
  const target = sink();
  target.mouseButton(button, true);
  target.mouseButton(button, false);
}

export function moveMouse(dx: number, dy: number): void {
  sink().mouseMove(dx, dy);
}

export function wheelMouse(x: number, y: number): void {
  sink().mouseWheel(x, y);
}

export function sleepUs(duration: number): void {
  sink().delayUs(duration);
}

export function keyHeld(key: Key): boolean {
  return sink().held("keyboard", key);
}

export function mouseHeld(button: MouseButton): boolean {
  return sink().held("mouse", button);
}
