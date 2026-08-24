export type OverlayNodeId = number;

export interface OverlayText {
  kind: "text";
  x: number;
  y: number;
  text: string;
  size: number;
}

export interface OverlayRect {
  kind: "rect";
  x: number;
  y: number;
  width: number;
  height: number;
  radius: number;
}

export interface OverlayLine {
  kind: "line";
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  width: number;
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
