import { EventEmitter } from "node:events";
import type { Socket } from "node:net";

export const RPC_PROTOCOL_VERSION = 1;
const HEADER_BYTES = 4;
const MAX_FRAME_BYTES = 1024 * 1024;

export interface RpcRequest {
  readonly id: number;
  readonly method: string;
  readonly params?: unknown;
}

export interface RpcSuccess {
  readonly id: number;
  readonly result: unknown;
}

export interface RpcFailure {
  readonly id: number;
  readonly error: { readonly code: string; readonly message: string };
}

export interface RpcEvent {
  readonly event: string;
  readonly data: unknown;
}

export type RpcMessage = RpcRequest | RpcSuccess | RpcFailure | RpcEvent;

/** Length-prefixed UTF-8 JSON framing. Control-plane only; realtime events reach this after SPSC. */
export class RpcFramer extends EventEmitter {
  #buffer: Buffer<ArrayBufferLike> = Buffer.alloc(0);
  #expected = -1;

  push(chunk: Buffer): void {
    this.#buffer = this.#buffer.length === 0 ? chunk : Buffer.concat([this.#buffer, chunk]);
    while (true) {
      if (this.#expected < 0) {
        if (this.#buffer.length < HEADER_BYTES) return;
        this.#expected = this.#buffer.readUInt32LE(0);
        this.#buffer = this.#buffer.subarray(HEADER_BYTES);
        if (this.#expected > MAX_FRAME_BYTES) {
          this.emit("error", new RangeError("Spellwire RPC frame exceeds 1 MiB"));
          return;
        }
      }
      if (this.#buffer.length < this.#expected) return;
      const body = this.#buffer.subarray(0, this.#expected);
      this.#buffer = this.#buffer.subarray(this.#expected);
      this.#expected = -1;
      try {
        this.emit("message", JSON.parse(body.toString("utf8")) as RpcMessage);
      } catch (error) {
        this.emit("error", error);
        return;
      }
    }
  }
}

export function writeRpcMessage(socket: Socket, message: RpcMessage): boolean {
  const body = Buffer.from(JSON.stringify(message), "utf8");
  if (body.length > MAX_FRAME_BYTES) throw new RangeError("Spellwire RPC frame exceeds 1 MiB");
  const frame = Buffer.allocUnsafe(HEADER_BYTES + body.length);
  frame.writeUInt32LE(body.length, 0);
  body.copy(frame, HEADER_BYTES);
  return socket.write(frame);
}
