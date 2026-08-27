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
          start: "bun src/main.ts",
          watch: "bun src/main.ts --watch",
          build: "spellwire compile src/main.ts dist/main.spellwire.bin",
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
    `${target}/src/main.ts`,
    `import { Key, Spellwire, rt, tapKey, ui } from "spellwire";\n\n` +
      `let enabled = true;\n` +
      `let presses = 0;\n\n` +
      `rt.hotkey("Q", () => {\n` +
      `  presses += 1;\n` +
      `  if (presses % 2 === 0) tapKey(Key.E);\n` +
      `}, { when: () => enabled });\n\n` +
      `rt.hotkey("F8", () => {\n` +
      `  enabled = !enabled;\n` +
      `}, { consume: false });\n\n` +
      `const app = await Spellwire.start({\n` +
      `  input: import.meta.file,\n` +
      `  watch: Bun.argv.includes("--watch"),\n` +
      `  overlay: (state) => {\n` +
      `    const enabled = state.enabled === true;\n` +
      `    return ui.column(\n` +
      `      {\n` +
      `        x: 24, y: 48, width: 280, padding: 16, gap: 12,\n` +
      `        fill: "#111827ee", radius: 16, stroke: "#ffffff24",\n` +
      `        shadow: { fill: "#00000066", y: 8, blur: 24 },\n` +
      `      },\n` +
      `      ui.text("SPELLWIRE", {\n` +
      `        fill: "#94a3b8ff", fontSize: 12, fontWeight: 700, letterSpacing: 1,\n` +
      `      }),\n` +
      `      ui.row(\n` +
      `        { width: "fill", gap: 8, align: "center" },\n` +
      `        ui.dot({ size: 8, fill: enabled ? "#34d399ff" : "#fb7185ff" }),\n` +
      `        ui.text(enabled ? "Active" : "Paused", {\n` +
      `          width: "fill", fill: "#ffffffff", fontSize: 16, fontWeight: 600,\n` +
      `        }),\n` +
      `        ui.badge("F8"),\n` +
      `      ),\n` +
      `      ui.text(\`Q presses: \${String(state.presses ?? 0)}\`, {\n` +
      `        fill: "#cbd5e1ff", fontFamily: "monospace", fontSize: 13,\n` +
      `      }),\n` +
      `    );\n` +
      `  },\n` +
      `});\n\n` +
      `await app.untilSignal();\n`,
  );

  await Bun.write(
    `${target}/README.md`,
    `# ${basename(target)}\n\n` +
      `[한국어](README.ko.md)\n\n` +
      `Spellwire compiles realtime handlers to the native VM before live input starts.\n\n` +
      `\`\`\`bash\n` +
      `# Run once\n` +
      `bun run start\n\n` +
      `# Run and reload after source changes\n` +
      `bun run watch\n\n` +
      `# Build dist/main.spellwire.bin and its state manifest\n` +
      `bun run build\n` +
      `\`\`\`\n\n` +
      `Edit \`src/main.ts\` for both realtime logic and the state-driven overlay. ` +
      `The compiler extracts only realtime handlers; ` +
      `application code remains on Bun. The first live run requests required global ` +
      `input permissions. String hotkeys compile to native chords; the \`when\` gate makes ` +
      `inactive input pass through. Press \`Ctrl+C\` to stop and release held synthetic input.\n`,
  );
  await Bun.write(
    `${target}/README.ko.md`,
    `# ${basename(target)}\n\n` +
      `[English](README.md)\n\n` +
      `Spellwire는 live input 시작 전에 realtime handler를 native VM으로 컴파일합니다.\n\n` +
      `\`\`\`bash\n` +
      `# 한 번 실행\n` +
      `bun run start\n\n` +
      `# source 변경을 hot reload하며 실행\n` +
      `bun run watch\n\n` +
      `# dist/main.spellwire.bin과 상태 manifest 생성\n` +
      `bun run build\n` +
      `\`\`\`\n\n` +
      `realtime 로직과 상태 기반 overlay는 모두 \`src/main.ts\`에서 편집하십시오. compiler는 ` +
      `realtime handler만 추출하고 application code는 Bun에 유지합니다. 첫 live run은 필요한 전역 입력 권한을 ` +
      `요청합니다. 문자열 hotkey는 native chord로 compile되며 \`when\` gate가 꺼진 입력은 ` +
      `원래 앱으로 통과합니다. \`Ctrl+C\`로 종료하면 눌린 상태의 합성 입력도 해제합니다.\n`,
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
