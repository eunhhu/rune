import { afterEach, describe, expect, test } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { randomUUID } from "node:crypto";

import type { NativeEffectHandler, NativeHost, NativeStateSnapshot } from "../src/native";
import { SpellwireRpcClient } from "../src/rpc";
import { SpellwireRpcServer } from "../src/rpc-server";

const temporaryDirectories: string[] = [];

afterEach(async () => {
  for (const directory of temporaryDirectories.splice(0)) {
    await rm(directory, { recursive: true, force: true });
  }
});

class FakeHost {
  readonly manifest = {
    version: 1,
    states: { count: { slot: 0, kind: "number" as const } },
    effects: {
      changed: {
        id: 0,
        fields: [{ name: "count", kind: "number" as const }],
      },
    },
  };
  readonly #stateHandlers = new Set<(snapshot: NativeStateSnapshot) => void>();
  readonly #effectHandlers = new Set<NativeEffectHandler>();
  #count = 0;

  readonly effects = {
    on: (_name: string, handler: NativeEffectHandler): (() => void) => {
      this.#effectHandlers.add(handler);
      return () => this.#effectHandlers.delete(handler);
    },
  };

  snapshotStates(): NativeStateSnapshot {
    return Object.freeze({ count: this.#count });
  }

  state(_name: string) {
    return {
      get: (): number => this.#count,
      set: (value: number | boolean): void => {
        this.#count = Number(value);
        const snapshot = this.snapshotStates();
        for (const handler of this.#stateHandlers) handler(snapshot);
      },
    };
  }

  onStateChange(handler: (snapshot: NativeStateSnapshot) => void): () => void {
    this.#stateHandlers.add(handler);
    return () => this.#stateHandlers.delete(handler);
  }

  emitEffect(): void {
    for (const handler of this.#effectHandlers) handler({ count: this.#count });
  }
}

describe("Spellwire local RPC", () => {
  test("authenticates and transports state, effects, and custom methods", async () => {
    const directory = await mkdtemp(join(tmpdir(), "spellwire-rpc-"));
    temporaryDirectories.push(directory);
    const endpoint = process.platform === "win32"
      ? `\\\\.\\pipe\\spellwire-test-${randomUUID()}`
      : join(directory, "spellwire.sock");
    const host = new FakeHost();
    const server = await SpellwireRpcServer.start(host as unknown as NativeHost, {
      endpoint,
      token: "test-token-123456",
    });
    server.expose("math.double", (params) => Number(params) * 2);
    const client = await SpellwireRpcClient.connect({ endpoint, token: "test-token-123456" });

    expect(await client.snapshotStates()).toEqual({ count: 0 });
    await client.setState("count", 7);
    expect(await client.getState("count")).toBe(7);
    expect(await client.call<number>("math.double", 9)).toBe(18);

    const statePromise = nextValue<NativeStateSnapshot>();
    const releaseState = await client.onState(statePromise.resolve);
    await client.setState("count", 8);
    expect(await statePromise.promise).toEqual({ count: 8 });

    const effectPromise = nextValue<Readonly<Record<string, number | boolean>>>();
    const releaseEffect = await client.onEffect("changed", effectPromise.resolve);
    host.emitEffect();
    expect(await effectPromise.promise).toEqual({ count: 8 });

    let duplicateCalls = 0;
    const duplicateHandler = (): void => {
      duplicateCalls += 1;
    };
    const releaseFirst = await client.onState(duplicateHandler);
    const releaseSecond = await client.onState(duplicateHandler);
    await client.setState("count", 9);
    expect(duplicateCalls).toBe(2);
    releaseFirst();
    await client.setState("count", 10);
    expect(duplicateCalls).toBe(3);
    releaseSecond();

    releaseState();
    releaseEffect();
    client.close();
    await server.close();
  });
});

function nextValue<T>(): { promise: Promise<T>; resolve: (value: T) => void } {
  let resolve = (_value: T): void => {};
  const promise = new Promise<T>((next) => {
    resolve = next;
  });
  return { promise, resolve };
}
