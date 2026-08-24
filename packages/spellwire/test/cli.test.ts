import { expect, test } from "bun:test";
import { rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";

import { runCli } from "../src/cli";

test("compile creates an explicit output directory and state manifest", async () => {
  const target = join(tmpdir(), `spellwire-cli-${crypto.randomUUID()}`);
  const input = join(target, "src", "main.spellwire.ts");
  const output = join(target, "dist", "main.spellwire.bin");

  try {
    await Bun.write(
      input,
      `import { Key, rt, tapKey } from "spellwire";\n` +
        `let count = 0;\n` +
        `rt.onKeyDown(Key.Q, () => { count += 1; tapKey(Key.E); });\n`,
    );

    await runCli(["compile", input, output]);

    expect(await Bun.file(output).exists()).toBe(true);
    const manifest = await Bun.file(`${output}.json`).json();
    expect(manifest.handlers).toBe(1);
    expect(manifest.states.count).toEqual({ slot: 0, kind: "number" });
  } finally {
    await rm(target, { recursive: true, force: true });
  }
});

test("rejects commands outside run, watch, and compile workflows", async () => {
  for (const args of [
    ["wat"],
    ["permissions"],
    ["src/main.spellwire.ts"],
    ["run", "--watch"],
  ]) {
    await expect(runCli(args)).rejects.toThrow(/unknown Spellwire (command|option)/);
  }
});
