import { fileURLToPath } from "node:url";
import { Spellwire, ui } from "../packages/spellwire/src/index";

const app = await Spellwire.start({
  input: fileURLToPath(new URL("../examples/stateful.spellwire.ts", import.meta.url)),
  overlayOptions: { fps: 0 },
  overlay: (state) =>
    ui.column(
      { x: 24, y: 48, width: 240, padding: 12, gap: 8, fill: "#111827ee", radius: 12 },
      ui.text(`phase=${String(state.phase ?? 0)}`, { fontFamily: "monospace" }),
      ui.text(`activations=${String(state.activations ?? 0)}`, { fontFamily: "monospace" }),
    ),
});

try {
  const initialState = app.host.snapshotStates();
  if (app.host.snapshotStates() !== initialState) {
    throw new Error("unchanged native state snapshot was not reused");
  }
  app.host.state("phase").set(2);
  app.host.state("activations").set(7);
  const applied = await app.refreshOverlay();
  const state = app.host.snapshotStates();
  const snapshotReuse = app.host.snapshotStates() === state;
  if (state.phase !== 2 || state.activations !== 7 || applied !== 2 || !snapshotReuse) {
    throw new Error(`unexpected overlay state: ${JSON.stringify({ state, applied })}`);
  }
  await Bun.sleep(100);
  console.log(JSON.stringify({
    platform: process.platform,
    arch: process.arch,
    scaleFactor: app.overlay?.renderer.ready.scaleFactor,
    state,
    applied,
    snapshotReuse,
    overlay: "ok",
  }));
} finally {
  await app.close();
}
