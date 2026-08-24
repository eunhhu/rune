from __future__ import annotations
import base64, hashlib, io, json, os, shutil, tarfile
from pathlib import Path

EXPECTED_SHA256='6c566c86ba2c32fef2987f2e52ed5939cb6fa0a54d13565eaedc2b88615735b3'
root=Path(__file__).resolve().parents[1]; bootstrap=root/'.bootstrap'
encoded=''.join(p.read_text().strip() for p in sorted(bootstrap.glob('v2-*.txt')))
archive=base64.b64decode(encoded,validate=True)
if hashlib.sha256(archive).hexdigest()!=EXPECTED_SHA256: raise RuntimeError('source payload checksum mismatch')
workflow=root/'.github/workflows/bootstrap.yml'; workflow_bytes=workflow.read_bytes()
for child in root.iterdir():
    if child.name in {'.git','.bootstrap'}: continue
    shutil.rmtree(child) if child.is_dir() else child.unlink()
with tarfile.open(fileobj=io.BytesIO(archive),mode='r:gz') as bundle:
    resolved=root.resolve()
    for member in bundle.getmembers():
        target=(root/member.name).resolve()
        if member.issym() or member.islnk() or (target!=resolved and resolved not in target.parents): raise RuntimeError('unsafe archive')
    bundle.extractall(root)
workflow.parent.mkdir(parents=True,exist_ok=True); workflow.write_bytes(workflow_bytes)

# Current Rust compatibility fixes.
vm=root/'crates/rune-core/src/vm.rs'; text=vm.read_text()
text=text.replace('Edge, HandlerTable, InputDevice, InputEvent, Instruction, MouseButton, Opcode, OutputEvent,','Edge, HandlerTable, InputDevice, InputEvent, MouseButton, Opcode, OutputEvent,')
text=text.replace('binary!(|left, right| left.wrapping_shl((right as u32) & 63))','binary!(|left: i64, right: i64| left.wrapping_shl((right as u32) & 63))')
text=text.replace('binary!(|left, right| left.wrapping_shr((right as u32) & 63))','binary!(|left: i64, right: i64| left.wrapping_shr((right as u32) & 63))')
text=text.replace('Opcode::Neg => push!(pop!().wrapping_neg()),','Opcode::Neg => { let value = pop!(); push!(value.wrapping_neg()); },')
text=text.replace('Opcode::Not => push!(i64::from(pop!() == 0)),','Opcode::Not => { let value = pop!(); push!(i64::from(value == 0)); },')
vm.write_text(text)

# Filesystem rebrand.
for old,new in [('crates/rune-core','crates/spellwire-core'),('crates/rune-native','crates/spellwire-native'),('crates/rune-bench','crates/spellwire-bench'),('packages/sdk','packages/spellwire'),('examples/stateful.rune.ts','examples/stateful.spellwire.ts'),('packaging/linux/99-rune-input.rules','packaging/linux/99-spellwire-input.rules')]:
    source=root/old
    if source.exists(): (root/new).parent.mkdir(parents=True,exist_ok=True); source.rename(root/new)

# Merge compiler into one public npm package.
compiler=root/'packages/compiler'; package=root/'packages/spellwire'; dst=package/'src/compiler'; dst.mkdir(parents=True,exist_ok=True)
for name in ('compiler.ts','encode.ts','ir.ts'): shutil.move(compiler/'src'/name,dst/name)
shutil.move(compiler/'src/cli.ts',package/'src/cli.ts')
for name in ('compiler.test.ts','encode.test.ts'): shutil.move(compiler/'test'/name,package/'test'/name)
shutil.rmtree(compiler)

# Global source-level rebrand.
text_ext={'.rs','.toml','.ts','.tsx','.js','.json','.md','.yml','.yaml','.txt','.lock'}
for path in root.rglob('*'):
    if not path.is_file() or (path.suffix not in text_ext and path.name not in {'.gitignore','Cargo.lock'}): continue
    value=path.read_text(errors='strict')
    for old,new in [('https://github.com/eunhhu/rune','https://github.com/eunhhu/spellwire'),('@rune/sdk','spellwire'),('@rune/compiler','spellwire'),('RUNE_NATIVE_PATH','SPELLWIRE_NATIVE_PATH'),('RuneCompileError','SpellwireCompileError'),('RuneOutputCallback','SpellwireOutputCallback'),('RuneOutputEvent','SpellwireOutputEvent'),('RuneEngine','SpellwireEngine'),('RuneRuntime','SpellwireRuntime'),('rune_engine_','spellwire_engine_'),('rune_abi_version','spellwire_abi_version'),('rune_capabilities','spellwire_capabilities'),('rune_core','spellwire_core'),('rune-native','spellwire-native'),('rune-core','spellwire-core'),('rune-bench','spellwire-bench'),('stateful.rune.ts','stateful.spellwire.ts'),('.rune.bin','.spellwire.bin'),('.rune.ts','.spellwire.ts'),('rune-compile','spellwire-compile'),('Rune','Spellwire'),('rune','spellwire')]: value=value.replace(old,new)
    path.write_text(value)

# Four-byte wire identity.
bytecode=root/'crates/spellwire-core/src/bytecode.rs'; value=bytecode.read_text().replace('*b"SPELLWIRE"','*b"SPWR"'); bytecode.write_text(value)
encode=package/'src/compiler/encode.ts'; value=encode.read_text().replace('[0x52, 0x55, 0x4e, 0x45]','[0x53, 0x50, 0x57, 0x52]').replace('// SPELLWIRE','// SPWR'); encode.write_text(value)
compiler_ts=package/'src/compiler/compiler.ts'; compiler_ts.write_text(compiler_ts.read_text().replace('from "spellwire";','from "../keys";'))

(dst/'index.ts').write_text('''export { compileSource, SpellwireCompileError, type CompileDiagnostic, type CompileOptions, type CompileResult } from "./compiler";\nexport { encodeModule } from "./encode";\nexport { InputDevice, InputEdge, Opcode, SourceFilter, WIRE_HANDLER_SIZE, WIRE_HEADER_SIZE, WIRE_INSTRUCTION_SIZE, WIRE_VERSION, type CompiledModule, type Handler, type Instruction, type StateSlot } from "./ir";\n''')
index=package/'src/index.ts'; index.write_text(index.read_text()+'\nexport * from "./compiler";\n')

# CLI accepts `spellwire compile file` and direct-file shorthand.
cli=package/'src/cli.ts'; cli.write_text('''#!/usr/bin/env bun\nimport { basename, dirname, extname, join, resolve } from "node:path";\nimport { compileSource } from "./compiler/compiler";\nimport { encodeModule } from "./compiler/encode";\nconst args=Bun.argv.slice(2); if(args[0]==="compile") args.shift(); const input=args[0];\nif(!input){console.error("usage: spellwire compile <macro.spellwire.ts> [output.spellwire.bin]");process.exit(2);}\nconst absolute=resolve(input), extension=extname(absolute), stem=basename(absolute,extension).replace(/\\.spellwire$/,
"");\nconst output=resolve(args[1]??join(dirname(absolute),`${stem}.spellwire.bin`));\nconst result=compileSource(await Bun.file(input).text(),{fileName:input}); await Bun.write(output,encodeModule(result.module));\nconst states=Object.fromEntries(result.module.states.map(s=>[s.name,{slot:s.slot,kind:s.kind}]));\nawait Bun.write(`${output}.json`,JSON.stringify({version:1,input:absolute,binary:output,handlers:result.module.handlers.length,instructions:result.module.code.length,states},null,2));\nconsole.log(`compiled ${input}: ${result.module.handlers.length} handlers, ${result.module.states.length} persistent states, ${result.module.code.length} instructions`); console.log(output);\n'''); os.chmod(cli,0o755)

package_json={'name':'spellwire','version':'0.1.0','description':'Stateful realtime input automation for TypeScript, compiled to a native Rust VM.','keywords':['bun','typescript','macro','automation','input','realtime'],'homepage':'https://github.com/eunhhu/spellwire#readme','bugs':{'url':'https://github.com/eunhhu/spellwire/issues'},'repository':{'type':'git','url':'git+https://github.com/eunhhu/spellwire.git','directory':'packages/spellwire'},'license':'MIT','author':'Sunwoo Moon','type':'module','exports':{'.':{'types':'./src/index.ts','import':'./src/index.ts'},'./compiler':{'types':'./src/compiler/index.ts','import':'./src/compiler/index.ts'}},'types':'./src/index.ts','bin':{'spellwire':'./src/cli.ts','spellwire-compile':'./src/cli.ts'},'files':['src','README.md','LICENSE'],'sideEffects':False,'engines':{'bun':'>=1.3.14'},'dependencies':{'typescript':'^5.8.3'},'publishConfig':{'access':'public'}}
(package/'package.json').write_text(json.dumps(package_json,indent=2)+'\n')
config=json.loads((package/'tsconfig.json').read_text()); config.pop('references',None); (package/'tsconfig.json').write_text(json.dumps(config,indent=2)+'\n')
for test in (package/'test').glob('*.ts'):
    value=test.read_text().replace('from "spellwire";','from "../src/index";').replace('from "../src";','from "../src/index";').replace('toBe("SPELLWIRE")','toBe("SPWR")')
    test.write_text(value)

# `bun create spellwire` package.
create=root/'packages/create-spellwire'; (create/'src').mkdir(parents=True); (create/'test').mkdir()
(create/'package.json').write_text(json.dumps({'name':'create-spellwire','version':'0.1.0','description':'Create a Spellwire TypeScript macro project.','homepage':'https://github.com/eunhhu/spellwire#readme','repository':{'type':'git','url':'git+https://github.com/eunhhu/spellwire.git','directory':'packages/create-spellwire'},'license':'MIT','author':'Sunwoo Moon','type':'module','bin':{'create-spellwire':'./src/index.ts'},'files':['src','README.md','LICENSE'],'engines':{'bun':'>=1.3.14'},'publishConfig':{'access':'public'}},indent=2)+'\n')
(create/'src/index.ts').write_text('''#!/usr/bin/env bun\nimport { existsSync } from "node:fs"; import { basename, resolve } from "node:path";\nexport async function createSpellwireProject(destination="spellwire-macro",install=true){const target=resolve(destination);if(existsSync(target))throw new Error(`destination already exists: ${target}`);\nawait Bun.write(`${target}/package.json`,JSON.stringify({name:basename(target),private:true,type:"module",scripts:{build:"spellwire compile src/main.spellwire.ts",check:"tsc --noEmit"},dependencies:{spellwire:"latest"},devDependencies:{"@types/bun":"latest",typescript:"^5.8.3"}},null,2)+"\\n");\nawait Bun.write(`${target}/tsconfig.json`,JSON.stringify({compilerOptions:{target:"ES2022",module:"ESNext",moduleResolution:"Bundler",strict:true,noEmit:true,types:["bun"]},include:["src/**/*.ts"]},null,2)+"\\n");\nawait Bun.write(`${target}/src/main.spellwire.ts`,`import { Key, keyDown, keyUp, rt } from "spellwire";\\nlet count=0;\\nrt.onKeyDown(Key.Q,()=>{count++;keyDown(Key.E);keyUp(Key.E);});\\n`); await Bun.write(`${target}/.gitignore`,"node_modules/\\n*.spellwire.bin*\\n");\nif(install){const process=Bun.spawn(["bun","install"],{cwd:target,stdout:"inherit",stderr:"inherit"});if(await process.exited!==0)throw new Error("bun install failed");}return target;}\nif(import.meta.main){const args=Bun.argv.slice(2), noInstall=args.includes("--no-install"), destination=args.find(a=>!a.startsWith("-"))??"spellwire-macro";try{const target=await createSpellwireProject(destination,!noInstall);console.log(`Created Spellwire project at ${target}`);}catch(error){console.error(error instanceof Error?error.message:error);process.exit(1);}}\n'''); os.chmod(create/'src/index.ts',0o755)
(create/'test/create.test.ts').write_text('''import { expect,test } from "bun:test"; import { rm } from "node:fs/promises"; import { join } from "node:path"; import { tmpdir } from "node:os"; import { createSpellwireProject } from "../src/index";\ntest("creates a Spellwire project",async()=>{const target=join(tmpdir(),`spellwire-${crypto.randomUUID()}`);try{await createSpellwireProject(target,false);expect((await Bun.file(join(target,"package.json")).json()).dependencies.spellwire).toBe("latest");expect(await Bun.file(join(target,"src/main.spellwire.ts")).exists()).toBe(true);}finally{await rm(target,{recursive:true,force:true});}});\n''')
(create/'tsconfig.json').write_text(json.dumps({'compilerOptions':{'target':'ES2022','module':'ESNext','moduleResolution':'Bundler','strict':True,'noUncheckedIndexedAccess':True,'exactOptionalPropertyTypes':True,'verbatimModuleSyntax':True,'declaration':True,'composite':True,'skipLibCheck':True,'types':['bun'],'rootDir':'.','outDir':'dist','tsBuildInfoFile':'dist/tsconfig.tsbuildinfo'},'include':['src/**/*.ts','test/**/*.ts']},indent=2)+'\n')
license=(root/'LICENSE').read_text(); (package/'LICENSE').write_text(license); (create/'LICENSE').write_text(license)
(package/'README.md').write_text('# spellwire\n\n```bash\nbun add spellwire\n```\n'); (create/'README.md').write_text('# create-spellwire\n\n```bash\nbun create spellwire my-macro\n```\n')

(root/'package.json').write_text(json.dumps({'name':'spellwire-workspace','private':True,'packageManager':'bun@1.4.0','workspaces':['packages/*'],'scripts':{'build:native':'cargo build -p spellwire-native --release','typecheck':'bunx tsc -b packages/spellwire packages/create-spellwire --pretty false','test:rust':'cargo test --workspace','test:ts':'bun test packages/spellwire/test packages/create-spellwire/test','check':'bun run typecheck && bun run test:ts && cargo test --workspace','compile:example':'bun packages/spellwire/src/cli.ts compile examples/stateful.spellwire.ts','bench':'cargo run -p spellwire-bench --release --','pack:dry-run':'npm pack --dry-run --workspace spellwire && npm pack --dry-run --workspace create-spellwire'},'devDependencies':{'@types/bun':'^1.4.0','typescript':'^5.8.3'}},indent=2)+'\n')
(root/'tsconfig.json').write_text(json.dumps({'files':[],'references':[{'path':'./packages/spellwire'},{'path':'./packages/create-spellwire'}]},indent=2)+'\n')

# Remove stale generated files; locks are regenerated by CI.
for path in [root/'bun.lock',root/'Cargo.lock']:
    path.unlink(missing_ok=True)
shutil.rmtree(bootstrap)
