import { expect, test } from "bun:test";
import { rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { createSpellwireProject } from "../src/index";

test("creates a buildable Spellwire project", async () => {
  const target = join(tmpdir(), `spellwire-${crypto.randomUUID()}`);

  try {
    await createSpellwireProject(target, { install: false });

    const packageJson = await Bun.file(join(target, "package.json")).json();
    expect(packageJson.dependencies.spellwire).toBe("latest");
    expect(Object.keys(packageJson.scripts)).toEqual(["start", "watch", "build"]);
    expect(packageJson.scripts.start).toBe("bun src/app.ts");
    expect(packageJson.scripts.watch).toBe("bun src/app.ts --watch");
    expect(packageJson.scripts.build).toBe(
      "spellwire compile src/main.spellwire.ts dist/main.spellwire.bin",
    );
    expect(await Bun.file(join(target, "src/main.spellwire.ts")).exists()).toBe(true);
    const app = await Bun.file(join(target, "src/app.ts")).text();
    expect(app).toContain("Spellwire.start");
    expect(app).toContain("ui.column");
    expect(app).toContain("overlay: (state)");
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
