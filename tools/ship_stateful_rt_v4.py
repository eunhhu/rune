from __future__ import annotations

import runpy
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

if not (ROOT / "crates/rune-core/src/rt_vm.rs").exists():
    generator = next(
        (path for path in [
            ROOT / "tools/ship_stateful_rt_v3.py",
            ROOT / "tools/ship_stateful_rt_v2.py",
        ] if path.exists()),
        None,
    )
    if generator is None:
        raise RuntimeError("no stateful runtime generator is available")
    runpy.run_path(str(generator), run_name="__main__")

compiler = ROOT / "packages/sdk/src/rt.ts"
text = compiler.read_text(encoding="utf-8")

# SourceFile.parseDiagnostics is intentionally internal in some TypeScript releases.
start = text.find("  const diagnostics =", text.find("function parseDefinition"))
end = text.find("  const statement =", start)
if start >= 0 and end > start:
    text = text[:start] + text[end:]

text = text.replace(
'''    const trigger = {
      "on.keyDown": { device: 0 as const, edge: 0 as const },
      "on.keyUp": { device: 0 as const, edge: 1 as const },
      "on.mouseDown": { device: 1 as const, edge: 0 as const },
      "on.mouseUp": { device: 1 as const, edge: 1 as const },
    }[name];''',
'''    const triggers: Readonly<Record<string, { device: 0 | 1; edge: 0 | 1 }>> = {
      "on.keyDown": { device: 0, edge: 0 },
      "on.keyUp": { device: 0, edge: 1 },
      "on.mouseDown": { device: 1, edge: 0 },
      "on.mouseUp": { device: 1, edge: 1 },
    };
    const trigger = triggers[name];''',
)

text = text.replace(
'''        const op = {
          code: "load_event_code" as const,
          edge: "load_event_edge" as const,
          source: "load_event_source" as const,
        }[expression.name.text];''',
'''        const eventFields: Readonly<Record<string, "load_event_code" | "load_event_edge" | "load_event_source">> = {
          code: "load_event_code",
          edge: "load_event_edge",
          source: "load_event_source",
        };
        const op = eventFields[expression.name.text];''',
)

text = text.replace(
'''      const op = {
        [ts.SyntaxKind.MinusToken]: "neg" as const,
        [ts.SyntaxKind.ExclamationToken]: "logical_not" as const,
        [ts.SyntaxKind.TildeToken]: "bit_not" as const,
      }[expression.operator];
      if (expression.operator === ts.SyntaxKind.PlusToken) return true;
      if (!op) throw new RtCompileError("unsupported realtime unary operator", expression);
      this.emit({ op });
      return true;''',
'''      switch (expression.operator) {
        case ts.SyntaxKind.PlusToken:
          return true;
        case ts.SyntaxKind.MinusToken:
          this.emit({ op: "neg" });
          return true;
        case ts.SyntaxKind.ExclamationToken:
          this.emit({ op: "logical_not" });
          return true;
        case ts.SyntaxKind.TildeToken:
          this.emit({ op: "bit_not" });
          return true;
        default:
          throw new RtCompileError("unsupported realtime unary operator", expression);
      }''',
)

text = text.replace(
    "      return table[expression.name.text as keyof typeof table];",
    "      const value = (table as unknown as Readonly<Record<string, number>>)[expression.name.text];\n"
    "      if (value === undefined) {\n"
    "        throw new RtCompileError(`${propertyChain(expression)} is not a Rune constant`, expression);\n"
    "      }\n"
    "      return value;",
)
compiler.write_text(text, encoding="utf-8")

(ROOT / "packages/sdk/src/rt-native.ts").write_text(
'''import { dlopen, FFIType, ptr, suffix } from "bun:ffi";
import { resolve } from "node:path";

import { compileRt, encodeRtModule, type RtModuleSpec } from "./rt";

const SYMBOLS = {
  rune_rt_load: { args: [FFIType.ptr, FFIType.u64], returns: FFIType.i32 },
  rune_rt_clear: { args: [], returns: FFIType.void },
  rune_rt_is_loaded: { args: [], returns: FFIType.u8 },
  rune_rt_state_get: { args: [FFIType.u64, FFIType.ptr], returns: FFIType.i32 },
  rune_rt_state_set: { args: [FFIType.u64, FFIType.i64], returns: FFIType.i32 },
  rune_rt_dispatch_failures: { args: [], returns: FFIType.u64 },
} as const;

interface NativeSymbols {
  rune_rt_load(data: unknown, length: bigint): number;
  rune_rt_clear(): void;
  rune_rt_is_loaded(): number;
  rune_rt_state_get(slot: bigint, output: unknown): number;
  rune_rt_state_set(slot: bigint, value: bigint): number;
  rune_rt_dispatch_failures(): bigint;
}

interface NativeLibrary {
  symbols: NativeSymbols;
  close(): void;
}

export interface RtLoadOptions {
  library?: string;
}

export class RtRuntime {
  private native?: NativeLibrary;
  private module?: RtModuleSpec;

  compile(definition: () => void): RtModuleSpec {
    return compileRt(definition);
  }

  load(definition: (() => void) | RtModuleSpec, options: RtLoadOptions = {}): RtModuleSpec {
    const module = typeof definition === "function" ? compileRt(definition) : definition;
    const native = this.ensureNative(options.library);
    const bytes = encodeRtModule(module);
    const result = native.symbols.rune_rt_load(ptr(bytes), BigInt(bytes.byteLength));
    if (result !== 0) {
      throw new Error(`Rune failed to load realtime module (native error ${result})`);
    }
    this.module = module;
    return module;
  }

  clear(): void {
    this.ensureNative().symbols.rune_rt_clear();
    this.module = undefined;
  }

  get loaded(): boolean {
    return this.ensureNative().symbols.rune_rt_is_loaded() !== 0;
  }

  state(nameOrSlot: string | number): number {
    const slot = this.resolveState(nameOrSlot);
    const output = new BigInt64Array(1);
    const result = this.ensureNative().symbols.rune_rt_state_get(BigInt(slot), ptr(output));
    if (result !== 0) throw new Error(`Rune state slot ${slot} is unavailable`);
    const value = Number(output[0]);
    if (!Number.isSafeInteger(value)) {
      throw new Error(`Rune state slot ${slot} exceeds JavaScript's safe integer range`);
    }
    return value;
  }

  setState(nameOrSlot: string | number, value: number): void {
    if (!Number.isSafeInteger(value)) throw new TypeError("Rune state values must be safe integers");
    const slot = this.resolveState(nameOrSlot);
    const result = this.ensureNative().symbols.rune_rt_state_set(BigInt(slot), BigInt(value));
    if (result !== 0) throw new Error(`Rune state slot ${slot} is unavailable`);
  }

  get dispatchFailures(): bigint {
    return this.ensureNative().symbols.rune_rt_dispatch_failures();
  }

  private resolveState(nameOrSlot: string | number): number {
    if (typeof nameOrSlot === "number") {
      if (!Number.isInteger(nameOrSlot) || nameOrSlot < 0) throw new TypeError("invalid state slot");
      return nameOrSlot;
    }
    if (!this.module) throw new Error("no realtime module is loaded by this controller");
    const slot = this.module.stateNames.indexOf(nameOrSlot);
    if (slot < 0) throw new Error(`unknown Rune state ${nameOrSlot}`);
    return slot;
  }

  private ensureNative(path?: string): NativeLibrary {
    if (!this.native) this.native = openNative(path ?? defaultLibraryPath());
    return this.native;
  }
}

function openNative(path: string): NativeLibrary {
  return dlopen(path, SYMBOLS) as unknown as NativeLibrary;
}

function defaultLibraryPath(): string {
  const configured = process.env.RUNE_NATIVE_LIBRARY;
  if (configured) return configured;
  const stem = process.platform === "win32" ? "rune_native" : "librune_native";
  return resolve(process.cwd(), "target", "release", `${stem}.${suffix}`);
}

export const rt = new RtRuntime();
''',
encoding="utf-8",
)

# Prefer internal module paths in the core implementation so it does not depend on re-export order.
vm = ROOT / "crates/rune-core/src/rt_vm.rs"
text = vm.read_text(encoding="utf-8")
text = text.replace(
'''use crate::{
    Edge, Injector, InputDevice, InputEvent, InputSource, MouseButton, OutputEvent, SourceFilter,
    Trigger,
};''',
'''use crate::{
    executor::Injector,
    model::{
        Edge, InputDevice, InputEvent, InputSource, MouseButton, OutputEvent, SourceFilter, Trigger,
    },
};''',
)
text = text.replace("RtInstruction::JumpIfFalse(17),", "RtInstruction::JumpIfFalse(18),")
vm.write_text(text, encoding="utf-8")

wire = ROOT / "crates/rune-core/src/rt_wire.rs"
text = wire.read_text(encoding="utf-8")
text = text.replace(
'''use crate::{
    Edge, InputDevice, MouseButton, RtFunction, RtHandler, RtInstruction, RtModule, SourceFilter,
    Trigger, RT_MAX_LOCALS,
};''',
'''use crate::{
    model::{Edge, InputDevice, MouseButton, SourceFilter, Trigger},
    rt_vm::{RtFunction, RtHandler, RtInstruction, RtModule, RT_MAX_LOCALS},
};''',
)
wire.write_text(text, encoding="utf-8")

global_rt = ROOT / "crates/rune-core/src/global_rt.rs"
text = global_rt.read_text(encoding="utf-8")
text = text.replace(
'''use crate::{
    Injector, InputEvent, RtBuildError, RtEngine, RtExecutionConfig, RtModule, RtScratch,
};''',
'''use crate::{
    executor::Injector,
    model::InputEvent,
    rt_vm::{RtBuildError, RtEngine, RtExecutionConfig, RtModule, RtScratch},
};''',
)
global_rt.write_text(text, encoding="utf-8")

for disposable in [
    ROOT / "tools/ship_stateful_rt_v2.py",
    ROOT / "tools/ship_stateful_rt_v3.py",
    ROOT / ".github/workflows/ship-stateful-runtime-v2.yml",
    ROOT / ".github/workflows/ship-stateful-runtime-v3.yml",
]:
    disposable.unlink(missing_ok=True)

shutil.rmtree(ROOT / ".bootstrap", ignore_errors=True)
(ROOT / ".github/workflows/bootstrap.yml").unlink(missing_ok=True)
