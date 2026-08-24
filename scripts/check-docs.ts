#!/usr/bin/env bun

import { existsSync } from "node:fs";
import { readdir } from "node:fs/promises";
import { dirname, extname, resolve } from "node:path";

const root = resolve(import.meta.dir, "..");
const ignoredDirectories = new Set([".git", "node_modules", "target", "dist"]);
const markdownFiles: string[] = [];
const failures: string[] = [];

async function collect(directory: string): Promise<void> {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (entry.isDirectory() && ignoredDirectories.has(entry.name)) continue;
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) await collect(path);
    else if (entry.isFile() && extname(entry.name) === ".md") markdownFiles.push(path);
  }
}

await collect(root);

for (const file of markdownFiles) {
  const counterpart = file.endsWith(".ko.md")
    ? file.replace(/\.ko\.md$/u, ".md")
    : file.replace(/\.md$/u, ".ko.md");
  if (!existsSync(counterpart)) {
    failures.push(`${file}: missing bilingual counterpart ${counterpart}`);
  }

  const text = await Bun.file(file).text();
  const fenceCount = text.match(/^```/gm)?.length ?? 0;
  if (fenceCount % 2 !== 0) failures.push(`${file}: unbalanced fenced code blocks`);

  for (const match of text.matchAll(/!?(?:\[[^\]]*\])\(([^)]+)\)/g)) {
    const rawTarget = match[1]?.trim();
    if (!rawTarget || /^(?:https?:|mailto:|#)/.test(rawTarget)) continue;
    const targetWithoutTitle = rawTarget.split(/\s+["']/u, 1)[0] ?? rawTarget;
    const path = decodeURIComponent(targetWithoutTitle.split("#", 1)[0] ?? "");
    if (!path || path.startsWith("/")) continue;
    const resolved = resolve(dirname(file), path);
    if (!existsSync(resolved)) failures.push(`${file}: missing local link ${rawTarget}`);
  }
}

if (failures.length > 0) {
  throw new Error(`documentation validation failed:\n${failures.join("\n")}`);
}

console.log(`documentation ok: ${markdownFiles.length} Markdown files`);
