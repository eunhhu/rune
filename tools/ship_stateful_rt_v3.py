from __future__ import annotations

import re
import runpy
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

v2 = ROOT / "tools/ship_stateful_rt_v2.py"
if not (ROOT / "crates/rune-core/src/rt_vm.rs").exists():
    if not v2.exists():
        raise RuntimeError("stateful runtime source and v2 generator are both missing")
    runpy.run_path(str(v2), run_name="__main__")


def replace(path: str, old: str, new: str) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    if old in text:
        target.write_text(text.replace(old, new), encoding="utf-8")


replace(
    "crates/rune-core/src/rt_vm.rs",
    "use crate::{\n    Edge, Injector, InputDevice, InputEvent, InputSource, MouseButton, OutputEvent, SourceFilter,\n    Trigger,\n};",
    "use crate::{\n    executor::Injector,\n    model::{\n        Edge, InputDevice, InputEvent, InputSource, MouseButton, OutputEvent, SourceFilter, Trigger,\n    },\n};",
)
replace(
    "crates/rune-core/src/rt_vm.rs",
    "RtInstruction::JumpIfFalse(17),",
    "RtInstruction::JumpIfFalse(18),",
)
replace(
    "crates/rune-core/src/rt_wire.rs",
    "use crate::{\n    Edge, InputDevice, MouseButton, RtFunction, RtHandler, RtInstruction, RtModule, SourceFilter,\n    Trigger, RT_MAX_LOCALS,\n};",
    "use crate::{\n    model::{Edge, InputDevice, MouseButton, SourceFilter, Trigger},\n    rt_vm::{RtFunction, RtHandler, RtInstruction, RtModule, RT_MAX_LOCALS},\n};",
)
replace(
    "crates/rune-core/src/global_rt.rs",
    "use crate::{\n    Injector, InputEvent, RtBuildError, RtEngine, RtExecutionConfig, RtModule, RtScratch,\n};",
    "use crate::{\n    executor::Injector,\n    model::InputEvent,\n    rt_vm::{RtBuildError, RtEngine, RtExecutionConfig, RtModule, RtScratch},\n};",
)

compiler = ROOT / "packages/sdk/src/rt.ts"
text = compiler.read_text(encoding="utf-8")
text = text.replace(
    "const diagnostics = file.parseDiagnostics;",
    "const diagnostics = (file as ts.SourceFile & { parseDiagnostics?: readonly ts.Diagnostic[] }).parseDiagnostics ?? [];",
)
text = re.sub(
    r"  if \(diagnostics\.length > 0\) \{\n"
    r"    throw new RtCompileError\(\n"
    r"      `unable to parse realtime definition: \$\{ts\.flattenDiagnosticMessageText\(diagnostics\[0\]\.messageText, \"\\\\n\"\)\}`,\n"
    r"    \);\n"
    r"  \}",
    "  const diagnostic = diagnostics[0];\n"
    "  if (diagnostic) {\n"
    "    throw new RtCompileError(\n"
    "      `unable to parse realtime definition: ${ts.flattenDiagnosticMessageText(diagnostic.messageText, \\\"\\\\n\\\")}`,\n"
    "    );\n"
    "  }",
    text,
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

bridge = ROOT / "packages/sdk/src/rt-native.ts"
text = bridge.read_text(encoding="utf-8")
text = text.replace(
    "native.symbols.rune_rt_load(ptr(bytes), bytes.byteLength)",
    "native.symbols.rune_rt_load(ptr(bytes), BigInt(bytes.byteLength))",
)
text = text.replace(
    "symbols.rune_rt_state_get(slot, ptr(output))",
    "symbols.rune_rt_state_get(BigInt(slot), ptr(output))",
)
text = text.replace(
    "symbols.rune_rt_state_set(slot, BigInt(value))",
    "symbols.rune_rt_state_set(BigInt(slot), BigInt(value))",
)
text = text.replace(
    "return this.ensureNative().symbols.rune_rt_dispatch_failures();",
    "return BigInt(this.ensureNative().symbols.rune_rt_dispatch_failures());",
)
bridge.write_text(text, encoding="utf-8")

for disposable in [
    ROOT / "tools/ship_stateful_rt_v2.py",
    ROOT / ".github/workflows/ship-stateful-runtime-v2.yml",
]:
    disposable.unlink(missing_ok=True)

shutil.rmtree(ROOT / ".bootstrap", ignore_errors=True)
(ROOT / ".github/workflows/bootstrap.yml").unlink(missing_ok=True)
