#!/usr/bin/env bun

import { basename, dirname, extname, join, resolve } from "node:path";
import { compileSource } from "./compiler/compiler";
import { encodeModule } from "./compiler/encode";

const VERSION = "0.1.0";
const HELP = `Spellwire ${VERSION}

Usage:
  spellwire compile <input.spellwire.ts> [output.spellwire.bin]
  spellwire <input.spellwire.ts> [output.spellwire.bin]
  spellwire --help
  spellwire --version
`;

async function main(args: string[]): Promise<void> {
  const command = args[0];

  if (command === "--help" || command === "-h" || command === "help") {
    console.log(HELP);
    return;
  }

  if (command === "--version" || command === "-v") {
    console.log(VERSION);
    return;
  }

  if (command === "compile") args.shift();

  const input = args[0];
  if (!input) {
    console.error(HELP);
    process.exitCode = 2;
    return;
  }

  const absolute = resolve(input);
  if (!(await Bun.file(absolute).exists())) {
    throw new Error(`input file does not exist: ${absolute}`);
  }

  const extension = extname(absolute);
  const stem = basename(absolute, extension).replace(/\.spellwire$/, "");
  const output = resolve(args[1] ?? join(dirname(absolute), `${stem}.spellwire.bin`));
  const result = compileSource(await Bun.file(absolute).text(), { fileName: absolute });

  await Bun.write(output, encodeModule(result.module));

  const states = Object.fromEntries(
    result.module.states.map((state) => [
      state.name,
      { slot: state.slot, kind: state.kind },
    ]),
  );

  await Bun.write(
    `${output}.json`,
    `${JSON.stringify(
      {
        version: 1,
        input: absolute,
        binary: output,
        handlers: result.module.handlers.length,
        instructions: result.module.code.length,
        states,
      },
      null,
      2,
    )}\n`,
  );

  console.log(
    `compiled ${input}: ${result.module.handlers.length} handlers, ` +
      `${result.module.states.length} persistent states, ` +
      `${result.module.code.length} instructions`,
  );
  console.log(output);
}

try {
  await main(Bun.argv.slice(2));
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exitCode = 1;
}
