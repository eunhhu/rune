import { expect, test } from "bun:test";
import {
  Key,
  getFallbackRealtimeRegistrations,
  rt,
  tapKey,
  withRealtimeActionSink,
} from "../src/index";

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

test("fallback hotkeys honor state gates", () => {
  let enabled = false;
  let hits = 0;
  const start = getFallbackRealtimeRegistrations().length;
  rt.hotkey("Ctrl+K", () => {
    hits += 1;
  }, { when: () => enabled });
  const registration = getFallbackRealtimeRegistrations()[start];

  registration?.handler();
  expect(hits).toBe(0);
  enabled = true;
  registration?.handler();
  expect(hits).toBe(1);
});
