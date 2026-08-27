import { expect, test } from "bun:test";
import { rm } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
import { tmpdir } from "node:os";

import { runCli } from "../src/cli";

const spellwireCli = fileURLToPath(new URL("../src/cli.ts", import.meta.url));

test("compile creates an explicit output directory and state manifest", async () => {
  const target = join(tmpdir(), `spellwire-cli-${crypto.randomUUID()}`);
  const input = join(target, "src", "main.ts");
  const output = join(target, "dist", "main.spellwire.bin");

  try {
    await Bun.write(
      input,
      `import { Key, effect, rt, tapKey } from "spellwire";\n` +
        `let count = 0;\n` +
        `const changed = effect("changed", { count: "number" });\n` +
        `rt.onKeyDown(Key.Q, () => { count += 1; changed.emit({ count }); tapKey(Key.E); });\n`,
    );

    await runCli(["compile", input, output]);

    expect(await Bun.file(output).exists()).toBe(true);
    const manifest = await Bun.file(`${output}.json`).json();
    expect(manifest.handlers).toBe(1);
    expect(manifest.states.count).toEqual({ slot: 0, kind: "number" });
    expect(manifest.effects.changed).toEqual({
      id: 0,
      fields: [{ name: "count", kind: "number" }],
    });

    const defaultCompile = Bun.spawn([process.execPath, spellwireCli, "compile"], {
      cwd: target,
      stdout: "ignore",
      stderr: "pipe",
    });
    const [exitCode, stderr] = await Promise.all([
      defaultCompile.exited,
      new Response(defaultCompile.stderr).text(),
    ]);
    if (exitCode !== 0) throw new Error(`default input failed: ${stderr.trim()}`);
    expect(await Bun.file(join(target, "src", "main.spellwire.bin")).exists()).toBe(true);
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
