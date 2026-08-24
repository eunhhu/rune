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
    expect(packageJson.scripts.typecheck).toBe("tsc --noEmit");
    expect(packageJson.scripts.build).toContain("spellwire compile");
    expect(await Bun.file(join(target, "src/main.spellwire.ts")).exists()).toBe(true);
    expect(await Bun.file(join(target, "README.md")).exists()).toBe(true);
  } finally {
    await rm(target, { recursive: true, force: true });
  }
});
