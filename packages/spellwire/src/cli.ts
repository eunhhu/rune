#!/usr/bin/env bun
import { basename, dirname, extname, join, resolve } from "node:path";
import { compileSource } from "./compiler/compiler";
import { encodeModule } from "./compiler/encode";
const args=Bun.argv.slice(2); if(args[0]==="compile") args.shift(); const input=args[0];
if(!input){console.error("usage: spellwire compile <macro.spellwire.ts> [output.spellwire.bin]");process.exit(2);}
const absolute=resolve(input), extension=extname(absolute), stem=basename(absolute,extension).replace(/\.spellwire$/,
"");
const output=resolve(args[1]??join(dirname(absolute),`${stem}.spellwire.bin`));
const result=compileSource(await Bun.file(input).text(),{fileName:input}); await Bun.write(output,encodeModule(result.module));
const states=Object.fromEntries(result.module.states.map(s=>[s.name,{slot:s.slot,kind:s.kind}]));
await Bun.write(`${output}.json`,JSON.stringify({version:1,input:absolute,binary:output,handlers:result.module.handlers.length,instructions:result.module.code.length,states},null,2));
console.log(`compiled ${input}: ${result.module.handlers.length} handlers, ${result.module.states.length} persistent states, ${result.module.code.length} instructions`); console.log(output);
