import { existsSync, watch as watchDirectory, type FSWatcher } from "node:fs";
import { basename, dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { dlopen, FFIType, suffix, type Pointer } from "bun:ffi";

import { compileSource } from "./compiler/compiler";
import { encodeModule } from "./compiler/encode";
import {
  DynamicInputLane,
  NativeState,
  RuntimeEventLane,
  readRuntimeEventI64,
  type NativeStateBridge,
} from "./runtime";

export const NATIVE_ABI_VERSION = 5;

export const NativeCapability = {
  HostCallbackInjection: 1 << 0,
  NativeObservation: 1 << 1,
  NativeInjection: 1 << 2,
  NativeOverlay: 1 << 3,
  HostLifecycle: 1 << 4,
  NonBlockingDelay: 1 << 5,
  NativeInputSuppression: 1 << 6,
  NativeEventLane: 1 << 7,
} as const;

export const NativePermission = {
  Observe: 1 << 0,
  Inject: 1 << 1,
} as const;

export interface NativeStateManifestEntry {
  readonly slot: number;
  readonly kind: "number" | "boolean";
}

export interface NativeEffectField {
  readonly name: string;
  readonly kind: "number" | "boolean";
}

export interface NativeEffectManifestEntry {
  readonly id: number;
  readonly fields: readonly NativeEffectField[];
}

export type NativeEffectPayload = Readonly<Record<string, number | boolean>>;
export type NativeEffectHandler<T extends NativeEffectPayload = NativeEffectPayload> =
  (payload: T) => void;
export type NativeRawEffectHandler = (
  record: Int32Array,
  payloadOffset: number,
  length: number,
) => void;

export interface NativeManifest {
  readonly version: number;
  readonly input?: string;
  readonly binary?: string;
  readonly handlers?: number;
  readonly instructions?: number;
  readonly states: Readonly<Record<string, NativeStateManifestEntry>>;
  readonly effects: Readonly<Record<string, NativeEffectManifestEntry>>;
}

export interface NativeHostOptions {
  readonly nativeLibraryPath?: string;
  readonly manifestPath?: string;
  /** Native-to-Bun state/effect ring capacity. Must be a power of two. Default: 1024. */
  readonly eventCapacity?: number;
  /** Drain interval while at least one state/effect subscriber exists. Default: 4 ms. */
  readonly eventPollIntervalMs?: number;
}

export interface NativeWatchOptions {
  readonly debounceMs?: number;
  readonly preserveState?: boolean;
  readonly onReload?: () => void;
  readonly onError?: (error: Error) => void;
}

export interface NativeHostWatcher {
  close(): void;
}

export interface NativeRuntimeInfo {
  readonly abiVersion: number;
  readonly capabilities: number;
  readonly permissions: number;
  readonly nativeLibraryPath: string;
}

export type NativeStateSnapshot = Readonly<Record<string, number | boolean>>;

const EMPTY_STATE_SNAPSHOT: NativeStateSnapshot = Object.freeze({});

export interface ProgramDescriptor {
  readonly bytes: Uint8Array;
  readonly manifest: NativeManifest;
}

const symbols = {
  spellwire_abi_version: { args: [], returns: FFIType.u32 },
  spellwire_capabilities: { args: [], returns: FFIType.u32 },
  spellwire_permission_status: { args: [], returns: FFIType.u32 },
  spellwire_request_permissions: { args: [], returns: FFIType.u32 },
  spellwire_host_new: { args: [FFIType.ptr, "usize"], returns: FFIType.ptr },
  spellwire_host_free: { args: [FFIType.ptr], returns: FFIType.void },
  spellwire_host_start: { args: [FFIType.ptr], returns: FFIType.i32 },
  spellwire_host_stop: { args: [FFIType.ptr], returns: FFIType.i32 },
  spellwire_host_reload: {
    args: [FFIType.ptr, FFIType.ptr, "usize", FFIType.bool],
    returns: FFIType.i32,
  },
  spellwire_host_dispatch: {
    args: [FFIType.ptr, FFIType.u8, FFIType.u16, FFIType.u8, FFIType.u8],
    returns: FFIType.i32,
  },
  spellwire_host_state_get: {
    args: [FFIType.ptr, "usize", FFIType.ptr],
    returns: FFIType.i32,
  },
  spellwire_host_state_set: {
    args: [FFIType.ptr, "usize", FFIType.i64],
    returns: FFIType.i32,
  },
  spellwire_host_state_snapshot: {
    args: [FFIType.ptr, FFIType.ptr, "usize"],
    returns: FFIType.i32,
  },
  spellwire_host_set_input_ring: {
    args: [FFIType.ptr, FFIType.ptr, "usize", "usize"],
    returns: FFIType.i32,
  },
  spellwire_host_set_event_ring: {
    args: [FFIType.ptr, FFIType.ptr, "usize", "usize"],
    returns: FFIType.i32,
  },
  spellwire_host_last_error: {
    args: [FFIType.ptr, FFIType.ptr, "usize"],
    returns: "usize",
  },
} as const;

function openNativeLibrary(path: string) {
  return dlopen(path, symbols);
}

type LoadedNativeLibrary = ReturnType<typeof openNativeLibrary>;
type NativePointer = Pointer | bigint;

const moduleDirectory = dirname(fileURLToPath(import.meta.url));

export function nativeLibraryFileName(): string {
  return process.platform === "win32" ? `spellwire_native.${suffix}` : `libspellwire_native.${suffix}`;
}

export function resolveNativeLibrary(explicitPath?: string): string {
  const fileName = nativeLibraryFileName();
  const platformDirectory = `${process.platform}-${process.arch}`;
  const candidates = [
    explicitPath,
    process.env.SPELLWIRE_NATIVE_LIBRARY,
    join(moduleDirectory, "..", "native", platformDirectory, fileName),
    join(moduleDirectory, "..", "..", "..", "target", "release", fileName),
    join(moduleDirectory, "..", "..", "..", "target", "debug", fileName),
  ].filter((candidate): candidate is string => typeof candidate === "string" && candidate.length > 0);

  for (const candidate of candidates) {
    const absolute = resolve(candidate);
    if (existsSync(absolute)) return absolute;
  }
  throw new Error(
    `Spellwire native library not found (${fileName}). Build it with ` +
      "`bun run build:native` or set SPELLWIRE_NATIVE_LIBRARY.",
  );
}

export function inspectNativeRuntime(
  options: { nativeLibraryPath?: string; requestPermissions?: boolean } = {},
): NativeRuntimeInfo {
  const nativeLibraryPath = resolveNativeLibrary(options.nativeLibraryPath);
  const library = openNativeLibrary(nativeLibraryPath);
  try {
    const abiVersion = library.symbols.spellwire_abi_version();
    const capabilities = library.symbols.spellwire_capabilities();
    const permissions = options.requestPermissions
      ? library.symbols.spellwire_request_permissions()
      : library.symbols.spellwire_permission_status();
    return { abiVersion, capabilities, permissions, nativeLibraryPath };
  } finally {
    library.close();
  }
}

export async function loadProgramDescriptor(
  inputPath: string,
  manifestPath?: string,
): Promise<ProgramDescriptor> {
  const absolute = resolve(inputPath);
  if (!(await Bun.file(absolute).exists())) {
    throw new Error(`Spellwire program does not exist: ${absolute}`);
  }
  if (absolute.endsWith(".ts")) {
    const result = compileSource(await Bun.file(absolute).text(), { fileName: absolute });
    return {
      bytes: encodeModule(result.module),
      manifest: {
        version: 1,
        input: absolute,
        handlers: result.module.handlers.length,
        instructions: result.module.code.length,
        states: Object.fromEntries(
          result.module.states.map((state) => [
            state.name,
            { slot: state.slot, kind: state.kind },
          ]),
        ),
        effects: Object.fromEntries(
          result.module.effects.map((effect) => [
            effect.name,
            { id: effect.id, fields: effect.fields },
          ]),
        ),
      },
    };
  }

  const bytes = new Uint8Array(await Bun.file(absolute).arrayBuffer());
  const manifestAbsolute = resolve(manifestPath ?? `${absolute}.json`);
  if (!(await Bun.file(manifestAbsolute).exists())) {
    throw new Error(`Spellwire manifest does not exist: ${manifestAbsolute}`);
  }
  const manifest = validateManifest(await Bun.file(manifestAbsolute).json(), manifestAbsolute);
  return { bytes, manifest };
}

export class NativeEffects {
  readonly #manifest: () => NativeManifest;
  readonly #retain: () => () => void;
  readonly #handlers = new Map<string, Set<NativeEffectHandler>>();
  readonly #rawHandlers = new Map<string, Set<NativeRawEffectHandler>>();
  #byId = new Map<number, readonly [string, NativeEffectManifestEntry]>();

  constructor(manifest: () => NativeManifest, retain: () => () => void) {
    this.#manifest = manifest;
    this.#retain = retain;
    this.refresh();
  }

  on<T extends NativeEffectPayload = NativeEffectPayload>(
    name: string,
    handler: NativeEffectHandler<T>,
  ): () => void {
    this.#entry(name);
    const handlers = this.#handlers.get(name) ?? new Set<NativeEffectHandler>();
    this.#handlers.set(name, handlers);
    const registration: NativeEffectHandler = (payload) => handler(payload as T);
    handlers.add(registration);
    const release = this.#retain();
    return () => {
      if (!handlers.delete(registration)) return;
      if (handlers.size === 0) this.#handlers.delete(name);
      release();
    };
  }

  /** Lower-allocation subscription. Values are ordered exactly like the declared schema. */
  onRaw(name: string, handler: NativeRawEffectHandler): () => void {
    this.#entry(name);
    const handlers = this.#rawHandlers.get(name) ?? new Set<NativeRawEffectHandler>();
    this.#rawHandlers.set(name, handlers);
    const registration: NativeRawEffectHandler = (record, offset, length) => {
      handler(record, offset, length);
    };
    handlers.add(registration);
    const release = this.#retain();
    return () => {
      if (!handlers.delete(registration)) return;
      if (handlers.size === 0) this.#rawHandlers.delete(name);
      release();
    };
  }

  dispatch(id: number, length: number, record: Int32Array, offset: number): void {
    const manifestEntry = this.#byId.get(id);
    if (!manifestEntry) return;
    const [name, entry] = manifestEntry;
    if (length !== entry.fields.length) return;
    const rawHandlers = this.#rawHandlers.get(name);
    const structuredHandlers = this.#handlers.get(name);
    if (!rawHandlers && !structuredHandlers) return;
    if (rawHandlers) {
      for (const handler of rawHandlers) handler(record, offset, length);
    }
    if (structuredHandlers) {
      const payload = Object.create(null) as Record<string, number | boolean>;
      for (let index = 0; index < entry.fields.length; index += 1) {
        const field = entry.fields[index];
        if (!field) continue;
        const value = readRuntimeEventI64(record, offset + index * 2);
        payload[field.name] = field.kind === "boolean" ? value !== 0n : Number(value);
      }
      const frozen = Object.freeze(payload);
      for (const handler of structuredHandlers) handler(frozen);
    }
  }

  refresh(): void {
    this.#byId = new Map(
      Object.entries(this.#manifest().effects).map(([name, entry]) => [
        entry.id,
        [name, entry] as const,
      ]),
    );
  }

  #entry(name: string): NativeEffectManifestEntry {
    const effects = this.#manifest().effects;
    const entry = Object.hasOwn(effects, name) ? effects[name] : undefined;
    if (!entry) throw new RangeError(`native effect ${JSON.stringify(name)} does not exist`);
    return entry;
  }
}

export class NativeHost implements NativeStateBridge {
  readonly inputPath: string;
  readonly nativeLibraryPath: string;
  readonly capabilities: number;
  states: Readonly<Record<string, NativeState<number | boolean>>>;
  readonly effects: NativeEffects;
  readonly events: RuntimeEventLane;

  readonly #library: LoadedNativeLibrary;
  readonly #manifestPath: string | undefined;
  #host: NativePointer | null;
  #manifest: NativeManifest;
  #running = false;
  #closed = false;
  #inputLane: DynamicInputLane | null = null;
  #inputWords: Int32Array | null = null;
  #eventWords: Int32Array | null = null;
  #eventAttached = false;
  #manualEvents = false;
  #stateSnapshot = new BigInt64Array(0);
  #stateSnapshotCache = new BigInt64Array(0);
  #stateSnapshotValue: NativeStateSnapshot = EMPTY_STATE_SNAPSHOT;
  #stateCacheValid = false;
  #stateValueDirty = true;
  #eventDropped = 0;
  #stateRevision = 0;
  #eventConsumers = 0;
  #pollingEvents = false;
  #eventTimer: ReturnType<typeof setInterval> | undefined;
  readonly #eventPollIntervalMs: number;
  readonly #stateHandlers = new Set<(snapshot: NativeStateSnapshot) => void>();
  #reloadTail: Promise<void> = Promise.resolve();

  private constructor(
    inputPath: string,
    manifestPath: string | undefined,
    nativeLibraryPath: string,
    descriptor: ProgramDescriptor,
    eventCapacity: number,
    eventPollIntervalMs: number,
  ) {
    this.inputPath = resolve(inputPath);
    this.#manifestPath = manifestPath;
    this.nativeLibraryPath = nativeLibraryPath;
    if (!Number.isSafeInteger(eventPollIntervalMs) || eventPollIntervalMs < 1 || eventPollIntervalMs > 1_000) {
      throw new RangeError("eventPollIntervalMs must be an integer between 1 and 1000");
    }
    this.events = new RuntimeEventLane(eventCapacity);
    this.#eventPollIntervalMs = eventPollIntervalMs;
    this.#library = openNativeLibrary(nativeLibraryPath);
    const abi = this.#library.symbols.spellwire_abi_version();
    if (abi !== NATIVE_ABI_VERSION) {
      this.#library.close();
      throw new Error(`Spellwire native ABI ${abi} is incompatible; expected ${NATIVE_ABI_VERSION}`);
    }
    this.capabilities = this.#library.symbols.spellwire_capabilities();
    this.#host = this.#library.symbols.spellwire_host_new(
      descriptor.bytes,
      descriptor.bytes.byteLength,
    );
    if (this.#host === null) {
      this.#library.close();
      throw new Error("Spellwire native host rejected the compiled program");
    }
    this.#manifest = descriptor.manifest;
    this.states = this.#createStates(descriptor.manifest);
    this.effects = new NativeEffects(
      () => this.#manifest,
      () => this.#retainEventConsumer(),
    );
    this.events.onState((slot, value) => this.#applyStateChange(slot, value));
    this.events.onEffectRaw((id, length, record, offset) => {
      this.effects.dispatch(id, length, record, offset);
    });
    this.events.onReload(() => {
      this.#stateCacheValid = false;
      this.#stateValueDirty = true;
      this.#stateRevision += 1;
    });
  }

  static async load(inputPath: string, options: NativeHostOptions = {}): Promise<NativeHost> {
    const descriptor = await loadProgramDescriptor(inputPath, options.manifestPath);
    const libraryPath = resolveNativeLibrary(options.nativeLibraryPath);
    return new NativeHost(
      inputPath,
      options.manifestPath,
      libraryPath,
      descriptor,
      options.eventCapacity ?? 1024,
      options.eventPollIntervalMs ?? 4,
    );
  }

  get running(): boolean {
    return this.#running;
  }

  get manifest(): NativeManifest {
    return this.#manifest;
  }

  permissionStatus(): number {
    this.#assertOpen();
    return this.#library.symbols.spellwire_permission_status();
  }

  requestPermissions(): number {
    this.#assertOpen();
    return this.#library.symbols.spellwire_request_permissions();
  }

  start(): void {
    this.#assertOpen();
    if (this.#running) return;
    this.#checkStatus(this.#library.symbols.spellwire_host_start(this.#requiredHost()), "start");
    this.#running = true;
    this.events.ring.clear();
    if (this.#inputLane !== null) {
      try {
        this.#setInputRing(this.#inputLane);
      } catch (error) {
        this.#library.symbols.spellwire_host_stop(this.#requiredHost());
        this.#running = false;
        throw error;
      }
    }
    try {
      this.#refreshStateCache();
      if (this.#eventConsumers > 0 || this.#manualEvents) this.#attachEventRing();
      this.#startEventPump();
    } catch (error) {
      this.#library.symbols.spellwire_host_stop(this.#requiredHost());
      this.#running = false;
      throw error;
    }
  }

  stop(): void {
    this.#assertOpen();
    if (!this.#running) return;
    this.#checkStatus(this.#library.symbols.spellwire_host_stop(this.#requiredHost()), "stop");
    this.#running = false;
    this.#inputWords = null;
    this.#eventWords = null;
    this.#eventAttached = false;
    this.events.ring.clear();
    this.#eventDropped = this.events.ring.dropped;
    this.#stopEventPump();
  }

  /** Publishes observed inputs into a shared ring without a native-to-JavaScript callback. */
  attachDynamicLane(lane: DynamicInputLane): void {
    this.#assertOpen();
    if (lane.ring.recordWords !== 6) {
      throw new RangeError("Spellwire dynamic input lanes must use six-word event records");
    }
    this.#inputLane = lane;
    if (this.#running) this.#setInputRing(lane);
  }

  detachDynamicLane(): void {
    this.#assertOpen();
    if (this.#running) {
      this.#checkStatus(
        this.#library.symbols.spellwire_host_set_input_ring(
          this.#requiredHost(),
          null,
          0,
          0,
        ),
        "detach input ring",
      );
    }
    this.#inputWords = null;
    this.#inputLane = null;
  }

  reload(options: { preserveState?: boolean } = {}): Promise<void> {
    this.#assertOpen();
    const task = this.#reloadTail.then(() =>
      this.#performReload(options.preserveState ?? true),
    );
    this.#reloadTail = task.catch(() => undefined);
    return task;
  }

  watch(options: NativeWatchOptions = {}): NativeHostWatcher {
    this.#assertOpen();
    const debounceMs = options.debounceMs ?? 75;
    if (!Number.isSafeInteger(debounceMs) || debounceMs < 0) {
      throw new RangeError("debounceMs must be a non-negative safe integer");
    }
    let timer: ReturnType<typeof setTimeout> | undefined;
    const watchedName = basename(this.inputPath);
    const watcher: FSWatcher = watchDirectory(dirname(this.inputPath), (_event, fileName) => {
      if (fileName !== null && fileName.toString() !== watchedName) return;
      if (timer !== undefined) clearTimeout(timer);
      timer = setTimeout(() => {
        timer = undefined;
        void this.reload({ preserveState: options.preserveState ?? true }).then(
          options.onReload,
          (error: unknown) => options.onError?.(toError(error)),
        );
      }, debounceMs);
    });
    return {
      close(): void {
        if (timer !== undefined) clearTimeout(timer);
        watcher.close();
      },
    };
  }

  state(name: string): NativeState<number | boolean> {
    const state = Object.hasOwn(this.states, name) ? this.states[name] : undefined;
    if (!state) throw new RangeError(`native state ${JSON.stringify(name)} does not exist`);
    return state;
  }

  /** Reads every named state for one state-driven UI reconciliation pass. */
  snapshotStates(): NativeStateSnapshot {
    this.#assertOpen();
    if (this.#eventAttached && !this.#pollingEvents) {
      this.#drainEvents(this.events.ring.capacity);
    }
    const entries = Object.entries(this.#manifest.states);
    if (entries.length === 0) return EMPTY_STATE_SNAPSHOT;
    const required = entries.reduce(
      (maximum, [, entry]) => Math.max(maximum, entry.slot + 1),
      0,
    );
    if (
      !this.#eventAttached ||
      !this.#stateCacheValid ||
      this.#stateSnapshotCache.length !== required
    ) {
      this.#refreshStateCache();
    }
    return this.#snapshotFromCache();
  }

  /** Drains native state/effect records synchronously. The hot path itself never calls JS. */
  pollEvents(maxEvents = 4096): number {
    this.#assertOpen();
    if (!Number.isSafeInteger(maxEvents) || maxEvents < 0) {
      throw new RangeError("maxEvents must be a non-negative safe integer");
    }
    if (!this.#running) throw new Error("Spellwire native host is not running");
    if (!this.#eventAttached) {
      this.#manualEvents = true;
      this.#attachEventRing();
      this.#refreshStateCache();
    }
    return this.#drainEvents(maxEvents);
  }

  #drainEvents(maxEvents: number): number {
    if (this.#pollingEvents) return 0;
    this.#pollingEvents = true;
    try {
      const revision = this.#stateRevision;
      const dropped = this.events.ring.dropped;
      if (dropped !== this.#eventDropped) {
        this.#eventDropped = dropped;
        this.#stateCacheValid = false;
        this.#stateRevision += 1;
      }
      const count = this.events.drain(maxEvents);
      if (this.#stateHandlers.size !== 0 && this.#stateRevision !== revision) {
        const snapshot = this.#snapshotFromCache();
        for (const handler of this.#stateHandlers) handler(snapshot);
      }
      return count;
    } finally {
      this.#pollingEvents = false;
    }
  }

  onStateChange(handler: (snapshot: NativeStateSnapshot) => void): () => void {
    this.#assertOpen();
    const registration = (snapshot: NativeStateSnapshot): void => handler(snapshot);
    this.#stateHandlers.add(registration);
    const release = this.#retainEventConsumer();
    return () => {
      if (!this.#stateHandlers.delete(registration)) return;
      release();
    };
  }

  getState(slot: number): bigint {
    this.#assertOpen();
    const output = new BigInt64Array(1);
    this.#checkStatus(
      this.#library.symbols.spellwire_host_state_get(this.#requiredHost(), slot, output),
      "read state",
    );
    return output[0] ?? 0n;
  }

  setState(slot: number, value: bigint): void {
    this.#assertOpen();
    this.#checkStatus(
      this.#library.symbols.spellwire_host_state_set(this.#requiredHost(), slot, value),
      "write state",
    );
  }

  dispatch(device: number, code: number, edge: number, source: number): void {
    this.#assertOpen();
    this.#checkStatus(
      this.#library.symbols.spellwire_host_dispatch(
        this.#requiredHost(),
        device,
        code,
        edge,
        source,
      ),
      "dispatch input",
    );
  }

  close(): void {
    if (this.#closed) return;
    if (this.#running) this.stop();
    this.#inputLane = null;
    this.#inputWords = null;
    const host = this.#host;
    this.#host = null;
    if (host !== null) this.#library.symbols.spellwire_host_free(host);
    this.#library.close();
    this.#closed = true;
  }

  async #performReload(preserveState: boolean): Promise<void> {
    const preserved = new Map<string, { kind: "number" | "boolean"; value: bigint }>();
    if (preserveState && this.#running) {
      for (const [name, entry] of Object.entries(this.#manifest.states)) {
        preserved.set(name, { kind: entry.kind, value: this.getState(entry.slot) });
      }
    }
    const descriptor = await loadProgramDescriptor(this.inputPath, this.#manifestPath);
    this.#checkStatus(
      this.#library.symbols.spellwire_host_reload(
        this.#requiredHost(),
        descriptor.bytes,
        descriptor.bytes.byteLength,
        false,
      ),
      "reload",
    );
    this.#manifest = descriptor.manifest;
    this.effects.refresh();
    this.states = this.#createStates(descriptor.manifest);
    this.#stateSnapshotCache = new BigInt64Array(0);
    this.#stateSnapshotValue = EMPTY_STATE_SNAPSHOT;
    this.#stateCacheValid = false;
    this.#stateValueDirty = true;
    if (this.#running) {
      for (const [name, entry] of Object.entries(descriptor.manifest.states)) {
        const previous = preserved.get(name);
        if (previous?.kind === entry.kind) this.setState(entry.slot, previous.value);
      }
    }
  }

  #createStates(
    manifest: NativeManifest,
  ): Readonly<Record<string, NativeState<number | boolean>>> {
    const states = Object.create(null) as Record<string, NativeState<number | boolean>>;
    for (const [name, entry] of Object.entries(manifest.states)) {
      states[name] = new NativeState<number | boolean>(entry.slot, entry.kind, this);
    }
    return Object.freeze(states);
  }

  #setInputRing(lane: DynamicInputLane): void {
    const words = new Int32Array(lane.ring.buffer);
    this.#checkStatus(
      this.#library.symbols.spellwire_host_set_input_ring(
        this.#requiredHost(),
        words,
        words.length,
        lane.ring.capacity,
      ),
      "attach input ring",
    );
    this.#inputWords = words;
  }

  #attachEventRing(): void {
    if (this.#eventAttached) return;
    this.events.ring.clear();
    const words = new Int32Array(this.events.ring.buffer);
    this.#checkStatus(
      this.#library.symbols.spellwire_host_set_event_ring(
        this.#requiredHost(),
        words,
        words.length,
        this.events.ring.capacity,
      ),
      "attach event ring",
    );
    this.#eventWords = words;
    this.#eventAttached = true;
    this.#eventDropped = this.events.ring.dropped;
  }

  #detachEventRing(): void {
    if (!this.#eventAttached) return;
    this.#checkStatus(
      this.#library.symbols.spellwire_host_set_event_ring(
        this.#requiredHost(),
        null,
        0,
        0,
      ),
      "detach event ring",
    );
    this.#eventAttached = false;
    this.#eventWords = null;
    this.events.ring.clear();
    this.#stateCacheValid = false;
  }

  #applyStateChange(slot: number, value: bigint): void {
    if (!this.#stateCacheValid || slot < 0 || slot >= this.#stateSnapshotCache.length) return;
    if (this.#stateSnapshotCache[slot] === value) return;
    this.#stateSnapshotCache[slot] = value;
    this.#stateValueDirty = true;
    this.#stateRevision += 1;
  }

  #snapshotFromCache(): NativeStateSnapshot {
    if (!this.#stateCacheValid) this.#refreshStateCache();
    if (!this.#stateValueDirty) return this.#stateSnapshotValue;
    const snapshot = Object.create(null) as Record<string, number | boolean>;
    for (const [name, entry] of Object.entries(this.#manifest.states)) {
      const value = this.#stateSnapshotCache[entry.slot] ?? 0n;
      snapshot[name] = entry.kind === "boolean" ? value !== 0n : Number(value);
    }
    this.#stateSnapshotValue = Object.freeze(snapshot);
    this.#stateValueDirty = false;
    return this.#stateSnapshotValue;
  }

  #refreshStateCache(): void {
    const required = Object.values(this.#manifest.states).reduce(
      (maximum, entry) => Math.max(maximum, entry.slot + 1),
      0,
    );
    if (required === 0) {
      this.#stateSnapshotCache = new BigInt64Array(0);
      this.#stateCacheValid = true;
      this.#stateValueDirty = false;
      this.#stateSnapshotValue = EMPTY_STATE_SNAPSHOT;
      return;
    }
    if (this.#stateSnapshot.length !== required) this.#stateSnapshot = new BigInt64Array(required);
    this.#checkStatus(
      this.#library.symbols.spellwire_host_state_snapshot(
        this.#requiredHost(),
        this.#stateSnapshot,
        required,
      ),
      "snapshot states",
    );
    let changed = !this.#stateCacheValid || this.#stateSnapshotCache.length !== required;
    if (!changed) {
      for (let slot = 0; slot < required; slot += 1) {
        if (this.#stateSnapshotCache[slot] !== this.#stateSnapshot[slot]) {
          changed = true;
          break;
        }
      }
    }
    if (!changed) return;
    if (this.#stateSnapshotCache.length !== required) {
      this.#stateSnapshotCache = new BigInt64Array(required);
    }
    this.#stateSnapshotCache.set(this.#stateSnapshot);
    this.#stateCacheValid = true;
    this.#stateValueDirty = true;
  }

  #retainEventConsumer(): () => void {
    this.#eventConsumers += 1;
    if (this.#running && !this.#eventAttached) {
      this.#attachEventRing();
      this.#refreshStateCache();
    }
    this.#startEventPump();
    let active = true;
    return () => {
      if (!active) return;
      active = false;
      this.#eventConsumers -= 1;
      if (this.#eventConsumers === 0) {
        this.#stopEventPump();
        if (this.#running && !this.#manualEvents) this.#detachEventRing();
      }
    };
  }

  #startEventPump(): void {
    if (!this.#running || this.#eventConsumers === 0 || this.#eventTimer !== undefined) return;
    this.#eventTimer = setInterval(() => {
      this.#drainEvents(4096);
    }, this.#eventPollIntervalMs);
    this.#eventTimer.unref?.();
  }

  #stopEventPump(): void {
    if (this.#eventTimer === undefined) return;
    clearInterval(this.#eventTimer);
    this.#eventTimer = undefined;
  }

  #checkStatus(status: number, operation: string): void {
    if (status === 0) return;
    const nativeMessage = [-5, -9, -10, -11].includes(status) ? this.#lastError() : "";
    throw new Error(
      `Spellwire native ${operation} failed with status ${status}` +
        (nativeMessage.length > 0 ? `: ${nativeMessage}` : ""),
    );
  }

  #lastError(): string {
    const host = this.#requiredHost();
    const required = Number(this.#library.symbols.spellwire_host_last_error(host, null, 0));
    if (required <= 1) return "";
    const buffer = new Uint8Array(required);
    this.#library.symbols.spellwire_host_last_error(host, buffer, buffer.byteLength);
    const terminator = buffer.indexOf(0);
    return new TextDecoder().decode(buffer.subarray(0, terminator < 0 ? buffer.length : terminator));
  }

  #requiredHost(): NativePointer {
    if (this.#host === null) throw new Error("Spellwire native host is closed");
    return this.#host;
  }

  #assertOpen(): void {
    if (this.#closed) throw new Error("Spellwire native host is closed");
  }
}

function validateManifest(value: unknown, path: string): NativeManifest {
  if (typeof value !== "object" || value === null) {
    throw new Error(`invalid Spellwire manifest object: ${path}`);
  }
  const record = value as Record<string, unknown>;
  if (record.version !== 1 || typeof record.states !== "object" || record.states === null) {
    throw new Error(`unsupported or malformed Spellwire manifest: ${path}`);
  }
  const states = Object.create(null) as Record<string, NativeStateManifestEntry>;
  const occupied = new Set<number>();
  for (const [name, rawEntry] of Object.entries(record.states)) {
    if (typeof rawEntry !== "object" || rawEntry === null) {
      throw new Error(`invalid state ${JSON.stringify(name)} in manifest: ${path}`);
    }
    const entry = rawEntry as Record<string, unknown>;
    if (
      !Number.isSafeInteger(entry.slot) ||
      (entry.slot as number) < 0 ||
      (entry.kind !== "number" && entry.kind !== "boolean") ||
      occupied.has(entry.slot as number)
    ) {
      throw new Error(`invalid state ${JSON.stringify(name)} in manifest: ${path}`);
    }
    occupied.add(entry.slot as number);
    states[name] = { slot: entry.slot as number, kind: entry.kind };
  }
  const effects = Object.create(null) as Record<string, NativeEffectManifestEntry>;
  const effectIds = new Set<number>();
  const rawEffects = record.effects ?? {};
  if (typeof rawEffects !== "object" || rawEffects === null) {
    throw new Error(`invalid effects in manifest: ${path}`);
  }
  for (const [name, rawEntry] of Object.entries(rawEffects)) {
    if (name.length === 0 || name.length > 128) {
      throw new Error(`invalid effect name ${JSON.stringify(name)} in manifest: ${path}`);
    }
    if (typeof rawEntry !== "object" || rawEntry === null) {
      throw new Error(`invalid effect ${JSON.stringify(name)} in manifest: ${path}`);
    }
    const entry = rawEntry as Record<string, unknown>;
    if (
      !Number.isSafeInteger(entry.id) ||
      (entry.id as number) < 0 ||
      (entry.id as number) > 0xffff ||
      effectIds.has(entry.id as number) ||
      !Array.isArray(entry.fields) ||
      entry.fields.length > 8
    ) {
      throw new Error(`invalid effect ${JSON.stringify(name)} in manifest: ${path}`);
    }
    const fieldNames = new Set<string>();
    const fields: NativeEffectField[] = entry.fields.map((rawField) => {
      if (typeof rawField !== "object" || rawField === null) {
        throw new Error(`invalid effect field in ${JSON.stringify(name)}: ${path}`);
      }
      const field = rawField as Record<string, unknown>;
      if (
        typeof field.name !== "string" ||
        field.name.length === 0 ||
        field.name.length > 128 ||
        fieldNames.has(field.name) ||
        (field.kind !== "number" && field.kind !== "boolean")
      ) {
        throw new Error(`invalid effect field in ${JSON.stringify(name)}: ${path}`);
      }
      fieldNames.add(field.name);
      return { name: field.name, kind: field.kind };
    });
    effectIds.add(entry.id as number);
    effects[name] = { id: entry.id as number, fields: Object.freeze(fields) };
  }
  return {
    version: 1,
    ...(typeof record.input === "string" ? { input: record.input } : {}),
    ...(typeof record.binary === "string" ? { binary: record.binary } : {}),
    ...(typeof record.handlers === "number" ? { handlers: record.handlers } : {}),
    ...(typeof record.instructions === "number" ? { instructions: record.instructions } : {}),
    states: Object.freeze(states),
    effects: Object.freeze(effects),
  };
}

function toError(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error));
}
