import { chmod } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createServer, type Server, type Socket } from "node:net";
import { randomUUID, timingSafeEqual } from "node:crypto";

import type { NativeEffectPayload, NativeHost, NativeStateSnapshot } from "./native";
import {
  RPC_PROTOCOL_VERSION,
  RpcFramer,
  writeRpcMessage,
  type RpcMessage,
  type RpcRequest,
} from "./rpc-protocol";

export interface SpellwireRpcServerOptions {
  readonly endpoint?: string;
  readonly token?: string;
}

export type RpcMethod<Params = unknown, Result = unknown> =
  (params: Params) => Result | Promise<Result>;

interface ClientState {
  readonly socket: Socket;
  authenticated: boolean;
  stateSubscribed: boolean;
  readonly effects: Set<string>;
  inFlight: number;
}

export class SpellwireRpcServer {
  readonly endpoint: string;
  readonly token: string;
  readonly #host: NativeHost;
  readonly #server: Server;
  readonly #clients = new Set<ClientState>();
  readonly #methods = new Map<string, RpcMethod>();
  readonly #effectReleases = new Map<string, () => void>();
  #releaseState: (() => void) | undefined;
  #closed = false;

  private constructor(host: NativeHost, server: Server, endpoint: string, token: string) {
    this.#host = host;
    this.#server = server;
    this.endpoint = endpoint;
    this.token = token;
    this.#installBuiltins();
    server.on("connection", (socket) => this.#accept(socket));
  }

  static async start(
    host: NativeHost,
    options: SpellwireRpcServerOptions = {},
  ): Promise<SpellwireRpcServer> {
    const endpoint = options.endpoint ?? defaultRpcEndpoint();
    const token = options.token ?? randomUUID();
    if (endpoint.length === 0) throw new RangeError("RPC endpoint must not be empty");
    if (token.length < 16) throw new RangeError("RPC token must contain at least 16 characters");
    const server = createServer();
    const rpc = new SpellwireRpcServer(host, server, endpoint, token);
    await new Promise<void>((resolve, reject) => {
      server.once("error", reject);
      server.listen(endpoint, () => {
        server.off("error", reject);
        resolve();
      });
    });
    try {
      if (process.platform !== "win32") await chmod(endpoint, 0o600);
    } catch (error) {
      await new Promise<void>((resolve) => server.close(() => resolve()));
      throw error;
    }
    return rpc;
  }

  expose<Params = unknown, Result = unknown>(
    name: string,
    method: RpcMethod<Params, Result>,
  ): () => void {
    if (name.length === 0 || name.length > 128) {
      throw new RangeError("RPC method must contain between 1 and 128 characters");
    }
    if (name.startsWith("rpc.") || name.startsWith("state.") || name.startsWith("effect.")) {
      throw new RangeError("custom RPC methods cannot use reserved prefixes");
    }
    if (this.#methods.has(name)) throw new RangeError(`RPC method ${JSON.stringify(name)} exists`);
    this.#methods.set(name, method as RpcMethod);
    return () => this.#methods.delete(name);
  }

  async close(): Promise<void> {
    if (this.#closed) return;
    this.#closed = true;
    this.#releaseState?.();
    for (const release of this.#effectReleases.values()) release();
    this.#effectReleases.clear();
    for (const client of this.#clients) client.socket.destroy();
    await new Promise<void>((resolve, reject) => {
      this.#server.close((error) => error ? reject(error) : resolve());
    });
  }

  #accept(socket: Socket): void {
    const client: ClientState = {
      socket,
      authenticated: false,
      stateSubscribed: false,
      effects: new Set(),
      inFlight: 0,
    };
    this.#clients.add(client);
    const framer = new RpcFramer();
    socket.on("data", (chunk: Buffer) => framer.push(chunk));
    socket.on("close", () => {
      this.#clients.delete(client);
      this.#reconcileSubscriptions();
    });
    socket.on("error", () => socket.destroy());
    framer.on("message", (message: RpcMessage) => {
      if (!isRpcRequest(message)) {
        socket.destroy(new Error("Spellwire RPC received a malformed request"));
        return;
      }
      if (client.inFlight >= 64) {
        this.#send(client, {
          id: message.id,
          error: { code: "BUSY", message: "too many concurrent RPC requests" },
        });
        return;
      }
      client.inFlight += 1;
      void this.#request(client, message).finally(() => {
        client.inFlight -= 1;
      });
    });
    framer.on("error", () => socket.destroy());
  }

  async #request(client: ClientState, request: RpcRequest): Promise<void> {
    try {
      if (request.method === "rpc.authenticate") {
        const params = objectParams(request.params);
        if (!secureTokenEqual(params.token, this.token) || params.version !== RPC_PROTOCOL_VERSION) {
          throw rpcError("AUTH_FAILED", "invalid token or protocol version");
        }
        client.authenticated = true;
        this.#send(client, { id: request.id, result: { version: RPC_PROTOCOL_VERSION } });
        return;
      }
      if (!client.authenticated) throw rpcError("AUTH_REQUIRED", "authenticate first");
      if (request.method === "state.subscribe") {
        client.stateSubscribed = true;
        this.#reconcileSubscriptions();
        this.#send(client, { id: request.id, result: null });
        return;
      }
      if (request.method === "state.unsubscribe") {
        client.stateSubscribed = false;
        this.#reconcileSubscriptions();
        this.#send(client, { id: request.id, result: null });
        return;
      }
      if (request.method === "effect.subscribe" || request.method === "effect.unsubscribe") {
        const name = stringParam(request.params, "name");
        if (!Object.hasOwn(this.#host.manifest.effects, name)) {
          throw rpcError("NOT_FOUND", `unknown effect ${name}`);
        }
        if (request.method === "effect.subscribe") client.effects.add(name);
        else client.effects.delete(name);
        this.#reconcileSubscriptions();
        this.#send(client, { id: request.id, result: null });
        return;
      }
      const method = this.#methods.get(request.method);
      if (!method) throw rpcError("METHOD_NOT_FOUND", `unknown method ${request.method}`);
      const result = await method(request.params);
      this.#send(client, { id: request.id, result: result ?? null });
    } catch (error) {
      const failure = error instanceof RpcServerError
        ? error
        : rpcError("INTERNAL", error instanceof Error ? error.message : String(error));
      this.#send(client, {
        id: request.id,
        error: { code: failure.code, message: failure.message },
      });
      if (failure.code === "AUTH_FAILED") client.socket.end();
    }
  }

  #installBuiltins(): void {
    this.#methods.set("state.snapshot", () => this.#host.snapshotStates());
    this.#methods.set("state.get", (params) => {
      const name = stringParam(params, "name");
      return this.#host.state(name).get();
    });
    this.#methods.set("state.set", (params) => {
      const record = objectParams(params);
      const name = stringParam(params, "name");
      if (typeof record.value !== "number" && typeof record.value !== "boolean") {
        throw rpcError("INVALID_PARAMS", "state value must be a number or boolean");
      }
      if (typeof record.value === "number" && !Number.isSafeInteger(record.value)) {
        throw rpcError("INVALID_PARAMS", "numeric state must be a safe integer");
      }
      this.#host.state(name).set(record.value);
      return null;
    });
  }

  #reconcileSubscriptions(): void {
    const needsState = [...this.#clients].some((client) => client.stateSubscribed);
    if (needsState && !this.#releaseState) {
      this.#releaseState = this.#host.onStateChange((snapshot) => this.#publishState(snapshot));
    } else if (!needsState && this.#releaseState) {
      this.#releaseState();
      this.#releaseState = undefined;
    }
    const names = new Set([...this.#clients].flatMap((client) => [...client.effects]));
    for (const name of names) {
      if (this.#effectReleases.has(name)) continue;
      this.#effectReleases.set(
        name,
        this.#host.effects.on(name, (payload) => this.#publishEffect(name, payload)),
      );
    }
    for (const [name, release] of this.#effectReleases) {
      if (names.has(name)) continue;
      release();
      this.#effectReleases.delete(name);
    }
  }

  #publishState(snapshot: NativeStateSnapshot): void {
    for (const client of this.#clients) {
      if (client.authenticated && client.stateSubscribed) {
        this.#send(client, { event: "state", data: snapshot });
      }
    }
  }

  #publishEffect(name: string, payload: NativeEffectPayload): void {
    for (const client of this.#clients) {
      if (client.authenticated && client.effects.has(name)) {
        this.#send(client, { event: `effect:${name}`, data: payload });
      }
    }
  }

  #send(client: ClientState, message: RpcMessage): void {
    if (!writeRpcMessage(client.socket, message)) {
      client.socket.destroy(new Error("Spellwire RPC client exceeded socket backpressure"));
    }
  }
}

class RpcServerError extends Error {
  constructor(readonly code: string, message: string) {
    super(message);
  }
}

function rpcError(code: string, message: string): RpcServerError {
  return new RpcServerError(code, message);
}

function objectParams(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw rpcError("INVALID_PARAMS", "params must be an object");
  }
  return value as Record<string, unknown>;
}

function stringParam(value: unknown, name: string): string {
  const field = objectParams(value)[name];
  if (typeof field !== "string" || field.length === 0) {
    throw rpcError("INVALID_PARAMS", `${name} must be a non-empty string`);
  }
  return field;
}

function defaultRpcEndpoint(): string {
  if (process.platform === "win32") return `\\\\.\\pipe\\spellwire-${process.pid}`;
  return join(tmpdir(), `spellwire-${process.getuid?.() ?? "user"}-${process.pid}.sock`);
}

function secureTokenEqual(value: unknown, expected: string): boolean {
  if (typeof value !== "string") return false;
  const left = Buffer.from(value);
  const right = Buffer.from(expected);
  return left.length === right.length && timingSafeEqual(left, right);
}

function isRpcRequest(message: RpcMessage): message is RpcRequest {
  return typeof message === "object" &&
    message !== null &&
    "id" in message &&
    Number.isSafeInteger(message.id) &&
    message.id >= 0 &&
    "method" in message &&
    typeof message.method === "string" &&
    message.method.length > 0 &&
    message.method.length <= 128;
}
