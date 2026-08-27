import { Socket } from "node:net";

import {
  RPC_PROTOCOL_VERSION,
  RpcFramer,
  writeRpcMessage,
  type RpcEvent,
  type RpcFailure,
  type RpcMessage,
  type RpcSuccess,
} from "./rpc-protocol";

export interface SpellwireRpcClientOptions {
  readonly endpoint: string;
  readonly token: string;
}

export type RpcEventHandler<T = unknown> = (data: T) => void;

interface PendingCall {
  resolve(value: unknown): void;
  reject(error: Error): void;
}

/** Node/Electron-compatible local IPC client. This module has no Bun or native-FFI dependency. */
export class SpellwireRpcClient {
  readonly endpoint: string;
  readonly #socket: Socket;
  readonly #framer = new RpcFramer();
  readonly #pending = new Map<number, PendingCall>();
  readonly #handlers = new Map<string, Set<RpcEventHandler>>();
  #nextId = 1;
  #closed = false;

  private constructor(socket: Socket, endpoint: string) {
    this.#socket = socket;
    this.endpoint = endpoint;
    socket.on("data", (chunk: Buffer) => this.#framer.push(chunk));
    socket.on("error", (error) => this.#failAll(error));
    socket.on("close", () => this.#failAll(new Error("Spellwire RPC connection closed")));
    this.#framer.on("message", (message: RpcMessage) => this.#receive(message));
    this.#framer.on("error", (error: Error) => {
      this.#failAll(error);
      socket.destroy(error);
    });
  }

  static async connect(options: SpellwireRpcClientOptions): Promise<SpellwireRpcClient> {
    if (options.endpoint.length === 0) throw new RangeError("RPC endpoint must not be empty");
    if (options.token.length < 16) throw new RangeError("RPC token must contain at least 16 characters");
    const socket = new Socket();
    await new Promise<void>((resolve, reject) => {
      socket.once("error", reject);
      socket.connect(options.endpoint, () => {
        socket.off("error", reject);
        resolve();
      });
    });
    const client = new SpellwireRpcClient(socket, options.endpoint);
    await client.call("rpc.authenticate", {
      token: options.token,
      version: RPC_PROTOCOL_VERSION,
    });
    return client;
  }

  call<T = unknown>(method: string, params?: unknown): Promise<T> {
    if (this.#closed) return Promise.reject(new Error("Spellwire RPC client is closed"));
    if (method.length === 0 || method.length > 128) {
      return Promise.reject(new RangeError("RPC method must contain between 1 and 128 characters"));
    }
    if (this.#pending.size >= 1024) {
      return Promise.reject(new Error("Spellwire RPC pending-call limit reached"));
    }
    const id = this.#nextId++;
    return new Promise<T>((resolve, reject) => {
      this.#pending.set(id, {
        resolve: (value) => resolve(value as T),
        reject,
      });
      try {
        writeRpcMessage(this.#socket, { id, method, ...(params === undefined ? {} : { params }) });
      } catch (error) {
        this.#pending.delete(id);
        reject(error instanceof Error ? error : new Error(String(error)));
      }
    });
  }

  getState(name: string): Promise<number | boolean> {
    return this.call("state.get", { name });
  }

  setState(name: string, value: number | boolean): Promise<void> {
    return this.call("state.set", { name, value });
  }

  snapshotStates(): Promise<Readonly<Record<string, number | boolean>>> {
    return this.call("state.snapshot");
  }

  async onState(handler: RpcEventHandler<Readonly<Record<string, number | boolean>>>): Promise<() => void> {
    const release = this.#on("state", handler as RpcEventHandler);
    try {
      await this.call("state.subscribe");
    } catch (error) {
      release();
      throw error;
    }
    return () => {
      release();
      if (!this.#handlers.has("state")) void this.call("state.unsubscribe").catch(() => undefined);
    };
  }

  async onEffect<T = Readonly<Record<string, number | boolean>>>(
    name: string,
    handler: RpcEventHandler<T>,
  ): Promise<() => void> {
    const key = `effect:${name}`;
    const release = this.#on(key, handler as RpcEventHandler);
    try {
      await this.call("effect.subscribe", { name });
    } catch (error) {
      release();
      throw error;
    }
    return () => {
      release();
      if (!this.#handlers.has(key)) {
        void this.call("effect.unsubscribe", { name }).catch(() => undefined);
      }
    };
  }

  close(): void {
    if (this.#closed) return;
    this.#closed = true;
    this.#socket.end();
    this.#failAll(new Error("Spellwire RPC client is closed"));
  }

  #on(event: string, handler: RpcEventHandler): () => void {
    const handlers = this.#handlers.get(event) ?? new Set<RpcEventHandler>();
    this.#handlers.set(event, handlers);
    const registration: RpcEventHandler = (data) => handler(data);
    handlers.add(registration);
    let active = true;
    return () => {
      if (!active) return;
      active = false;
      handlers.delete(registration);
      if (handlers.size === 0) this.#handlers.delete(event);
    };
  }

  #receive(message: RpcMessage): void {
    if (typeof message !== "object" || message === null) {
      this.#socket.destroy(new Error("Spellwire RPC received a malformed message"));
      return;
    }
    if ("event" in message) {
      if (typeof message.event !== "string" || message.event.length > 256) {
        this.#socket.destroy(new Error("Spellwire RPC received a malformed event"));
        return;
      }
      const event = message as RpcEvent;
      for (const handler of this.#handlers.get(event.event) ?? []) handler(event.data);
      return;
    }
    if (!("id" in message) || !Number.isSafeInteger(message.id) || message.id < 0) {
      this.#socket.destroy(new Error("Spellwire RPC received a malformed response"));
      return;
    }
    const pending = this.#pending.get(message.id);
    if (!pending) return;
    this.#pending.delete(message.id);
    if ("error" in message) {
      const failure = message as RpcFailure;
      pending.reject(new Error(`${failure.error.code}: ${failure.error.message}`));
    } else {
      pending.resolve((message as RpcSuccess).result);
    }
  }

  #failAll(error: Error): void {
    this.#closed = true;
    for (const pending of this.#pending.values()) pending.reject(error);
    this.#pending.clear();
  }
}
