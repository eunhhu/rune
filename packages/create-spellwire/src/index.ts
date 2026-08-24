#!/usr/bin/env bun

import { existsSync } from "node:fs";
import { basename, resolve } from "node:path";

export interface CreateSpellwireOptions {
  install?: boolean;
}

export async function createSpellwireProject(
  destination = "spellwire-macro",
  options: CreateSpellwireOptions = {},
): Promise<string> {
  const target = resolve(destination);
  const install = options.install ?? true;

  if (existsSync(target)) {
    throw new Error(`destination already exists: ${target}`);
  }

  await Bun.write(
    `${target}/package.json`,
    `${JSON.stringify(
      {
        name: basename(target),
        private: true,
        type: "module",
        scripts: {
          build: "spellwire compile src/main.spellwire.ts",
          typecheck: "tsc --noEmit",
          check: "bun run typecheck && bun run build",
        },
        dependencies: {
          spellwire: "latest",
        },
        devDependencies: {
          "@types/bun": "latest",
          typescript: "^5.8.3",
        },
      },
      null,
      2,
    )}\n`,
  );

  await Bun.write(
    `${target}/tsconfig.json`,
    `${JSON.stringify(
      {
        compilerOptions: {
          target: "ES2022",
          module: "ESNext",
          moduleResolution: "Bundler",
          strict: true,
          noEmit: true,
          types: ["bun"],
        },
        include: ["src/**/*.ts"],
      },
      null,
      2,
    )}\n`,
  );

  await Bun.write(
    `${target}/src/main.spellwire.ts`,
    `import { Key, rt, tapKey } from "spellwire";\n\n` +
      `let presses = 0;\n\n` +
      `rt.onKeyDown(Key.Q, () => {\n` +
      `  presses += 1;\n` +
      `  if (presses % 2 === 0) tapKey(Key.E);\n` +
      `});\n`,
  );

  await Bun.write(
    `${target}/README.md`,
    `# ${basename(target)}\n\n` +
      `\`\`\`bash\n` +
      `bun run typecheck\n` +
      `bun run build\n` +
      `\`\`\`\n`,
  );
  await Bun.write(`${target}/.gitignore`, "node_modules/\n*.spellwire.bin*\n");

  if (install) {
    const child = Bun.spawn(["bun", "install"], {
      cwd: target,
      stdout: "inherit",
      stderr: "inherit",
    });
    if ((await child.exited) !== 0) {
      throw new Error("bun install failed");
    }
  }

  return target;
}

function printHelp(): void {
  console.log(`Create a Spellwire project

Usage:
  bun create spellwire [directory] [--no-install]
`);
}

if (import.meta.main) {
  const args = Bun.argv.slice(2);
  if (args.includes("--help") || args.includes("-h")) {
    printHelp();
  } else {
    const destination = args.find((arg) => !arg.startsWith("-")) ?? "spellwire-macro";
    try {
      const target = await createSpellwireProject(destination, {
        install: !args.includes("--no-install"),
      });
      console.log(`Created Spellwire project at ${target}`);
      console.log(`Next: cd ${destination} && bun run check`);
    } catch (error) {
      console.error(error instanceof Error ? error.message : error);
      process.exitCode = 1;
    }
  }
}
