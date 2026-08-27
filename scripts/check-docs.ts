#!/usr/bin/env bun

import { existsSync } from "node:fs";
import { readdir } from "node:fs/promises";
import { dirname, extname, resolve } from "node:path";
import ts from "typescript";

const root = resolve(import.meta.dir, "..");
const ignoredDirectories = new Set([".git", "node_modules", "target", "dist"]);
const markdownFiles: string[] = [];
const failures: string[] = [];
const forbiddenDocumentationPatterns = [
  { pattern: /\bone[- ]page\b/iu, description: "forced one-page wording" },
  { pattern: /한 페이지/u, description: "forced one-page wording" },
  {
    pattern: /packages\/spellwire\/src\/cli\.ts permissions/u,
    description: "removed permissions CLI command",
  },
  {
    pattern: /\b(?:observe|inject): (?:granted|missing)\b/u,
    description: "obsolete permission output format",
  },
] as const;

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
  for (const { pattern, description } of forbiddenDocumentationPatterns) {
    if (pattern.test(text)) failures.push(`${file}: contains ${description}`);
  }

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
      "## Quick lookup",
      "## Public export index",
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
      "## 빠른 찾기",
      "## 공개 export 목록",
      "## 영속 realtime 상태",
      "## 통합 application lifecycle",
      "### UI 생성 함수",
      "### Layout과 visual 속성",
      "### Window 동작",
    ],
  ],
  ["README.md", ["## API at a glance", "[API reference](docs/api.md)"]],
  ["README.ko.md", ["## API 한눈에 보기", "[API 레퍼런스](docs/api.ko.md)"]],
]);

for (const [relativePath, requiredFragments] of requiredApiSurface) {
  const file = resolve(root, relativePath);
  const text = await Bun.file(file).text();
  for (const fragment of requiredFragments) {
    if (!text.includes(fragment)) {
      failures.push(`${file}: missing required API documentation ${JSON.stringify(fragment)}`);
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

function collectPublicExports(source: string, fileName: string): Set<string> {
  const sourceFile = ts.createSourceFile(fileName, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
  const names = new Set<string>();

  for (const statement of sourceFile.statements) {
    if (ts.isExportDeclaration(statement)) {
      if (statement.exportClause && ts.isNamedExports(statement.exportClause)) {
        for (const element of statement.exportClause.elements) names.add(element.name.text);
      }
      continue;
    }

    if (!ts.canHaveModifiers(statement)) continue;
    const isExported = ts
      .getModifiers(statement)
      ?.some((modifier) => modifier.kind === ts.SyntaxKind.ExportKeyword);
    if (!isExported) continue;

    if (
      (ts.isFunctionDeclaration(statement) ||
        ts.isClassDeclaration(statement) ||
        ts.isInterfaceDeclaration(statement) ||
        ts.isTypeAliasDeclaration(statement) ||
        ts.isEnumDeclaration(statement)) &&
      statement.name
    ) {
      names.add(statement.name.text);
      continue;
    }

    if (ts.isVariableStatement(statement)) {
      for (const declaration of statement.declarationList.declarations) {
        if (ts.isIdentifier(declaration.name)) names.add(declaration.name.text);
      }
    }
  }

  return names;
}

const publicExportFiles = [
  "packages/spellwire/src/index.ts",
  "packages/spellwire/src/compiler/index.ts",
] as const;
const publicExports = new Set<string>();
for (const relativePath of publicExportFiles) {
  const file = resolve(root, relativePath);
  const source = await Bun.file(file).text();
  for (const name of collectPublicExports(source, relativePath)) publicExports.add(name);
}

for (const relativePath of ["docs/api.md", "docs/api.ko.md"] as const) {
  const file = resolve(root, relativePath);
  const text = await Bun.file(file).text();
  for (const name of publicExports) {
    if (!new RegExp(`\\b${name}\\b`, "u").test(text)) {
      failures.push(`${file}: missing public export ${name}`);
    }
  }
}

if (failures.length > 0) {
  throw new Error(`documentation validation failed:\n${failures.join("\n")}`);
}

console.log(`documentation ok: ${markdownFiles.length} Markdown files`);
