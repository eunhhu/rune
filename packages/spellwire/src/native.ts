import { existsSync, watch as watchDirectory, type FSWatcher } from "node:fs";
import { basename, dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { dlopen, FFIType, suffix, type Pointer } from "bun:ffi";

import { compileSource } from "./compiler/compiler";
import { encodeModule } from "./compiler/encode";
import { DynamicInputLane, NativeState, type NativeStateBridge } from "./runtime";

export const NATIVE_ABI_VERSION = 4;

export const NativeCapability = {
  HostCallbackInjection: 1 << 0,
  NativeObservation: 1 << 1,
  NativeInjection: 1 << 2,
  NativeOverlay: 1 << 3,
  HostLifecycle: 1 << 4,
  NonBlockingDelay: 1 << 5,
} as const;

export const NativePermission = {
  Observe: 1 << 0,
  Inject: 1 << 1,
} as const;

export interface NativeStateManifestEntry {
  readonly slot: number;
  readonly kind: "number" | "boolean";
}

export interface NativeManifest {
  readonly version: number;
  readonly input?: string;
  readonly binary?: string;
  readonly handlers?: number;
  readonly instructions?: number;
  readonly states: Readonly<Record<string, NativeStateManifestEntry>>;
}

export interface NativeHostOptions {
  readonly nativeLibraryPath?: string;
  readonly manifestPath?: string;
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

export class NativeHost implements NativeStateBridge {
  readonly inputPath: string;
  readonly nativeLibraryPath: string;
  readonly capabilities: number;
  states: Readonly<Record<string, NativeState<number | boolean>>>;

  readonly #library: LoadedNativeLibrary;
  readonly #manifestPath: string | undefined;
  #host: NativePointer | null;
  #manifest: NativeManifest;
  #running = false;
  #closed = false;
  #inputLane: DynamicInputLane | null = null;
  #inputWords: Int32Array | null = null;
  #stateSnapshot = new BigInt64Array(0);
  #stateSnapshotCache = new BigInt64Array(0);
  #stateSnapshotValue: NativeStateSnapshot = EMPTY_STATE_SNAPSHOT;
  #reloadTail: Promise<void> = Promise.resolve();

  private constructor(
    inputPath: string,
    manifestPath: string | undefined,
    nativeLibraryPath: string,
    descriptor: ProgramDescriptor,
  ) {
    this.inputPath = resolve(inputPath);
    this.#manifestPath = manifestPath;
    this.nativeLibraryPath = nativeLibraryPath;
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
  }

  static async load(inputPath: string, options: NativeHostOptions = {}): Promise<NativeHost> {
    const descriptor = await loadProgramDescriptor(inputPath, options.manifestPath);
    const libraryPath = resolveNativeLibrary(options.nativeLibraryPath);
    return new NativeHost(inputPath, options.manifestPath, libraryPath, descriptor);
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
    if (this.#inputLane !== null) {
      try {
        this.#setInputRing(this.#inputLane);
      } catch (error) {
        this.#library.symbols.spellwire_host_stop(this.#requiredHost());
        this.#running = false;
        throw error;
      }
    }
  }

  stop(): void {
    this.#assertOpen();
    if (!this.#running) return;
    this.#checkStatus(this.#library.symbols.spellwire_host_stop(this.#requiredHost()), "stop");
    this.#running = false;
    this.#inputWords = null;
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
    const state = this.states[name];
    if (!state) throw new RangeError(`native state ${JSON.stringify(name)} does not exist`);
    return state;
  }

  /** Reads every named state for one state-driven UI reconciliation pass. */
  snapshotStates(): NativeStateSnapshot {
    this.#assertOpen();
    const entries = Object.entries(this.#manifest.states);
    if (entries.length === 0) return EMPTY_STATE_SNAPSHOT;
    const required = entries.reduce(
      (maximum, [, entry]) => Math.max(maximum, entry.slot + 1),
      0,
    );
    if (this.#stateSnapshot.length !== required) {
      this.#stateSnapshot = new BigInt64Array(required);
    }
    this.#checkStatus(
      this.#library.symbols.spellwire_host_state_snapshot(
        this.#requiredHost(),
        this.#stateSnapshot,
        this.#stateSnapshot.length,
      ),
      "snapshot states",
    );
    let unchanged = this.#stateSnapshotCache.length === required;
    for (let index = 0; unchanged && index < required; index += 1) {
      unchanged = this.#stateSnapshotCache[index] === this.#stateSnapshot[index];
    }
    if (unchanged) return this.#stateSnapshotValue;

    this.#stateSnapshotCache = this.#stateSnapshot.slice();
    this.#stateSnapshotValue = Object.freeze(
      Object.fromEntries(
        entries.map(([name, entry]) => {
          const value = this.#stateSnapshot[entry.slot] ?? 0n;
          return [name, entry.kind === "boolean" ? value !== 0n : Number(value)];
        }),
      ),
    );
    return this.#stateSnapshotValue;
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
    this.states = this.#createStates(descriptor.manifest);
    this.#stateSnapshotCache = new BigInt64Array(0);
    this.#stateSnapshotValue = EMPTY_STATE_SNAPSHOT;
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
    return Object.freeze(
      Object.fromEntries(
        Object.entries(manifest.states).map(([name, entry]) => [
          name,
          new NativeState<number | boolean>(entry.slot, entry.kind, this),
        ]),
      ),
    );
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
  const states: Record<string, NativeStateManifestEntry> = {};
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
  return {
    version: 1,
    ...(typeof record.input === "string" ? { input: record.input } : {}),
    ...(typeof record.binary === "string" ? { binary: record.binary } : {}),
    ...(typeof record.handlers === "number" ? { handlers: record.handlers } : {}),
    ...(typeof record.instructions === "number" ? { instructions: record.instructions } : {}),
    states: Object.freeze(states),
  };
}

function toError(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error));
}
