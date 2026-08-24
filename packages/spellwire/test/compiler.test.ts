import { describe, expect, test } from "bun:test";
import { Key, MouseButton } from "../src/index";
import { compileSource, Opcode, SourceFilter, SpellwireCompileError } from "../src/index";

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

  test("rejects TypeScript syntax errors before lowering handlers", () => {
    const source = `
      import { Key, rt } from "../src/index";
      rt.onKeyDown(Key.Q, () => {});
    }`;
    expect(() => compileSource(source)).toThrow(SpellwireCompileError);
  });

  test("resolves shorthand, quoted, and computed source options", () => {
    const shorthand = compileSource(`
      import { InputSource, Key, rt } from "../src/index";
      const source = InputSource.Synthetic;
      rt.onKeyDown(Key.Q, () => {}, { source });
    `);
    const quoted = compileSource(`
      import { InputSource, Key, rt } from "../src/index";
      rt.onKeyDown(Key.Q, () => {}, { "source": InputSource.Any });
    `);
    const computed = compileSource(`
      import { InputSource, Key, rt } from "../src/index";
      rt.onKeyDown(Key.Q, () => {}, { ["source"]: InputSource.Physical });
    `);

    expect(shorthand.module.handlers[0]?.source).toBe(SourceFilter.Synthetic);
    expect(quoted.module.handlers[0]?.source).toBe(SourceFilter.Any);
    expect(computed.module.handlers[0]?.source).toBe(SourceFilter.Physical);
  });

  test("rejects source option shapes that would otherwise change trigger semantics", () => {
    const spread = `
      import { InputSource, Key, rt } from "../src/index";
      const options = { source: InputSource.Synthetic };
      rt.onKeyDown(Key.Q, () => {}, { ...options });
    `;
    const misspelled = `
      import { InputSource, Key, rt } from "../src/index";
      rt.onKeyDown(Key.Q, () => {}, { soruce: InputSource.Synthetic });
    `;
    expect(() => compileSource(spread)).toThrow(SpellwireCompileError);
    expect(() => compileSource(misspelled)).toThrow(SpellwireCompileError);
  });

  test("rejects unsigned shifts because realtime values are signed i64", () => {
    const source = `
      import { Key, rt } from "../src/index";
      let value = -1;
      rt.onKeyDown(Key.Q, () => { value = value >>> 1; });
    `;
    expect(() => compileSource(source)).toThrow(SpellwireCompileError);
  });

  test("rejects trigger codes outside native handler-table ranges", () => {
    const invalidKey = `
      import { Key, rt } from "../src/index";
      rt.onKeyDown(999 as Key, () => {});
    `;
    const invalidMouse = `
      import { MouseButton, rt } from "../src/index";
      rt.onMouseDown(8 as MouseButton, () => {});
    `;
    expect(() => compileSource(invalidKey)).toThrow(SpellwireCompileError);
    expect(() => compileSource(invalidMouse)).toThrow(SpellwireCompileError);
  });

  test("rejects resource options outside native VM limits", () => {
    const source = `rt.onKeyDown(20, () => {});`;
    expect(() => compileSource(source, { stackLimit: 0 })).toThrow(SpellwireCompileError);
    expect(() => compileSource(source, { stackLimit: 257 })).toThrow(SpellwireCompileError);
    expect(() => compileSource(source, { instructionBudget: 0 })).toThrow(SpellwireCompileError);
    expect(() => compileSource(source, { instructionBudget: 0x1_0000_0000 })).toThrow(
      SpellwireCompileError,
    );
  });
});
