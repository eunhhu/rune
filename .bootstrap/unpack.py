from __future__ import annotations

import base64
import shutil
import tarfile
from pathlib import Path

root = Path(__file__).resolve().parents[1]
bootstrap = root / ".bootstrap"
parts = sorted(bootstrap.glob("archive-*.txt"))

if not parts:
    raise RuntimeError("Rune bootstrap archive is missing")

payload = "".join(part.read_text(encoding="utf-8").strip() for part in parts)
archive = bootstrap / "rune.tar.gz"
archive.write_bytes(base64.b64decode(payload, validate=True))

root_resolved = root.resolve()
with tarfile.open(archive, "r:gz") as bundle:
    for member in bundle.getmembers():
        if member.issym() or member.islnk():
            raise RuntimeError(f"links are not allowed in bootstrap archive: {member.name}")
        target = (root / member.name).resolve()
        if target != root_resolved and root_resolved not in target.parents:
            raise RuntimeError(f"unsafe archive member: {member.name}")
    bundle.extractall(root)

required = [
    root / "crates" / "rune-native" / "src" / "lib.rs",
    root / "packages" / "sdk" / "src" / "native.ts",
    root / "packages" / "sdk" / "test" / "encode.test.ts",
]
missing = [str(path.relative_to(root)) for path in required if not path.is_file()]
if missing:
    raise RuntimeError(f"bootstrap archive is incomplete: {', '.join(missing)}")

(root / ".github" / "workflows" / "bootstrap.yml").unlink(missing_ok=True)
shutil.rmtree(bootstrap)
