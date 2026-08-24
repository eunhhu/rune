import { rt } from "spellwire";

let hits = 0;
let enabled = false;

// The K transition must reach the native VM but not the focused application.
rt.hotkey("K", () => {
  hits++;
}, { when: () => enabled });
