import { expect, test } from "bun:test";
import {
  Key,
  getFallbackRealtimeRegistrations,
  rt,
  sleep,
  sleepHours,
  sleepMinutes,
  sleepMs,
  sleepSeconds,
  sleepUs,
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

test("delay helpers lower to microseconds in the JavaScript fallback", () => {
  const delays: number[] = [];
  withRealtimeActionSink(
    {
      key() {},
      mouseButton() {},
      mouseMove() {},
      mouseWheel() {},
      delayUs(duration) {
        delays.push(duration);
      },
      held() {
        return false;
      },
    },
    () => {
      sleepUs(7);
      sleepMs(2);
      sleepSeconds(3);
      sleepMinutes(1);
      sleepHours(1);
      sleep.ms(4);
      sleep.seconds(5);
    },
  );
  expect(delays).toEqual([
    7,
    2_000,
    3_000_000,
    60_000_000,
    3_600_000_000,
    4_000,
    5_000_000,
  ]);
  expect(() => withRealtimeActionSink(
    {
      key() {}, mouseButton() {}, mouseMove() {}, mouseWheel() {}, delayUs() {}, held: () => false,
    },
    () => sleepMs(-1),
  )).toThrow(RangeError);
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
