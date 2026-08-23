import { Key, MouseButton, macro, rune } from "@rune/sdk";

const lunge = macro("lunge", (m) => {
  m.on.keyDown(Key.Q).run(
    m.key.down(Key.E),
    m.mouse.down(MouseButton.Left),
    m.delay.us(80),
    m.mouse.up(MouseButton.Left),
    m.key.up(Key.E),
  );
});

rune.configure({ spinThresholdUs: 100 }).load(lunge).start();
console.log("Rune is running. Press Ctrl+C to stop.");

process.on("SIGINT", () => {
  rune.close();
  process.exit(0);
});
