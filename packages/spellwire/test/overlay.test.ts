import { describe, expect, test } from "bun:test";

import { OverlayScene } from "../src/overlay";
import { OverlayView, ui } from "../src/overlay-ui";

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
    expect(scene.drainMutations().map((mutation) => mutation.revision)).toEqual([3]);
  });

  test("lays out modern frames without absolute child coordinates", () => {
    const tree = ui.column(
      {
        x: 20,
        y: 24,
        width: 240,
        padding: 16,
        gap: 8,
        fill: "#111827ee",
        radius: 16,
        stroke: "#ffffff22",
        shadow: { fill: "#00000066", y: 8, blur: 20 },
      },
      ui.text("Spellwire", { fontSize: 16, fontWeight: 700 }),
      ui.row(
        { width: "fill", gap: 8 },
        ui.dot({ size: 8, fill: "#34d399ff" }),
        ui.text("Active", { width: "fill" }),
      ),
    );
    const view = new OverlayView(tree);
    expect(view.set(tree)).toBe(4);
    const nodes = [...view.scene.snapshot().values()];
    expect(nodes[0]).toMatchObject({ kind: "rect", x: 20, y: 24, width: 240 });
    expect(nodes[1]).toMatchObject({ kind: "text", x: 36, y: 40, text: "Spellwire" });
    expect(nodes[2]).toMatchObject({ kind: "ellipse", x: 36, y: 67.2, width: 8 });
    expect(nodes[3]).toMatchObject({ kind: "text", x: 52, text: "Active" });
  });

  test("reconciles state bindings only when snapshots change", () => {
    let reads = 0;
    let enabled = false;
    const source = {
      snapshotStates: () => {
        reads += 1;
        return { enabled };
      },
    };
    const tree = ui.bind(source, (state) =>
      ui.text(state.enabled ? "Enabled" : "Paused"),
    );
    const view = new OverlayView(tree);
    expect(view.set(tree)).toBe(1);
    view.scene.drainMutations();
    expect(view.refresh()).toBe(0);
    expect(view.scene.drainMutations()).toEqual([]);
    enabled = true;
    expect(view.refresh()).toBe(1);
    expect(view.scene.drainMutations()[0]?.node).toMatchObject({ text: "Enabled" });
    expect(reads).toBe(3);
  });
});
