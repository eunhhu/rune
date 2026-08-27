import { InputSource, Key, effect, keyDown, keyUp, rt, sleepUs, tapKey } from "spellwire";

let observed = 0;
const loopback = effect("loopback", { observed: "number" });

rt.onKeyDown(
  Key.F19,
  () => {
    tapKey(Key.F20);
  },
  { source: InputSource.Physical },
);

// Exercises cancellation safety: reload/stop must synthesize the missing up before this resumes.
rt.onKeyDown(
  Key.F18,
  () => {
    keyDown(Key.F20);
    sleepUs(1_000_000);
    keyUp(Key.F20);
  },
  { source: InputSource.Physical },
);

rt.onKeyDown(
  Key.F20,
  () => {
    observed++;
    loopback.emit({ observed });
  },
  { source: InputSource.Synthetic },
);
