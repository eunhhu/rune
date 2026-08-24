import {
  InputSource,
  Key,
  MouseButton,
  clickMouse,
  keyDown,
  keyHeld,
  keyUp,
  rt,
  sleepUs,
} from "spellwire";

// Module-scope integer/boolean `let`s captured by a realtime handler become
// persistent native state slots. They survive every input dispatch.
let phase = 0;
let enabled = true;
let activations = 0;

// Normal helper functions are inlined into native bytecode.
function tapRepeated(key: Key, count: number): void {
  for (let index = 0; index < count; index++) {
    keyDown(key);
    keyUp(key);
  }
}

rt.onKeyDown(
  Key.Q,
  () => {
    if (!enabled || keyHeld(Key.LeftShift)) return;

    activations++;
    phase = (phase + 1) % 3;
    tapRepeated(Key.E, phase + 1);

    if (phase === 2) {
      clickMouse(MouseButton.Left);
      sleepUs(80);
    }
  },
  { source: InputSource.Physical },
);

rt.onKeyDown(Key.F8, () => {
  enabled = !enabled;
});
