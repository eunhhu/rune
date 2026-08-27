#!/usr/bin/env bun

import { existsSync } from "node:fs";
import { readdir } from "node:fs/promises";
import { dirname, extname, resolve } from "node:path";
import ts from "typescript";

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

const requiredApiSurface = new Map<string, readonly string[]>([
  [
    "docs/api.md",
    [
      "## Find an API without leaving this page",
      "## Persistent realtime state",
      "## Unified application lifecycle",
      "### UI constructors",
      "### Layout and visual properties",
      "### Window behavior",
    ],
  ],
  [
    "docs/api.ko.md",
    [
      "## 이 페이지에서 바로 찾기",
      "## 영속 realtime 상태",
      "## 통합 application lifecycle",
      "### UI 생성 함수",
      "### Layout과 visual 속성",
      "### Window 동작",
    ],
  ],
  ["README.md", ["## API at a glance", "[one-page API reference](docs/api.md)"]],
  ["README.ko.md", ["## API 한눈에 보기", "[한 페이지 API 레퍼런스](docs/api.ko.md)"]],
]);

for (const [relativePath, requiredFragments] of requiredApiSurface) {
  const file = resolve(root, relativePath);
  const text = await Bun.file(file).text();
  for (const fragment of requiredFragments) {
    if (!text.includes(fragment)) {
      failures.push(`${file}: missing one-page API navigation contract ${JSON.stringify(fragment)}`);
    }
  }

  if (relativePath === "docs/api.md" || relativePath === "docs/api.ko.md") {
    let block = 0;
    for (const match of text.matchAll(/```ts\r?\n([\s\S]*?)```/g)) {
      block += 1;
      const source = match[1] ?? "";
      const sourceFile = ts.createSourceFile(
        `${relativePath}:typescript-block-${block}`,
        source,
        ts.ScriptTarget.Latest,
        true,
        ts.ScriptKind.TS,
      );
      for (const diagnostic of sourceFile.parseDiagnostics) {
        failures.push(
          `${file}: TypeScript block ${block}: ${ts.flattenDiagnosticMessageText(
            diagnostic.messageText,
            "\n",
          )}`,
        );
      }
    }
  }
}

if (failures.length > 0) {
  throw new Error(`documentation validation failed:\n${failures.join("\n")}`);
}

console.log(`documentation ok: ${markdownFiles.length} Markdown files`);
