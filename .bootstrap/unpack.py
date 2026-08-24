from __future__ import annotations

import base64
import hashlib
import io
import shutil
import tarfile
from pathlib import Path

EXPECTED_SHA256 = "6c566c86ba2c32fef2987f2e52ed5939cb6fa0a54d13565eaedc2b88615735b3"

root = Path(__file__).resolve().parents[1]
bootstrap = root / ".bootstrap"
parts = sorted(bootstrap.glob("v2-*.txt"))

if not parts:
    raise RuntimeError("Rune v2 source payload is missing")

encoded = "".join(part.read_text(encoding="utf-8").strip() for part in parts)
archive_bytes = base64.b64decode(encoded, validate=True)
actual_sha256 = hashlib.sha256(archive_bytes).hexdigest()
if actual_sha256 != EXPECTED_SHA256:
    raise RuntimeError(
        f"source payload checksum mismatch: expected {EXPECTED_SHA256}, got {actual_sha256}"
    )

workflow = root / ".github" / "workflows" / "bootstrap.yml"
workflow_bytes = workflow.read_bytes()

for child in root.iterdir():
    if child.name in {".git", ".bootstrap"}:
        continue
    if child.is_dir():
        shutil.rmtree(child)
    else:
        child.unlink()

root_resolved = root.resolve()
with tarfile.open(fileobj=io.BytesIO(archive_bytes), mode="r:gz") as bundle:
    for member in bundle.getmembers():
        if member.issym() or member.islnk():
            raise RuntimeError(f"links are not allowed in source payload: {member.name}")
        target = (root / member.name).resolve()
        if target != root_resolved and root_resolved not in target.parents:
            raise RuntimeError(f"unsafe archive member: {member.name}")
    bundle.extractall(root)

workflow.parent.mkdir(parents=True, exist_ok=True)
workflow.write_bytes(workflow_bytes)

# The archive predates stricter closure-parameter inference in current Rust.
vm_path = root / "crates" / "rune-core" / "src" / "vm.rs"
vm = vm_path.read_text(encoding="utf-8")
vm = vm.replace(
    "Edge, HandlerTable, InputDevice, InputEvent, Instruction, MouseButton, Opcode, OutputEvent,",
    "Edge, HandlerTable, InputDevice, InputEvent, MouseButton, Opcode, OutputEvent,",
)
vm = vm.replace(
    "binary!(|left, right| left.wrapping_shl((right as u32) & 63))",
    "binary!(|left: i64, right: i64| left.wrapping_shl((right as u32) & 63))",
)
vm = vm.replace(
    "binary!(|left, right| left.wrapping_shr((right as u32) & 63))",
    "binary!(|left: i64, right: i64| left.wrapping_shr((right as u32) & 63))",
)
vm_path.write_text(vm, encoding="utf-8")

required = [
    root / "packages" / "compiler" / "src" / "compiler.ts",
    root / "packages" / "sdk" / "src" / "realtime.ts",
    root / "crates" / "rune-core" / "src" / "vm.rs",
    root / "crates" / "rune-native" / "src" / "lib.rs",
    root / ".github" / "workflows" / "ci.yml",
]
missing = [str(path.relative_to(root)) for path in required if not path.is_file()]
if missing:
    raise RuntimeError(f"source payload is incomplete: {', '.join(missing)}")

shutil.rmtree(bootstrap)
