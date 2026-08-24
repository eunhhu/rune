from __future__ import annotations

import runpy
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# Recover the original verified source if main still contains the one-shot archive.
bootstrap = ROOT / ".bootstrap/unpack.py"
if bootstrap.exists():
    runpy.run_path(str(bootstrap), run_name="__main__")

# Generate the stateful runtime only when an earlier queued run has not already done so.
if not (ROOT / "crates/rune-core/src/rt_vm.rs").exists():
    generators = [
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


def replace(path: str, old: str, new: str) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    if old in text:
        target.write_text(text.replace(old, new), encoding="utf-8")


# Keep implementation modules independent of public re-export order.
replace(
    "crates/rune-core/src/rt_vm.rs",
    "use crate::{\n    Edge, Injector, InputDevice, InputEvent, InputSource, MouseButton, OutputEvent, SourceFilter,\n    Trigger,\n};",
    "use crate::{\n    executor::Injector,\n    model::{\n        Edge, InputDevice, InputEvent, InputSource, MouseButton, OutputEvent, SourceFilter, Trigger,\n    },\n};",
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

vm_path = ROOT / "crates/rune-core/src/rt_vm.rs"
vm = vm_path.read_text(encoding="utf-8")
vm = vm.replace("RtInstruction::JumpIfFalse(17),", "RtInstruction::JumpIfFalse(18),")
vm = vm.replace("i64::from(held != 0)", "(held != 0) as i64")
vm = vm.replace("i64::from(operation(left, right))", "operation(left, right) as i64")
vm = vm.replace(
    "impl<E: fmt::Debug + fmt::Display> std::error::Error for RtExecutionError<E> {}",
    "impl<E: fmt::Debug + fmt::Display + 'static> std::error::Error for RtExecutionError<E> {}",
)
vm = vm.replace(
    "                    scratch\n                        .push(scratch.locals[index])\n                        .map_err(RtExecutionError::Fault)?;",
    "                    let value = scratch.locals[index];\n                    scratch.push(value).map_err(RtExecutionError::Fault)?;",
)
vm = vm.replace(
    "                    scratch.locals[index] = scratch.peek().map_err(RtExecutionError::Fault)?;",
    "                    let value = scratch.peek().map_err(RtExecutionError::Fault)?;\n                    scratch.locals[index] = value;",
)
vm = vm.replace(
    "                            previous_local_len: scratch.local_len,",
    "                            previous_local_len,",
)
if "let previous_local_len = scratch.local_len;" not in vm:
    vm = vm.replace(
        "                    scratch\n                        .push_frame(CallFrame {",
        "                    let previous_local_len = scratch.local_len;\n                    scratch\n                        .push_frame(CallFrame {",
    )
vm = vm.replace(
    "    scratch.output[scratch.output_len] = event;\n    scratch.output_len += 1;",
    "    let index = scratch.output_len;\n    scratch.output[index] = event;\n    scratch.output_len = index + 1;",
)

# Match the baseline OutputEvent field names without assuming a later refactor.
model = (ROOT / "crates/rune-core/src/model.rs").read_text(encoding="utf-8")
if "MouseMove { x:" in model or "MouseMove {\n        x:" in model:
    vm = vm.replace(
        "OutputEvent::MouseMove {\n                            dx: narrow_i32(dx),\n                            dy: narrow_i32(dy),\n                        }",
        "OutputEvent::MouseMove {\n                            x: narrow_i32(dx),\n                            y: narrow_i32(dy),\n                        }",
    )
vm_path.write_text(vm, encoding="utf-8")

# ArcSwap's empty value is explicit for the minimum supported Rust compiler.
global_path = ROOT / "crates/rune-core/src/global_rt.rs"
global_text = global_path.read_text(encoding="utf-8")
global_text = global_text.replace(
    "SLOT.get_or_init(ArcSwapOption::empty)",
    "SLOT.get_or_init(|| ArcSwapOption::from(None::<Arc<RtEngine>>))",
)
global_text = global_text.replace(
    "SLOT.get_or_init(|| ArcSwapOption::from(None))",
    "SLOT.get_or_init(|| ArcSwapOption::from(None::<Arc<RtEngine>>))",
)
global_path.write_text(global_text, encoding="utf-8")

# Avoid depending on TryFrom error details from the original enum definitions.
wire_path = ROOT / "crates/rune-core/src/rt_wire.rs"
wire = wire_path.read_text(encoding="utf-8")
wire = wire.replace(
    "    let device = InputDevice::try_from(trigger.device)\n        .map_err(|()| RtDecodeError::new(\"invalid trigger device\"))?;\n    let edge = Edge::try_from(trigger.edge)\n        .map_err(|()| RtDecodeError::new(\"invalid trigger edge\"))?;\n    let source = SourceFilter::try_from(trigger.source)\n        .map_err(|()| RtDecodeError::new(\"invalid trigger source\"))?;",
    "    let device = match trigger.device {\n        0 => InputDevice::Keyboard,\n        1 => InputDevice::MouseButton,\n        _ => return Err(RtDecodeError::new(\"invalid trigger device\")),\n    };\n    let edge = match trigger.edge {\n        0 => Edge::Down,\n        1 => Edge::Up,\n        _ => return Err(RtDecodeError::new(\"invalid trigger edge\")),\n    };\n    let source = match trigger.source {\n        0 => SourceFilter::Physical,\n        1 => SourceFilter::Synthetic,\n        2 => SourceFilter::Any,\n        _ => return Err(RtDecodeError::new(\"invalid trigger source\")),\n    };",
)
wire = wire.replace(
    "        WireInstruction::MouseDown { button } => RtInstruction::MouseDown(\n            MouseButton::try_from(button)\n                .map_err(|()| RtDecodeError::new(\"invalid mouse button\"))?,\n        ),\n        WireInstruction::MouseUp { button } => RtInstruction::MouseUp(\n            MouseButton::try_from(button)\n                .map_err(|()| RtDecodeError::new(\"invalid mouse button\"))?,\n        ),",
    "        WireInstruction::MouseDown { button } => {\n            RtInstruction::MouseDown(decode_mouse_button(button)?)\n        }\n        WireInstruction::MouseUp { button } => {\n            RtInstruction::MouseUp(decode_mouse_button(button)?)\n        }",
)
if "fn decode_mouse_button" not in wire:
    wire = wire.replace(
        "fn validate_instruction(\n",
        "fn decode_mouse_button(button: u8) -> Result<MouseButton, RtDecodeError> {\n    match button {\n        0 => Ok(MouseButton::Left),\n        1 => Ok(MouseButton::Right),\n        2 => Ok(MouseButton::Middle),\n        3 => Ok(MouseButton::Back),\n        4 => Ok(MouseButton::Forward),\n        _ => Err(RtDecodeError::new(\"invalid mouse button\")),\n    }\n}\n\nfn validate_instruction(\n",
        1,
    )
wire_path.write_text(wire, encoding="utf-8")

# Remove all old one-shot machinery. The v8 workflow deletes itself after verification.
for version in range(2, 8):
    (ROOT / f"tools/ship_stateful_rt_v{version}.py").unlink(missing_ok=True)
    (ROOT / f".github/workflows/ship-stateful-runtime-v{version}.yml").unlink(missing_ok=True)
shutil.rmtree(ROOT / ".bootstrap", ignore_errors=True)
(ROOT / ".github/workflows/bootstrap.yml").unlink(missing_ok=True)
