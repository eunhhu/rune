import { describe, expect, test } from "bun:test";

import { OverlayScene } from "../src/overlay";

describe("OverlayScene", () => {
  test("retains cloned nodes and drains only mutations", () => {
    const scene = new OverlayScene();
    const source = {
      kind: "rect" as const,
      x: 1,
      y: 2,
      width: 30,
      height: 40,
      radius: 5,
      color: "#11223344",
    };
    const id = scene.create(source);
    source.x = 99;

    const retained = scene.snapshot().get(id);
    expect(retained?.kind).toBe("rect");
    if (retained?.kind !== "rect") throw new Error("expected retained rect");
    expect(retained.x).toBe(1);
    expect(scene.drainMutations()).toEqual([
      { revision: 1, id, node: { ...source, x: 1 } },
    ]);
    expect(scene.drainMutations()).toEqual([]);

    scene.update(id, { kind: "text", x: 8, y: 9, text: "ok", size: 16 });
    expect(scene.remove(id)).toBe(true);
    expect(scene.remove(id)).toBe(false);
    expect(scene.drainMutations().map((mutation) => mutation.revision)).toEqual([2, 3]);
  });
});
