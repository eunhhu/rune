import { describe, expect, test } from "bun:test";
import { Key, MouseButton } from "../src/index";
import {
  compileSource,
  Modifier,
  Opcode,
  SourceFilter,
  SpellwireCompileError,
  TriggerFlag,
} from "../src/index";

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

  test("fuses discarded state assignments and updates into direct opcodes", () => {
    const result = compileSource(`
      import { Key, rt } from "../src/index";
      let count = 0;
      let enabled = true;
      let mask = 0;
      rt.onKeyDown(Key.Q, () => {
        count++;
        count += 2;
        count -= 1;
        enabled = !enabled;
        mask ^= 4;
        count = 0;
      });
    `);

    expect(result.module.code.map(({ opcode, a, immediate }) => ({ opcode, a, immediate }))).toEqual([
      { opcode: Opcode.AddStateImm, a: 0, immediate: 1n },
      { opcode: Opcode.AddStateImm, a: 0, immediate: 2n },
      { opcode: Opcode.AddStateImm, a: 0, immediate: -1n },
      { opcode: Opcode.ToggleState, a: 1, immediate: 0n },
      { opcode: Opcode.XorStateImm, a: 2, immediate: 4n },
      { opcode: Opcode.StoreStateImm, a: 0, immediate: 0n },
      { opcode: Opcode.Halt, a: 0, immediate: 0n },
    ]);
  });

  test("lowers every delay unit to one wide native delay opcode", () => {
    const result = compileSource(`
      import { Key, rt, sleep, sleepHours, sleepMs } from "../src/index";
      let seconds = 2;
      rt.onKeyDown(Key.Q, () => {
        sleepMs(250);
        sleep.seconds(3);
        sleepHours(2);
        sleep.seconds(seconds);
      });
    `);
    const delays = result.module.code.filter(({ opcode }) => opcode === Opcode.DelayUs);
    expect(delays).toHaveLength(4);
    expect(delays.slice(0, 3).map(({ flags, immediate }) => ({ flags, immediate }))).toEqual([
      { flags: 0x40, immediate: 250_000n },
      { flags: 0x40, immediate: 3_000_000n },
      { flags: 0x40, immediate: 7_200_000_000n },
    ]);
    expect(delays[3]).toMatchObject({ flags: 0xc0, immediate: 1_000_000n });
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

  test("lowers portable chords and paired consuming remaps", () => {
    const result = compileSource(`
      import { Key, rt, tapKey } from "../src/index";
      const chord = "Ctrl+Shift+K";
      rt.hotkey(chord, () => tapKey(Key.E), { repeat: false });
      rt.remap("CapsLock", "Escape");
    `);

    expect(result.module.handlers).toHaveLength(3);
    expect(result.module.handlers[0]).toMatchObject({
      code: Key.K,
      modifiers: Modifier.Control | Modifier.Shift,
      flags: TriggerFlag.Consume | TriggerFlag.ExactModifiers | TriggerFlag.IgnoreRepeat,
    });
    expect(result.module.handlers.slice(1).map((handler) => handler.code)).toEqual([
      Key.CapsLock,
      Key.CapsLock,
    ]);
    expect(result.module.handlers.slice(1).every(
      (handler) => (handler.flags & TriggerFlag.Consume) !== 0,
    )).toBe(true);
    expect(result.module.code.some(
      (instruction) => instruction.opcode === Opcode.KeyDown && instruction.a === Key.Escape,
    )).toBe(true);
    expect(result.module.code.some(
      (instruction) => instruction.opcode === Opcode.KeyUp && instruction.a === Key.Escape,
    )).toBe(true);
  });

  test("lowers state-gated release hotkeys and conditional remaps", () => {
    const result = compileSource(`
      import { Key, rt, tapKey } from "../src/index";
      let enabled = true;
      rt.hotkey("Ctrl+K", () => tapKey(Key.E), { edge: "up", when: () => enabled });
      rt.remap(Key.CapsLock, Key.Escape, { when: () => !enabled });
    `);

    expect(result.module.handlers[0]).toMatchObject({
      edge: 1,
      gate: 0,
      flags: TriggerFlag.Consume | TriggerFlag.ExactModifiers,
    });
    expect(result.module.handlers.slice(1).every(
      (handler) => handler.gate === 0 && (handler.flags & TriggerFlag.GateInverted) !== 0,
    )).toBe(true);
  });

  test("rejects dynamic or malformed chord policies", () => {
    expect(() => compileSource(`
      import { rt } from "../src/index";
      const chord = process.argv[2];
      rt.hotkey(chord, () => {});
    `)).toThrow("constant string");
    expect(() => compileSource(`
      import { rt } from "../src/index";
      rt.hotkey("Ctrl+K+L", () => {});
    `)).toThrow("multiple trigger keys");
    expect(() => compileSource(`
      import { Key, rt } from "../src/index";
      rt.remap(Key.A, Key.B, { consume: false });
    `)).toThrow("Unsupported realtime handler option");
    expect(() => compileSource(`
      import { rt } from "../src/index";
      let mode = 1;
      rt.hotkey("K", () => {}, { when: () => mode > 0 });
    `)).toThrow("native boolean state");
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
