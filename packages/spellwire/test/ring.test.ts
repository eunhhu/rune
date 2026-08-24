import { describe, expect, test } from "bun:test";
import { SpscInt32Ring } from "../src/ring";

describe("SpscInt32Ring", () => {
  test("moves fixed records without allocating in the ring", () => {
    const ring = new SpscInt32Ring(4, 3);
    expect(ring.push([1, 2, 3])).toBe(true);
    expect(ring.push([4, 5, 6])).toBe(true);
    const target = new Int32Array(3);
    expect(ring.pop(target)).toBe(true);
    expect([...target]).toEqual([1, 2, 3]);
    expect(ring.pop(target)).toBe(true);
    expect([...target]).toEqual([4, 5, 6]);
    expect(ring.pop(target)).toBe(false);
  });

  test("counts overflow instead of overwriting unread records", () => {
    const ring = new SpscInt32Ring(2, 1);
    expect(ring.push([1])).toBe(true);
    expect(ring.push([2])).toBe(true);
    expect(ring.push([3])).toBe(false);
    expect(ring.dropped).toBe(1);
  });
});
