import { expect, test } from "bun:test";
import { rm } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { createSpellwireProject } from "../src/index";

const spellwireCli = fileURLToPath(new URL("../../spellwire/src/cli.ts", import.meta.url));

test("creates a buildable Spellwire project", async () => {
  const target = join(tmpdir(), `spellwire-${crypto.randomUUID()}`);

  try {
    await createSpellwireProject(target, { install: false });

    const packageJson = await Bun.file(join(target, "package.json")).json();
    expect(packageJson.dependencies.spellwire).toBe("latest");
    expect(Object.keys(packageJson.scripts)).toEqual(["start", "watch", "build"]);
    expect(packageJson.scripts.start).toBe("bun src/main.ts");
    expect(packageJson.scripts.watch).toBe("bun src/main.ts --watch");
    expect(packageJson.scripts.build).toBe(
      "spellwire compile src/main.ts dist/main.spellwire.bin",
    );
    const source = await Bun.file(join(target, "src/main.ts")).text();
    expect(source).toContain('rt.hotkey("Q"');
    expect(source).toContain("when: () => enabled");
    expect(source).toContain("Spellwire.start");
    expect(source).toContain("input: import.meta.file");
    expect(source).toContain("ui.column");
    expect(source).toContain("overlay: (state)");
    const output = join(target, "dist/main.spellwire.bin");
    const compiler = Bun.spawn(
      [
        process.execPath,
        spellwireCli,
        "compile",
        join(target, "src/main.ts"),
        output,
      ],
      { stdout: "ignore", stderr: "pipe" },
    );
    const [exitCode, stderr] = await Promise.all([
      compiler.exited,
      new Response(compiler.stderr).text(),
    ]);
    if (exitCode !== 0) throw new Error(`generated project failed to compile: ${stderr.trim()}`);
    expect(await Bun.file(output).exists()).toBe(true);
    expect(await Bun.file(`${output}.json`).exists()).toBe(true);
    expect(await Bun.file(join(target, "src/app.ts")).exists()).toBe(false);
    const readme = await Bun.file(join(target, "README.md")).text();
    expect(readme).toContain("bun run start");
    expect(readme).toContain("bun run watch");
    expect(readme).toContain("bun run build");
    const koreanReadme = await Bun.file(join(target, "README.ko.md")).text();
    expect(koreanReadme).toContain("bun run start");
    expect(koreanReadme).toContain("bun run watch");
    expect(koreanReadme).toContain("bun run build");
    expect(await Bun.file(join(target, ".gitignore")).text()).toContain("dist/");
  } finally {
    await rm(target, { recursive: true, force: true });
  }
});
