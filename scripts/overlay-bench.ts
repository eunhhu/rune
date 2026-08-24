import { OverlayView, ui } from "../packages/spellwire/src/overlay-ui";

const iterations = Number.parseInt(Bun.argv[2] ?? "20000", 10);
if (!Number.isSafeInteger(iterations) || iterations < 100) {
  throw new RangeError("overlay benchmark iterations must be an integer >= 100");
}

let counter = 0;
const source = { snapshotStates: () => ({ counter, enabled: (counter & 1) === 0 }) };
const root = ui.bind(source, (state) =>
  ui.column(
    {
      x: 24,
      y: 24,
      width: 320,
      padding: 16,
      gap: 8,
      fill: "#111827ee",
      radius: 16,
      stroke: "#ffffff24",
      shadow: { fill: "#00000066", y: 8, blur: 24 },
    },
    ui.text("SPELLWIRE", { fontSize: 12, fontWeight: 700, letterSpacing: 1 }),
    ...Array.from({ length: 12 }, (_, index) =>
      ui.row(
        { key: `row-${index}`, width: "fill", justify: "space-between" },
        ui.text(`Metric ${index}`, { fontSize: 13 }),
        ui.text(index === 0 ? String(state.counter) : String(index * 10), {
          fontFamily: "monospace",
          fontSize: 13,
        }),
      ),
    ),
  ),
);

const view = new OverlayView(root);
view.set(root);
view.scene.drainMutations();
const samples = new BigUint64Array(iterations);
let published = 0;
for (let index = 0; index < iterations; index += 1) {
  counter = index + 1;
  const start = process.hrtime.bigint();
  view.refresh();
  published += view.scene.drainMutations().length;
  samples[index] = process.hrtime.bigint() - start;
}

samples.sort();
const percentile = (value: number): number => {
  const index = Math.min(samples.length - 1, Math.ceil(samples.length * value) - 1);
  return Number(samples[index] ?? 0n);
};

console.log(JSON.stringify({
  iterations,
  primitives: view.scene.snapshot().size,
  published,
  p50Ns: percentile(0.5),
  p95Ns: percentile(0.95),
  p99Ns: percentile(0.99),
}));
