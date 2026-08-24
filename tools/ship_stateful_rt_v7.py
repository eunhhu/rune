from __future__ import annotations

import runpy
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

if not (ROOT / "crates/rune-core/src/rt_vm.rs").exists():
    generators = [
        ROOT / "tools/ship_stateful_rt_v6.py",
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
vm = vm.replace("i64::from(held != 0)", "(held != 0) as i64")
vm = vm.replace("i64::from(operation(left, right))", "operation(left, right) as i64")
vm = vm.replace(
'''    scratch.output[scratch.output_len] = event;
    scratch.output_len += 1;''',
'''    let index = scratch.output_len;
    scratch.output[index] = event;
    scratch.output_len = index + 1;''',
)
vm = vm.replace("RtInstruction::JumpIfFalse(17),", "RtInstruction::JumpIfFalse(18),")
vm_path.write_text(vm, encoding="utf-8")

wire_path = ROOT / "crates/rune-core/src/rt_wire.rs"
wire = wire_path.read_text(encoding="utf-8")
wire = wire.replace(
'''    let device = InputDevice::try_from(trigger.device)
        .map_err(|()| RtDecodeError::new("invalid trigger device"))?;
    let edge = Edge::try_from(trigger.edge)
        .map_err(|()| RtDecodeError::new("invalid trigger edge"))?;
    let source = SourceFilter::try_from(trigger.source)
        .map_err(|()| RtDecodeError::new("invalid trigger source"))?;''',
'''    let device = match trigger.device {
        0 => InputDevice::Keyboard,
        1 => InputDevice::MouseButton,
        _ => return Err(RtDecodeError::new("invalid trigger device")),
    };
    let edge = match trigger.edge {
        0 => Edge::Down,
        1 => Edge::Up,
        _ => return Err(RtDecodeError::new("invalid trigger edge")),
    };
    let source = match trigger.source {
        0 => SourceFilter::Physical,
        1 => SourceFilter::Synthetic,
        2 => SourceFilter::Any,
        _ => return Err(RtDecodeError::new("invalid trigger source")),
    };''',
)
wire = wire.replace(
'''        WireInstruction::MouseDown { button } => RtInstruction::MouseDown(
            MouseButton::try_from(button)
                .map_err(|()| RtDecodeError::new("invalid mouse button"))?,
        ),
        WireInstruction::MouseUp { button } => RtInstruction::MouseUp(
            MouseButton::try_from(button)
                .map_err(|()| RtDecodeError::new("invalid mouse button"))?,
        ),''',
'''        WireInstruction::MouseDown { button } => {
            RtInstruction::MouseDown(decode_mouse_button(button)?)
        }
        WireInstruction::MouseUp { button } => {
            RtInstruction::MouseUp(decode_mouse_button(button)?)
        }''',
)
if "fn decode_mouse_button" not in wire:
    anchor = "fn validate_instruction(\n"
    helper = '''fn decode_mouse_button(button: u8) -> Result<MouseButton, RtDecodeError> {
    match button {
        0 => Ok(MouseButton::Left),
        1 => Ok(MouseButton::Right),
        2 => Ok(MouseButton::Middle),
        3 => Ok(MouseButton::Back),
        4 => Ok(MouseButton::Forward),
        _ => Err(RtDecodeError::new("invalid mouse button")),
    }
}

'''
    wire = wire.replace(anchor, helper + anchor, 1)
wire_path.write_text(wire, encoding="utf-8")

global_path = ROOT / "crates/rune-core/src/global_rt.rs"
global_text = global_path.read_text(encoding="utf-8")
global_text = global_text.replace(
    "SLOT.get_or_init(|| ArcSwapOption::from(None))",
    "SLOT.get_or_init(|| ArcSwapOption::from(None::<Arc<RtEngine>>))",
)
global_path.write_text(global_text, encoding="utf-8")

for disposable in [
    ROOT / "tools/ship_stateful_rt_v2.py",
    ROOT / "tools/ship_stateful_rt_v3.py",
    ROOT / "tools/ship_stateful_rt_v4.py",
    ROOT / "tools/ship_stateful_rt_v5.py",
    ROOT / "tools/ship_stateful_rt_v6.py",
    ROOT / ".github/workflows/ship-stateful-runtime-v2.yml",
    ROOT / ".github/workflows/ship-stateful-runtime-v3.yml",
    ROOT / ".github/workflows/ship-stateful-runtime-v4.yml",
    ROOT / ".github/workflows/ship-stateful-runtime-v5.yml",
    ROOT / ".github/workflows/ship-stateful-runtime-v6.yml",
]:
    disposable.unlink(missing_ok=True)

shutil.rmtree(ROOT / ".bootstrap", ignore_errors=True)
(ROOT / ".github/workflows/bootstrap.yml").unlink(missing_ok=True)
