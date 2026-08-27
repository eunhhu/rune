import type { Key, Modifier, MouseButton } from "./keys";
import { InputSource } from "./keys";
import { parseHotkey } from "./hotkey";

export interface RealtimeOptions {
  source?: InputSource;
  /** Suppress the original physical transition when the native platform supports it. */
  consume?: boolean;
  /** Logical modifier groups required by this trigger. */
  modifiers?: Modifier;
  /** Reject additional logical modifiers. Default false for low-level on* registrations. */
  exactModifiers?: boolean;
  /** Run on OS key-repeat transitions. Default true. */
  repeat?: boolean;
  /** Native boolean-state gate. The compiler evaluates this before action and suppression. */
  when?: () => boolean;
}

export interface HotkeyOptions extends Omit<RealtimeOptions, "modifiers"> {
  /** Reject additional logical modifiers. Default true for string hotkeys. */
  exactModifiers?: boolean;
  /** Trigger transition. Default `down`. */
  edge?: "down" | "up";
}

export interface RemapOptions {
  source?: InputSource;
  repeat?: boolean;
  /** Native boolean-state gate applied to both source transitions. */
  when?: () => boolean;
}

export interface RealtimeRegistration {
  readonly device: "keyboard" | "mouse";
  readonly edge: "down" | "up";
  readonly code: number;
  readonly source: InputSource;
  readonly consume: boolean;
  readonly modifiers: number;
  readonly exactModifiers: boolean;
  readonly repeat: boolean;
  readonly when?: () => boolean;
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
    consume: options?.consume ?? false,
    modifiers: options?.modifiers ?? 0,
    exactModifiers: options?.exactModifiers ?? false,
    repeat: options?.repeat ?? true,
    ...(options?.when ? { when: options.when } : {}),
    handler: options?.when
      ? () => {
          if (options.when?.()) handler();
        }
      : handler,
  });
}

/**
 * Realtime handler markers.
 *
 * `spellwire compile` recognizes these calls and lowers their callback bodies to native
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

  /** Portable, statically compiled chord. String hotkeys consume and match exactly by default. */
  hotkey(chord: string, handler: () => void, options: HotkeyOptions = {}): void {
    const parsed = parseHotkey(chord);
    register(parsed.device, options.edge ?? "down", parsed.code, handler, {
      ...options,
      modifiers: parsed.modifiers,
      consume: options.consume ?? true,
      exactModifiers: options.exactModifiers ?? true,
    });
  },

  /** One-to-one keyboard remap with paired down/up transitions and original-input suppression. */
  remap(from: Key | string, to: Key | string, options: RemapOptions = {}): void {
    const sourceKey = remapKey(from, "source");
    const targetKey = remapKey(to, "target");
    const triggerOptions: RealtimeOptions = {
      source: options.source ?? InputSource.Physical,
      consume: true,
      repeat: options.repeat ?? true,
      ...(options.when ? { when: options.when } : {}),
    };
    register("keyboard", "down", sourceKey, () => keyDown(targetKey), triggerOptions);
    register("keyboard", "up", sourceKey, () => keyUp(targetKey), triggerOptions);
  },
});

function remapKey(value: Key | string, label: "source" | "target"): Key {
  if (typeof value === "number") {
    if (!Number.isInteger(value) || value < 0 || value > 0xff) {
      throw new RangeError(`remap ${label} key must be an integer between 0 and 255`);
    }
    return value;
  }
  const parsed = parseHotkey(value);
  if (parsed.device !== "keyboard" || parsed.modifiers !== 0) {
    throw new SyntaxError(`remap ${label} must name one keyboard key`);
  }
  return parsed.code as Key;
}

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

const MICROSECONDS_PER_MILLISECOND = 1_000;
const MICROSECONDS_PER_SECOND = 1_000_000;
const MICROSECONDS_PER_MINUTE = 60_000_000;
const MICROSECONDS_PER_HOUR = 3_600_000_000;

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
      "Spellwire realtime intrinsic executed outside a handler. Compile the script or run it through SpellwireRuntime.",
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

function sleepScaled(duration: number, scale: number, label: string): void {
  if (!Number.isSafeInteger(duration) || duration < 0) {
    throw new RangeError(`${label} duration must be a non-negative safe integer`);
  }
  const microseconds = duration * scale;
  if (!Number.isSafeInteger(microseconds)) {
    throw new RangeError(`${label} duration exceeds the safe integer microsecond range`);
  }
  sink().delayUs(microseconds);
}

export function sleepUs(duration: number): void {
  sleepScaled(duration, 1, "sleepUs");
}

export function sleepMs(duration: number): void {
  sleepScaled(duration, MICROSECONDS_PER_MILLISECOND, "sleepMs");
}

export function sleepSeconds(duration: number): void {
  sleepScaled(duration, MICROSECONDS_PER_SECOND, "sleepSeconds");
}

export function sleepMinutes(duration: number): void {
  sleepScaled(duration, MICROSECONDS_PER_MINUTE, "sleepMinutes");
}

export function sleepHours(duration: number): void {
  sleepScaled(duration, MICROSECONDS_PER_HOUR, "sleepHours");
}

/** Unit-based delay API. The AOT compiler lowers every member to one native delay opcode. */
export const sleep = Object.freeze({
  us: sleepUs,
  ms: sleepMs,
  seconds: sleepSeconds,
  minutes: sleepMinutes,
  hours: sleepHours,
});

export function keyHeld(key: Key): boolean {
  return sink().held("keyboard", key);
}

export function mouseHeld(button: MouseButton): boolean {
  return sink().held("mouse", button);
}
