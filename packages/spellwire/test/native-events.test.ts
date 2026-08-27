import { expect, test } from "bun:test";

import { NativeEffects, type NativeManifest } from "../src/native";

test("NativeEffects keeps duplicate callback subscriptions independent", () => {
  const manifest: NativeManifest = {
    version: 1,
    states: Object.freeze({}),
    effects: Object.freeze(Object.fromEntries([
      ["__proto__", { id: 7, fields: [{ name: "__proto__", kind: "number" as const }] }],
    ])),
  };
  let consumers = 0;
  const effects = new NativeEffects(
    () => manifest,
    () => {
      consumers += 1;
      let active = true;
      return () => {
        if (!active) return;
        active = false;
        consumers -= 1;
      };
    },
  );
  const values: number[] = [];
  const handler = (payload: Readonly<Record<string, number | boolean>>): void => {
    values.push(payload.__proto__ as number);
  };
  const releaseFirst = effects.on("__proto__", handler);
  const releaseSecond = effects.on("__proto__", handler);
  const record = new Int32Array(20);
  record[4] = 9;

  effects.dispatch(7, 1, record, 4);
  releaseFirst();
  effects.dispatch(7, 1, record, 4);
  releaseSecond();

  expect(values).toEqual([9, 9, 9]);
  expect(consumers).toBe(0);
});
