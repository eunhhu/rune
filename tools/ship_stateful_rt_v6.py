from __future__ import annotations

import runpy
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

if not (ROOT / "crates/rune-core/src/rt_vm.rs").exists():
    generators = [
        ROOT / "tools/ship_stateful_rt_v5.py",
        ROOT / "tools/ship_stateful_rt_v4.py",
        ROOT / "tools/ship_stateful_rt_v3.py",
        ROOT / "tools/ship_stateful_rt_v2.py",
    ]
    generator = next((path for path in generators if path.exists()), None)
    if generator is None:
        raise RuntimeError("no stateful runtime generator is available")
    runpy.run_path(str(generator), run_name="__main__")

vm_path = ROOT / "crates/rune-core/src/rt_vm.rs"
vm = vm_path.read_text(encoding="utf-8")
vm = vm.replace(
'''                    scratch
                        .push(scratch.locals[index])
                        .map_err(RtExecutionError::Fault)?;''',
'''                    let value = scratch.locals[index];
                    scratch.push(value).map_err(RtExecutionError::Fault)?;''',
)
vm = vm.replace(
'''                    scratch.locals[index] = scratch.peek().map_err(RtExecutionError::Fault)?;''',
'''                    let value = scratch.peek().map_err(RtExecutionError::Fault)?;
                    scratch.locals[index] = value;''',
)
vm = vm.replace(
'''                    scratch
                        .push_frame(CallFrame {
                            return_ip: ip + 1,
                            previous_base: local_base,
                            previous_limit: local_limit,
                            previous_local_len: scratch.local_len,
                            stack_base,
                        })''',
'''                    let previous_local_len = scratch.local_len;
                    scratch
                        .push_frame(CallFrame {
                            return_ip: ip + 1,
                            previous_base: local_base,
                            previous_limit: local_limit,
                            previous_local_len,
                            stack_base,
                        })''',
)
vm = vm.replace(
'''        scratch.output[scratch.output_len] = event;
    scratch.output_len += 1;''',
'''        let index = scratch.output_len;
    scratch.output[index] = event;
    scratch.output_len = index + 1;''',
)
vm = vm.replace("RtInstruction::JumpIfFalse(17),", "RtInstruction::JumpIfFalse(18),")
vm = vm.replace(
"impl<E: fmt::Debug + fmt::Display> std::error::Error for RtExecutionError<E> {}",
"impl<E: fmt::Debug + fmt::Display + 'static> std::error::Error for RtExecutionError<E> {}",
)
vm_path.write_text(vm, encoding="utf-8")

global_path = ROOT / "crates/rune-core/src/global_rt.rs"
global_text = global_path.read_text(encoding="utf-8")
global_text = global_text.replace(
    "SLOT.get_or_init(ArcSwapOption::empty)",
    "SLOT.get_or_init(|| ArcSwapOption::from(None))",
)
global_path.write_text(global_text, encoding="utf-8")

for disposable in [
    ROOT / "tools/ship_stateful_rt_v2.py",
    ROOT / "tools/ship_stateful_rt_v3.py",
    ROOT / "tools/ship_stateful_rt_v4.py",
    ROOT / "tools/ship_stateful_rt_v5.py",
    ROOT / ".github/workflows/ship-stateful-runtime-v2.yml",
    ROOT / ".github/workflows/ship-stateful-runtime-v3.yml",
    ROOT / ".github/workflows/ship-stateful-runtime-v4.yml",
    ROOT / ".github/workflows/ship-stateful-runtime-v5.yml",
]:
    disposable.unlink(missing_ok=True)

shutil.rmtree(ROOT / ".bootstrap", ignore_errors=True)
(ROOT / ".github/workflows/bootstrap.yml").unlink(missing_ok=True)
