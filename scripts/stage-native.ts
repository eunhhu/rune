#!/usr/bin/env bun

import { createHash } from "node:crypto";
import { chmod, copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";

const releaseDirectory = resolve("target", "release");
const platformDirectory = `${process.platform}-${process.arch}`;
const nativeRoot = resolve(
  process.env.SPELLWIRE_STAGE_ROOT ?? join("packages", "spellwire", "native"),
);
const destination = join(nativeRoot, platformDirectory);
const library =
  process.platform === "win32"
    ? "spellwire_native.dll"
    : process.platform === "darwin"
      ? "libspellwire_native.dylib"
      : "libspellwire_native.so";
const overlay = process.platform === "win32" ? "spellwire-overlay.exe" : "spellwire-overlay";

await mkdir(destination, { recursive: true });
for (const file of [library, overlay]) {
  await copyFile(join(releaseDirectory, file), join(destination, file));
}
if (process.platform !== "win32") await chmod(join(destination, overlay), 0o755);

const checksums: string[] = [];
for (const file of [library, overlay]) {
  const digest = createHash("sha256").update(await readFile(join(destination, file))).digest("hex");
  checksums.push(`${digest}  ${file}`);
}
await writeFile(join(destination, "SHA256SUMS"), `${checksums.join("\n")}\n`);
console.log(destination);
