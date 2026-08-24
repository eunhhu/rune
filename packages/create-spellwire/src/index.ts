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
          start: "spellwire run",
          watch: "spellwire watch",
          build: "spellwire compile src/main.spellwire.ts dist/main.spellwire.bin",
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
      `Spellwire compiles realtime handlers to the native VM before live input starts.\n\n` +
      `\`\`\`bash\n` +
      `# Run once\n` +
      `bun run start\n\n` +
      `# Run and reload after source changes\n` +
      `bun run watch\n\n` +
      `# Build dist/main.spellwire.bin and its state manifest\n` +
      `bun run build\n` +
      `\`\`\`\n\n` +
      `Edit \`src/main.spellwire.ts\`. The first live run requests required global ` +
      `input permissions. Press \`Ctrl+C\` to stop and release held synthetic input.\n`,
  );
  await Bun.write(
    `${target}/README.ko.md`,
    `# ${basename(target)}\n\n` +
      `Spellwire는 live input 시작 전에 realtime handler를 native VM으로 컴파일합니다.\n\n` +
      `\`\`\`bash\n` +
      `# 한 번 실행\n` +
      `bun run start\n\n` +
      `# source 변경을 hot reload하며 실행\n` +
      `bun run watch\n\n` +
      `# dist/main.spellwire.bin과 상태 manifest 생성\n` +
      `bun run build\n` +
      `\`\`\`\n\n` +
      `\`src/main.spellwire.ts\`를 편집하십시오. 첫 live run은 필요한 전역 입력 권한을 ` +
      `요청합니다. \`Ctrl+C\`로 종료하면 눌린 상태의 합성 입력도 해제합니다.\n`,
  );
  await Bun.write(`${target}/.gitignore`, "node_modules/\ndist/\n*.spellwire.bin*\n");

  if (install) {
    const child = Bun.spawn([process.execPath, "install"], {
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
      console.log(`Next: cd ${destination} && bun run start`);
    } catch (error) {
      console.error(error instanceof Error ? error.message : error);
      process.exitCode = 1;
    }
  }
}
