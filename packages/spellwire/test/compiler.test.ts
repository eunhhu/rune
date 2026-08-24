import { describe, expect, test } from "bun:test";
import { Key, MouseButton } from "../src/index";
import { compileSource, Opcode, SpellwireCompileError } from "../src/index";

const stateful = `
import { Key, MouseButton, clickMouse, keyDown, keyUp, rt, sleepUs } from "../src/index";

let phase = 0;
let enabled = true;
const dynamicOnly = { label: "kept in Bun" };

function emitBurst(key: Key, count: number): void {
  for (let i = 0; i < count; i++) {
    keyDown(key);
    keyUp(key);
  }
}

rt.onKeyDown(Key.Q, () => {
  if (!enabled) return;
  phase = (phase + 1) % 3;
  emitBurst(Key.E, phase + 1);
  if (phase === 2) {
    clickMouse(MouseButton.Left);
    sleepUs(80);
  }
});
`;

describe("Spellwire TypeScript AOT compiler", () => {
  test("lowers persistent state, conditions, loops, and helper functions", () => {
    const result = compileSource(stateful, { fileName: "stateful.spellwire.ts" });
    expect(result.module.handlers).toHaveLength(1);
    expect(result.module.states.map((state) => state.name)).toEqual(["phase", "enabled"]);
    expect(result.module.code.some((instruction) => instruction.opcode === Opcode.StoreState)).toBe(true);
    expect(result.module.code.some((instruction) => instruction.opcode === Opcode.JumpIfFalse)).toBe(true);
    expect(result.module.code.some((instruction) => instruction.opcode === Opcode.KeyDown)).toBe(true);
    expect(result.module.code.at(-1)?.opcode).toBe(Opcode.Halt);
  });

  test("ignores unrelated dynamic TypeScript until realtime code captures it", () => {
    const source = `
      import { Key, rt } from "../src/index";
      const object = { value: 1 };
      let dynamic = new Map();
      console.log(object, dynamic);
      rt.onKeyDown(Key.Q, () => {});
    `;
    expect(() => compileSource(source)).not.toThrow();
  });

  test("rejects dynamic values captured by a realtime handler", () => {
    const source = `
      import { Key, rt } from "../src/index";
      const object = { value: 1 };
      rt.onKeyDown(Key.Q, () => { if (object.value) {} });
    `;
    expect(() => compileSource(source)).toThrow(SpellwireCompileError);
  });
});
