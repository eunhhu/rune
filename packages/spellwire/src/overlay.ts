import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export type OverlayNodeId = number;

export interface OverlayStroke {
  readonly fill: string;
  readonly width: number;
}

export interface OverlayShadow {
  readonly fill: string;
  readonly x?: number;
  readonly y?: number;
  readonly blur?: number;
  readonly spread?: number;
}

export interface OverlayFont {
  readonly family?: "system" | "monospace";
  readonly weight?: number;
  readonly lineHeight?: number;
  readonly letterSpacing?: number;
  readonly align?: "left" | "center" | "right";
}

export interface OverlayText {
  readonly kind: "text";
  readonly x: number;
  readonly y: number;
  readonly width?: number;
  readonly height?: number;
  readonly text: string;
  readonly size: number;
  readonly color?: string;
  readonly fill?: string;
  readonly opacity?: number;
  readonly font?: OverlayFont;
  readonly z?: number;
}

export interface OverlayRect {
  readonly kind: "rect";
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly radius: number;
  readonly color?: string;
  readonly fill?: string;
  readonly stroke?: OverlayStroke;
  readonly shadow?: OverlayShadow;
  readonly opacity?: number;
  readonly z?: number;
}

export interface OverlayEllipse {
  readonly kind: "ellipse";
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly color?: string;
  readonly fill?: string;
  readonly stroke?: OverlayStroke;
  readonly shadow?: OverlayShadow;
  readonly opacity?: number;
  readonly z?: number;
}

export interface OverlayLine {
  readonly kind: "line";
  readonly x1: number;
  readonly y1: number;
  readonly x2: number;
  readonly y2: number;
  readonly width: number;
  readonly color?: string;
  readonly fill?: string;
  readonly opacity?: number;
  readonly z?: number;
}

export type OverlayNode = OverlayText | OverlayRect | OverlayEllipse | OverlayLine;

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
  readonly #pending = new Map<OverlayNodeId, OverlayMutation>();
  #nextId = 1;
  #revision = 0;

  create(node: OverlayNode): OverlayNodeId {
    const id = this.#nextId++;
    const retained = cloneNode(node);
    this.#nodes.set(id, retained);
    this.#pending.set(id, { revision: ++this.#revision, id, node: retained });
    return id;
  }

  update(id: OverlayNodeId, node: OverlayNode): boolean {
    const previous = this.#nodes.get(id);
    if (!previous) throw new RangeError(`overlay node ${id} does not exist`);
    const retained = cloneNode(node);
    if (nodesEqual(previous, retained)) return false;
    this.#nodes.set(id, retained);
    this.#pending.set(id, { revision: ++this.#revision, id, node: retained });
    return true;
  }

  remove(id: OverlayNodeId): boolean {
    if (!this.#nodes.delete(id)) return false;
    this.#pending.set(id, { revision: ++this.#revision, id, node: null });
    return true;
  }

  drainMutations(): OverlayMutation[] {
    const mutations = [...this.#pending.values()].sort(
      (left, right) => left.revision - right.revision,
    );
    this.#pending.clear();
    return mutations;
  }

  snapshot(): ReadonlyMap<OverlayNodeId, OverlayNode> {
    return new Map([...this.#nodes].map(([id, node]) => [id, cloneNode(node)]));
  }
}

export interface NativeOverlayOptions {
  readonly executablePath?: string;
  readonly readyTimeoutMs?: number;
  readonly window?: OverlayWindowOptions;
}

export interface OverlayWindowOptions {
  readonly title?: string;
  readonly transparent?: boolean;
  readonly alwaysOnTop?: boolean;
  readonly focusable?: boolean;
  readonly clickThrough?: boolean;
  readonly decorations?: boolean;
  readonly resizable?: boolean;
  readonly visible?: boolean;
}

export interface ResolvedOverlayWindowOptions {
  readonly title: string;
  readonly transparent: boolean;
  readonly alwaysOnTop: boolean;
  readonly focusable: boolean;
  readonly clickThrough: boolean;
  readonly decorations: boolean;
  readonly resizable: boolean;
  readonly visible: boolean;
}

export interface NativeOverlayReady {
  readonly event: "ready";
  readonly width: number;
  readonly height: number;
  readonly scaleFactor: number;
  readonly alphaMode: string;
  readonly window: ResolvedOverlayWindowOptions;
}

type OverlayCommand =
  | { readonly op: "batch"; readonly mutations: readonly OverlayWireMutation[] }
  | { readonly op: "clear" | "show" | "hide" | "exit" };

type OverlayWireMutation =
  | { readonly id: number; readonly node: OverlayNode }
  | { readonly id: number; readonly remove: true };

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

function spawnOverlay(executablePath: string, window: ResolvedOverlayWindowOptions) {
  return Bun.spawn([executablePath, "--window-config", JSON.stringify(window)], {
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
    const window = resolveOverlayWindowOptions(options.window);
    const child = spawnOverlay(executablePath, window);
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
      await this.#write({
        op: "batch",
        mutations: mutations.map((mutation) =>
          mutation.node === null
            ? { id: mutation.id, remove: true }
            : {
                id: mutation.id,
                node: scaleNode(mutation.node, this.ready.scaleFactor),
              },
        ),
      });
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

export function resolveOverlayWindowOptions(
  options: OverlayWindowOptions = {},
): ResolvedOverlayWindowOptions {
  const title = options.title ?? "Spellwire Overlay";
  if (
    typeof title !== "string" ||
    title.trim().length === 0 ||
    [...title].length > 256
  ) {
    throw new RangeError("overlay window title must contain 1 to 256 characters");
  }
  const resolved = {
    title,
    transparent: options.transparent ?? true,
    alwaysOnTop: options.alwaysOnTop ?? true,
    focusable: options.focusable ?? false,
    clickThrough: options.clickThrough ?? true,
    decorations: options.decorations ?? false,
    resizable: options.resizable ?? false,
    visible: options.visible ?? true,
  } satisfies ResolvedOverlayWindowOptions;
  for (const [name, value] of Object.entries(resolved)) {
    if (name !== "title" && typeof value !== "boolean") {
      throw new TypeError(`overlay window ${name} must be boolean`);
    }
  }
  return Object.freeze(resolved);
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
    typeof record.scaleFactor === "number" &&
    record.scaleFactor > 0 &&
    typeof record.alphaMode === "string" &&
    isResolvedOverlayWindowOptions(record.window)
  );
}

function isResolvedOverlayWindowOptions(value: unknown): value is ResolvedOverlayWindowOptions {
  if (typeof value !== "object" || value === null) return false;
  const record = value as Record<string, unknown>;
  return typeof record.title === "string" &&
    typeof record.transparent === "boolean" &&
    typeof record.alwaysOnTop === "boolean" &&
    typeof record.focusable === "boolean" &&
    typeof record.clickThrough === "boolean" &&
    typeof record.decorations === "boolean" &&
    typeof record.resizable === "boolean" &&
    typeof record.visible === "boolean";
}

function scaleNode(node: OverlayNode, scale: number): OverlayNode {
  if (scale === 1) return node;
  if (node.kind === "text") {
    return {
      ...node,
      x: node.x * scale,
      y: node.y * scale,
      ...(node.width === undefined ? {} : { width: node.width * scale }),
      ...(node.height === undefined ? {} : { height: node.height * scale }),
      size: node.size * scale,
      ...(node.font === undefined
        ? {}
        : {
            font: {
              ...node.font,
              ...(node.font.lineHeight === undefined
                ? {}
                : { lineHeight: node.font.lineHeight * scale }),
              ...(node.font.letterSpacing === undefined
                ? {}
                : { letterSpacing: node.font.letterSpacing * scale }),
            },
          }),
    };
  }
  if (node.kind === "line") {
    return {
      ...node,
      x1: node.x1 * scale,
      y1: node.y1 * scale,
      x2: node.x2 * scale,
      y2: node.y2 * scale,
      width: node.width * scale,
    };
  }
  const effects = {
    ...(node.stroke === undefined
      ? {}
      : { stroke: { ...node.stroke, width: node.stroke.width * scale } }),
    ...(node.shadow === undefined
      ? {}
      : {
          shadow: {
            ...node.shadow,
            ...(node.shadow.x === undefined ? {} : { x: node.shadow.x * scale }),
            ...(node.shadow.y === undefined ? {} : { y: node.shadow.y * scale }),
            ...(node.shadow.blur === undefined ? {} : { blur: node.shadow.blur * scale }),
            ...(node.shadow.spread === undefined
              ? {}
              : { spread: node.shadow.spread * scale }),
          },
        }),
  };
  if (node.kind === "ellipse") {
    return {
      ...node,
      x: node.x * scale,
      y: node.y * scale,
      width: node.width * scale,
      height: node.height * scale,
      ...effects,
    };
  }
  return {
    ...node,
    x: node.x * scale,
    y: node.y * scale,
    width: node.width * scale,
    height: node.height * scale,
    radius: node.radius * scale,
    ...effects,
  };
}

function cloneNode(node: OverlayNode): OverlayNode {
  return structuredClone(node);
}

function nodesEqual(left: OverlayNode, right: OverlayNode): boolean {
  if (left.kind !== right.kind) return false;
  if (left.kind === "text" && right.kind === "text") {
    return (
      left.x === right.x && left.y === right.y && left.width === right.width &&
      left.height === right.height && left.text === right.text && left.size === right.size &&
      left.color === right.color && left.fill === right.fill && left.opacity === right.opacity &&
      left.z === right.z && objectFieldsEqual(left.font, right.font)
    );
  }
  if (left.kind === "line" && right.kind === "line") {
    return (
      left.x1 === right.x1 && left.y1 === right.y1 && left.x2 === right.x2 &&
      left.y2 === right.y2 && left.width === right.width && left.color === right.color &&
      left.fill === right.fill && left.opacity === right.opacity && left.z === right.z
    );
  }
  if (left.kind === "rect" && right.kind === "rect") {
    return (
      left.x === right.x && left.y === right.y && left.width === right.width &&
      left.height === right.height && left.radius === right.radius && left.color === right.color &&
      left.fill === right.fill && left.opacity === right.opacity && left.z === right.z &&
      objectFieldsEqual(left.stroke, right.stroke) && objectFieldsEqual(left.shadow, right.shadow)
    );
  }
  if (left.kind === "ellipse" && right.kind === "ellipse") {
    return (
      left.x === right.x && left.y === right.y && left.width === right.width &&
      left.height === right.height && left.color === right.color && left.fill === right.fill &&
      left.opacity === right.opacity && left.z === right.z &&
      objectFieldsEqual(left.stroke, right.stroke) && objectFieldsEqual(left.shadow, right.shadow)
    );
  }
  return false;
}

function objectFieldsEqual(
  left: object | undefined,
  right: object | undefined,
): boolean {
  if (left === right) return true;
  if (!left || !right) return false;
  const leftRecord = left as Readonly<Record<string, unknown>>;
  const rightRecord = right as Readonly<Record<string, unknown>>;
  const keys = Object.keys(leftRecord);
  return keys.length === Object.keys(right).length && keys.every(
    (key) => Object.hasOwn(rightRecord, key) && leftRecord[key] === rightRecord[key],
  );
}
