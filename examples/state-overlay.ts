import { fileURLToPath } from "node:url";
import { Spellwire, ui } from "../packages/spellwire/src/index";

const app = await Spellwire.start({
  input: fileURLToPath(new URL("./stateful.spellwire.ts", import.meta.url)),
  watch: true,
  overlay: (state) => {
    const enabled = state.enabled === true;
    return ui.column(
      {
        x: 24,
        y: 48,
        width: 300,
        padding: 16,
        gap: 12,
        fill: "#111827ee",
        radius: 16,
        stroke: "#ffffff24",
        shadow: { fill: "#00000066", y: 8, blur: 24 },
      },
      ui.text("SPELLWIRE", {
        fill: "#94a3b8ff",
        fontSize: 12,
        fontWeight: 700,
        letterSpacing: 1,
      }),
      ui.row(
        { width: "fill", gap: 8, align: "center" },
        ui.dot({ size: 8, fill: enabled ? "#34d399ff" : "#fb7185ff" }),
        ui.text(enabled ? "Active" : "Paused", {
          width: "fill",
          fill: "#ffffffff",
          fontSize: 16,
          fontWeight: 600,
        }),
        ui.badge("F8"),
      ),
      ui.divider(),
      ui.row(
        { width: "fill", justify: "space-between" },
        ui.text("Phase", { fill: "#94a3b8ff", fontSize: 13 }),
        ui.text(String(state.phase ?? 0), {
          fill: "#ffffffff",
          fontFamily: "monospace",
          fontSize: 13,
          fontWeight: 600,
        }),
      ),
      ui.row(
        { width: "fill", justify: "space-between" },
        ui.text("Activations", { fill: "#94a3b8ff", fontSize: 13 }),
        ui.text(String(state.activations ?? 0), {
          fill: "#ffffffff",
          fontFamily: "monospace",
          fontSize: 13,
          fontWeight: 600,
        }),
      ),
    );
  },
});

await app.untilSignal();
