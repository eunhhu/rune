import { expect, test } from "bun:test";
import { Key, tapKey, withRealtimeActionSink } from "../src";

test("intrinsics have a debuggable JavaScript fallback", () => {
  const events: Array<[number, boolean]> = [];
  withRealtimeActionSink(
    {
      key(code, down) {
        events.push([code, down]);
      },
      mouseButton() {},
      mouseMove() {},
      mouseWheel() {},
      delayUs() {},
      held() {
        return false;
      },
    },
    () => tapKey(Key.E),
  );
  expect(events).toEqual([
    [Key.E, true],
    [Key.E, false],
  ]);
});
