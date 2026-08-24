from __future__ import annotations

import runpy
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

if not (ROOT / "crates/rune-core/src/rt_vm.rs").exists():
    generators = [
        ROOT / "tools/ship_stateful_rt_v8.py",
        ROOT / "tools/ship_stateful_rt_v7.py",
        ROOT / "tools/ship_stateful_rt_v6.py",
        ROOT / "tools/ship_stateful_rt_v5.py",
        ROOT / "tools/ship_stateful_rt_v4.py",
        ROOT / "tools/ship_stateful_rt_v3.py",
        ROOT / "tools/ship_stateful_rt_v2.py",
    ]
    generator = next((path for path in generators if path.exists()), None)
    if generator is None:
        raise RuntimeError("stateful runtime sources and generators are both missing")
    runpy.run_path(str(generator), run_name="__main__")

rt_path = ROOT / "packages/sdk/src/rt.ts"
text = rt_path.read_text(encoding="utf-8")
text = text.replace("export const Key = {", "const KEY_CODES = {")
text = text.replace("export type Key = (typeof Key)[keyof typeof Key];\n\n", "")
text = text.replace("export const MouseButton = {", "const MOUSE_BUTTON_CODES = {")
text = text.replace(
    "export type MouseButton = (typeof MouseButton)[keyof typeof MouseButton];\n\n",
    "",
)
text = text.replace("_key: Key", "_key: number")
text = text.replace("_button: MouseButton", "_button: number")
text = text.replace("_input: Key | MouseButton", "_input: number")
text = text.replace(
    'const table = owner === "Key" ? Key : owner === "MouseButton" ? MouseButton : undefined;',
    'const table = owner === "Key" ? KEY_CODES : owner === "MouseButton" ? MOUSE_BUTTON_CODES : undefined;',
)
rt_path.write_text(text, encoding="utf-8")

# The compiler test should consume the SDK's already-established public Key export.
test_path = ROOT / "packages/sdk/test/rt.test.ts"
test = test_path.read_text(encoding="utf-8")
test = test.replace("  Key,\n", "")
if 'from "../src/index"' not in test:
    test = 'import { Key } from "../src/index";\n' + test
# The index re-exports the compiler; importing its Key no longer creates an ambiguous export.
test_path.write_text(test, encoding="utf-8")

# Clean every older one-shot job. The v9 workflow removes itself only after validation.
for version in range(2, 9):
    (ROOT / f"tools/ship_stateful_rt_v{version}.py").unlink(missing_ok=True)
    (ROOT / f".github/workflows/ship-stateful-runtime-v{version}.yml").unlink(missing_ok=True)
shutil.rmtree(ROOT / ".bootstrap", ignore_errors=True)
(ROOT / ".github/workflows/bootstrap.yml").unlink(missing_ok=True)
