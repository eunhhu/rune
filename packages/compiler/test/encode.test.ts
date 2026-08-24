import { expect, test } from "bun:test";
import { Key } from "@rune/sdk";
import { compileSource, encodeModule, WIRE_VERSION } from "../src";

test("encodes a versioned native module", () => {
  const result = compileSource(`
    import { Key, keyDown, rt } from "@rune/sdk";
    let count = 0;
    rt.onKeyDown(Key.Q, () => { count++; keyDown(Key.E); });
  `);
  const bytes = encodeModule(result.module);
  expect(new TextDecoder().decode(bytes.subarray(0, 4))).toBe("RUNE");
  expect(new DataView(bytes.buffer).getUint16(4, true)).toBe(WIRE_VERSION);
  expect(bytes.byteLength).toBeGreaterThan(24);
});
