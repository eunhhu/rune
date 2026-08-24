import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export type OverlayNodeId = number;

export interface OverlayText {
  kind: "text";
  x: number;
  y: number;
  text: string;
  size: number;
  color?: string;
}

export interface OverlayRect {
  kind: "rect";
  x: number;
  y: number;
  width: number;
  height: number;
  radius: number;
  color?: string;
}

export interface OverlayLine {
  kind: "line";
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  width: number;
  color?: string;
}

export type OverlayNode = OverlayText | OverlayRect | OverlayLine;

export interface OverlayMutation {
  revision: number;
  id: OverlayNodeId;
  node: OverlayNode | null;
}

/**
 * Retained overlay scene. A renderer consumes mutations/snapshots on its own thread;
 * no per-frame JavaScript callback is part of the contract.
 */
export class OverlayScene {
  readonly #nodes = new Map<OverlayNodeId, OverlayNode>();
  readonly #pending: OverlayMutation[] = [];
  #nextId = 1;
  #revision = 0;

  create(node: OverlayNode): OverlayNodeId {
    const id = this.#nextId++;
    this.#nodes.set(id, structuredClone(node));
    this.#pending.push({ revision: ++this.#revision, id, node: structuredClone(node) });
    return id;
  }

  update(id: OverlayNodeId, node: OverlayNode): void {
    if (!this.#nodes.has(id)) throw new RangeError(`overlay node ${id} does not exist`);
    this.#nodes.set(id, structuredClone(node));
    this.#pending.push({ revision: ++this.#revision, id, node: structuredClone(node) });
  }

  remove(id: OverlayNodeId): boolean {
    if (!this.#nodes.delete(id)) return false;
    this.#pending.push({ revision: ++this.#revision, id, node: null });
    return true;
  }

  drainMutations(): OverlayMutation[] {
    return this.#pending.splice(0, this.#pending.length);
  }

  snapshot(): ReadonlyMap<OverlayNodeId, OverlayNode> {
    return new Map(this.#nodes);
  }
}

export interface NativeOverlayOptions {
  readonly executablePath?: string;
  readonly readyTimeoutMs?: number;
}

export interface NativeOverlayReady {
  readonly event: "ready";
  readonly width: number;
  readonly height: number;
  readonly alphaMode: string;
}

type OverlayCommand =
  | { readonly op: "upsert"; readonly id: number; readonly node: OverlayNode }
  | { readonly op: "remove"; readonly id: number }
  | { readonly op: "clear" | "show" | "hide" | "exit" };

const moduleDirectory = dirname(fileURLToPath(import.meta.url));

export function overlayExecutableFileName(): string {
  return process.platform === "win32" ? "spellwire-overlay.exe" : "spellwire-overlay";
}

export function resolveOverlayExecutable(explicitPath?: string): string {
  const fileName = overlayExecutableFileName();
  const platformDirectory = `${process.platform}-${process.arch}`;
  const candidates = [
    explicitPath,
    process.env.SPELLWIRE_OVERLAY_EXECUTABLE,
    join(moduleDirectory, "..", "native", platformDirectory, fileName),
    join(moduleDirectory, "..", "..", "..", "target", "release", fileName),
    join(moduleDirectory, "..", "..", "..", "target", "debug", fileName),
  ].filter((candidate): candidate is string => typeof candidate === "string" && candidate.length > 0);

  for (const candidate of candidates) {
    const absolute = resolve(candidate);
    if (existsSync(absolute)) return absolute;
  }
  throw new Error(
    `Spellwire overlay executable not found (${fileName}). Build it with ` +
      "`bun run build:native` or set SPELLWIRE_OVERLAY_EXECUTABLE.",
  );
}

function spawnOverlay(executablePath: string) {
  return Bun.spawn([executablePath], {
    stdin: "pipe",
    stdout: "pipe",
    stderr: "inherit",
  });
}

/** Dedicated native retained-mode overlay process. JavaScript writes only scene mutations. */
export class NativeOverlayRenderer {
  readonly executablePath: string;
  readonly ready: NativeOverlayReady;

  readonly #child: ReturnType<typeof spawnOverlay>;
  #closed = false;

  private constructor(
    executablePath: string,
    child: ReturnType<typeof spawnOverlay>,
    ready: NativeOverlayReady,
  ) {
    this.executablePath = executablePath;
    this.#child = child;
    this.ready = ready;
  }

  static async start(options: NativeOverlayOptions = {}): Promise<NativeOverlayRenderer> {
    const readyTimeoutMs = options.readyTimeoutMs ?? 5_000;
    if (!Number.isSafeInteger(readyTimeoutMs) || readyTimeoutMs <= 0) {
      throw new RangeError("readyTimeoutMs must be a positive safe integer");
    }
    const executablePath = resolveOverlayExecutable(options.executablePath);
    const child = spawnOverlay(executablePath);
    try {
      const ready = await readReady(child.stdout, child.exited, readyTimeoutMs);
      return new NativeOverlayRenderer(executablePath, child, ready);
    } catch (error) {
      child.kill();
      throw error;
    }
  }

  async apply(scene: OverlayScene): Promise<number> {
    this.#assertOpen();
    const mutations = scene.drainMutations();
    if (mutations.length > 0) {
      const commands = mutations
        .map((mutation) =>
          JSON.stringify(
            mutation.node === null
          ? { op: "remove", id: mutation.id }
              : { op: "upsert", id: mutation.id, node: mutation.node },
          ),
        )
        .join("\n");
      this.#child.stdin.write(`${commands}\n`);
      await this.#child.stdin.flush();
    }
    return mutations.length;
  }

  async clear(): Promise<void> {
    await this.#write({ op: "clear" });
  }

  async show(): Promise<void> {
    await this.#write({ op: "show" });
  }

  async hide(): Promise<void> {
    await this.#write({ op: "hide" });
  }

  async close(): Promise<void> {
    if (this.#closed) return;
    this.#closed = true;
    try {
      await this.#write({ op: "exit" }, true);
      this.#child.stdin.end();
      await this.#child.exited;
    } catch (error) {
      this.#child.kill();
      await this.#child.exited;
      throw error;
    }
  }

  async #write(command: OverlayCommand, allowClosed = false): Promise<void> {
    if (!allowClosed) this.#assertOpen();
    this.#child.stdin.write(`${JSON.stringify(command)}\n`);
    await this.#child.stdin.flush();
  }

  #assertOpen(): void {
    if (this.#closed) throw new Error("Spellwire native overlay is closed");
  }
}

async function readReady(
  stream: ReadableStream<Uint8Array>,
  exited: Promise<number>,
  timeoutMs: number,
): Promise<NativeOverlayReady> {
  const reader = stream.getReader();
  const decoder = new TextDecoder();
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    const timeout = new Promise<{ readonly kind: "timeout" }>((resolveTimeout) => {
      timer = setTimeout(() => resolveTimeout({ kind: "timeout" }), timeoutMs);
    });
    const processExit = exited.then((code) => ({ kind: "exit" as const, code }));
    let text = "";
    while (!text.includes("\n")) {
      const outcome = await Promise.race([
        reader.read().then((result) => ({ kind: "read" as const, result })),
        processExit,
        timeout,
      ]);
      if (outcome.kind === "timeout") throw new Error("Spellwire overlay ready timeout");
      if (outcome.kind === "exit") {
        throw new Error(`Spellwire overlay exited before ready (status ${outcome.code})`);
      }
      const result = outcome.result;
      if (result.done) throw new Error("Spellwire overlay closed stdout before ready");
      text += decoder.decode(result.value, { stream: true });
    }
    const value: unknown = JSON.parse(text.slice(0, text.indexOf("\n")));
    if (!isReady(value)) throw new Error("Spellwire overlay returned an invalid ready message");
    return value;
  } finally {
    if (timer !== undefined) clearTimeout(timer);
    await reader.cancel().catch(() => undefined);
    reader.releaseLock();
  }
}

function isReady(value: unknown): value is NativeOverlayReady {
  if (typeof value !== "object" || value === null) return false;
  const record = value as Record<string, unknown>;
  return (
    record.event === "ready" &&
    typeof record.width === "number" &&
    typeof record.height === "number" &&
    typeof record.alphaMode === "string"
  );
}
