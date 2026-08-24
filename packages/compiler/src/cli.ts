#!/usr/bin/env bun

import { basename, dirname, extname, join, resolve } from "node:path";
import { compileSource } from "./compiler";
import { encodeModule } from "./encode";

function outputPath(input: string): string {
  const absolute = resolve(input);
  const extension = extname(absolute);
  const stem = basename(absolute, extension).replace(/\.rune$/, "");
  return join(dirname(absolute), `${stem}.rune.bin`);
}

const input = Bun.argv[2];
if (!input) {
  console.error("usage: rune-compile <macro.rune.ts> [output.rune.bin]");
  process.exit(2);
}

const source = await Bun.file(input).text();
const result = compileSource(source, { fileName: input });
const binary = encodeModule(result.module);
const output = resolve(Bun.argv[3] ?? outputPath(input));
await Bun.write(output, binary);

const stateManifest = Object.fromEntries(
  result.module.states.map((state) => [state.name, { slot: state.slot, kind: state.kind }]),
);
await Bun.write(
  `${output}.json`,
  JSON.stringify(
    {
      version: 1,
      input: resolve(input),
      binary: output,
      handlers: result.module.handlers.length,
      instructions: result.module.code.length,
      states: stateManifest,
    },
    null,
    2,
  ),
);

console.log(
  `compiled ${input}: ${result.module.handlers.length} handlers, ${result.module.states.length} persistent states, ${result.module.code.length} instructions`,
);
console.log(output);
