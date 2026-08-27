import { SpscInt32Ring } from "./ring";

export enum InputDevice {
  Keyboard = 0,
  MouseButton = 1,
}

export enum InputEdge {
  Down = 0,
  Up = 1,
}

export enum EventSource {
  Physical = 0,
  Synthetic = 1,
}

export interface InputEvent {
  readonly device: InputDevice;
  readonly code: number;
  readonly edge: InputEdge;
  readonly source: EventSource;
  readonly timestampLo: number;
  readonly timestampHi: number;
}

export type InputHandler = (event: InputEvent) => void;

interface HandlerRegistration {
  readonly handler: InputHandler;
  removePending: boolean;
}

const EVENT_WORDS = 6;
export const RUNTIME_EVENT_WORDS = 20;

export const RuntimeEventKind = {
  State: 1,
  Effect: 2,
  Reload: 3,
} as const;

export type RuntimeEventKind = (typeof RuntimeEventKind)[keyof typeof RuntimeEventKind];
export type StateChangeHandler = (slot: number, value: bigint) => void;
export type RawEffectHandler = (
  id: number,
  length: number,
  record: Int32Array,
  payloadOffset: number,
) => void;

/**
 * Native-to-JavaScript SPSC lane for changed state and transient effects.
 * `drain()` allocates nothing; raw effect handlers must consume the reused record synchronously.
 */
export class RuntimeEventLane {
  readonly ring: SpscInt32Ring;
  readonly #record = new Int32Array(RUNTIME_EVENT_WORDS);
  readonly #stateHandlers = new Set<StateChangeHandler>();
  readonly #effectHandlers = new Set<RawEffectHandler>();
  readonly #reloadHandlers = new Set<() => void>();
  #draining = false;

  constructor(capacity = 1024, buffer?: SharedArrayBuffer) {
    this.ring = new SpscInt32Ring(capacity, RUNTIME_EVENT_WORDS, buffer);
  }

  onState(handler: StateChangeHandler): () => void {
    this.#stateHandlers.add(handler);
    return () => this.#stateHandlers.delete(handler);
  }

  onEffectRaw(handler: RawEffectHandler): () => void {
    this.#effectHandlers.add(handler);
    return () => this.#effectHandlers.delete(handler);
  }

  onReload(handler: () => void): () => void {
    this.#reloadHandlers.add(handler);
    return () => this.#reloadHandlers.delete(handler);
  }

  drain(maxEvents = 4096): number {
    if (this.#draining) throw new Error("runtime event lane drain is not reentrant");
    if (!Number.isSafeInteger(maxEvents) || maxEvents < 0) {
      throw new RangeError("maxEvents must be a non-negative safe integer");
    }
    this.#draining = true;
    let count = 0;
    try {
      while (count < maxEvents && this.ring.pop(this.#record)) {
        const kind = this.#record[0];
        if (kind === RuntimeEventKind.State) {
          const slot = this.#record[1] ?? 0;
          const value = joinI64(this.#record[4] ?? 0, this.#record[5] ?? 0);
          for (const handler of this.#stateHandlers) handler(slot, value);
        } else if (kind === RuntimeEventKind.Effect) {
          const id = this.#record[1] ?? 0;
          const length = this.#record[2] ?? 0;
          if (length >= 0 && length <= 8) {
            for (const handler of this.#effectHandlers) {
              handler(id, length, this.#record, 4);
            }
          }
        } else if (kind === RuntimeEventKind.Reload) {
          for (const handler of this.#reloadHandlers) handler();
        }
        count += 1;
      }
      return count;
    } finally {
      this.#draining = false;
    }
  }
}

export function readRuntimeEventI64(record: Int32Array, wordOffset: number): bigint {
  return joinI64(record[wordOffset] ?? 0, record[wordOffset + 1] ?? 0);
}

function joinI64(low: number, high: number): bigint {
  return BigInt.asIntN(64, (BigInt(high >>> 0) << 32n) | BigInt(low >>> 0));
}

/**
 * Best-effort JavaScript lane. A native producer writes fixed records to this ring;
 * a dedicated Bun worker can drain it without a native-to-JS callback per event.
 */
export class DynamicInputLane {
  readonly ring: SpscInt32Ring;
  readonly #handlers: HandlerRegistration[][];
  readonly #record = new Int32Array(EVENT_WORDS);
  #activeHandlers: HandlerRegistration[] | undefined;
  #draining = false;

  constructor(capacity = 1024, buffer?: SharedArrayBuffer) {
    this.ring = new SpscInt32Ring(capacity, EVENT_WORDS, buffer);
    this.#handlers = Array.from({ length: 2 * 2 * 256 }, () => []);
  }

  on(device: InputDevice, code: number, edge: InputEdge, handler: InputHandler): () => void {
    const bucket = this.#bucket(device, code, edge);
    const handlers = this.#handlers[bucket];
    if (!handlers) {
      throw new RangeError("input tuple is outside the dynamic lane range");
    }
    const registration: HandlerRegistration = { handler, removePending: false };
    handlers.push(registration);
    return () => {
      const index = handlers.indexOf(registration);
      if (index < 0 || registration.removePending) return;
      if (this.#activeHandlers === handlers) {
        registration.removePending = true;
      } else {
        handlers.splice(index, 1);
      }
    };
  }

  drain(maxEvents = 1024): number {
    if (this.#draining) throw new Error("dynamic input lane drain is not reentrant");
    if (!Number.isSafeInteger(maxEvents) || maxEvents < 0) {
      throw new RangeError("maxEvents must be a non-negative safe integer");
    }
    this.#draining = true;
    let count = 0;
    try {
      while (count < maxEvents && this.ring.pop(this.#record)) {
        const event: InputEvent = {
          device: this.#record[0] as InputDevice,
          code: this.#record[1] ?? 0,
          edge: this.#record[2] as InputEdge,
          source: this.#record[3] as EventSource,
          timestampLo: this.#record[4] ?? 0,
          timestampHi: this.#record[5] ?? 0,
        };
        const handlers = this.#handlers[this.#bucket(event.device, event.code, event.edge)];
        if (handlers) {
          const handlerCount = handlers.length;
          this.#activeHandlers = handlers;
          try {
            for (let index = 0; index < handlerCount; index += 1) {
              handlers[index]?.handler(event);
            }
          } finally {
            this.#activeHandlers = undefined;
            for (let index = handlers.length - 1; index >= 0; index -= 1) {
              if (handlers[index]?.removePending) handlers.splice(index, 1);
            }
          }
        }
        count += 1;
      }
      return count;
    } finally {
      this.#draining = false;
    }
  }

  #bucket(device: InputDevice, code: number, edge: InputEdge): number {
    if (device !== InputDevice.Keyboard && device !== InputDevice.MouseButton) return -1;
    if (edge !== InputEdge.Down && edge !== InputEdge.Up) return -1;
    if (!Number.isInteger(code) || code < 0 || code >= 256) return -1;
    return (device * 2 + edge) * 256 + code;
  }
}

export interface NativeStateBridge {
  getState(slot: number): bigint;
  setState(slot: number, value: bigint): void;
}

export class NativeState<T extends number | boolean = number> {
  constructor(
    readonly slot: number,
    readonly kind: "number" | "boolean",
    private readonly bridge: NativeStateBridge,
  ) {}

  get(): T {
    const value = this.bridge.getState(this.slot);
    return (this.kind === "boolean" ? value !== 0n : Number(value)) as T;
  }

  set(value: T): void {
    this.bridge.setState(
      this.slot,
      this.kind === "boolean" ? (value ? 1n : 0n) : BigInt(Math.trunc(value as number)),
    );
  }
}
