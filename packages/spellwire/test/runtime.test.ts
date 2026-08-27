import { describe, expect, test } from "bun:test";
import {
  DynamicInputLane,
  EventSource,
  InputDevice,
  InputEdge,
  RuntimeEventLane,
  readRuntimeEventI64,
  type InputEvent,
} from "../src/runtime";

function pushKey(lane: DynamicInputLane, code: number, timestamp = 0): void {
  expect(
    lane.ring.push([
      InputDevice.Keyboard,
      code,
      InputEdge.Down,
      EventSource.Physical,
      timestamp,
      0,
    ]),
  ).toBe(true);
}

describe("DynamicInputLane", () => {
  test("applies subscription mutations only to subsequent events", () => {
    const lane = new DynamicInputLane(4);
    const calls: string[] = [];
    let unsubscribe = (): void => {};
    unsubscribe = lane.on(InputDevice.Keyboard, 4, InputEdge.Down, () => {
      calls.push("first");
      unsubscribe();
    });
    lane.on(InputDevice.Keyboard, 4, InputEdge.Down, () => calls.push("second"));

    pushKey(lane, 4);
    pushKey(lane, 4);
    expect(lane.drain()).toBe(2);
    expect(calls).toEqual(["first", "second", "second"]);
  });

  test("gives retained handlers stable event snapshots", () => {
    const lane = new DynamicInputLane(4);
    const retained: InputEvent[] = [];
    const retain = (event: InputEvent): void => {
      retained.push(event);
    };
    lane.on(InputDevice.Keyboard, 4, InputEdge.Down, retain);
    lane.on(InputDevice.Keyboard, 5, InputEdge.Down, retain);

    pushKey(lane, 4, 10);
    pushKey(lane, 5, 20);
    lane.drain();

    expect(retained[0]).not.toBe(retained[1]);
    expect(retained.map((event) => [event.code, event.timestampLo])).toEqual([
      [4, 10],
      [5, 20],
    ]);
  });

  test("rejects invalid device and edge tuples", () => {
    const lane = new DynamicInputLane(2);
    expect(() => lane.on(-1 as InputDevice, 0, 2 as InputEdge, () => {})).toThrow(RangeError);
  });

  test("rejects reentrant drains", () => {
    const lane = new DynamicInputLane(2);
    lane.on(InputDevice.Keyboard, 4, InputEdge.Down, () => {
      expect(() => lane.drain()).toThrow("not reentrant");
    });
    pushKey(lane, 4);
    expect(lane.drain()).toBe(1);
  });
});

describe("RuntimeEventLane", () => {
  test("delivers changed state and fixed i64 effect payloads", () => {
    const lane = new RuntimeEventLane(4);
    const state: Array<[number, bigint]> = [];
    const effects: Array<[number, bigint, bigint]> = [];
    lane.onState((slot, value) => state.push([slot, value]));
    lane.onEffectRaw((id, length, record, offset) => {
      expect(length).toBe(2);
      effects.push([
        id,
        readRuntimeEventI64(record, offset),
        readRuntimeEventI64(record, offset + 2),
      ]);
    });

    const stateRecord = new Int32Array(20);
    stateRecord.set([1, 3, 1, 0, -1, -1]);
    const effectRecord = new Int32Array(20);
    effectRecord.set([2, 7, 2, 0, 42, 0, 0, -0x8000_0000]);
    expect(lane.ring.push(stateRecord)).toBe(true);
    expect(lane.ring.push(effectRecord)).toBe(true);
    expect(lane.drain()).toBe(2);

    expect(state).toEqual([[3, -1n]]);
    expect(effects).toEqual([[7, 42n, -0x8000_0000_0000_0000n]]);
  });
});
