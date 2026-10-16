#!/usr/bin/env python3
"""Guard the tracked publication surface; pair this with human provenance review."""
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
OWN_CARTS = {"converter/tests/semantics.p8", "converter/tests/functions.p8", "converter/tests/api_overloads.p8", "converter/tests/numeric_strings.p8", "converter/tests/fraction_literals.p8", "converter/tests/palette_state.p8", "converter/tests/bitwise_defaults.p8"}
DENIED_ROOTS = {
    "recovery", "firmware", "converted", "target", "bin", "logs",
    ".cache", ".toolchain", ".python-tools", ".venv", "local-carts",
}


def main():
    tracked = subprocess.run(
        ["git", "ls-files", "-z"], cwd=ROOT, check=True,
        capture_output=True,
    ).stdout.decode().split("\0")
    files = [p for p in tracked if p]
    if not files:
        raise SystemExit("No tracked files to audit; stage the intended publication first.")
    failures = []
    for name in files:
        path = Path(name)
        suffix = path.suffix.lower()
        if path.parts[0] in DENIED_ROOTS or ".git" in path.parts or "__pycache__" in path.parts:
            failures.append(f"Private/build directory: {name}")
        if name.startswith("compatibility/carts/") or re.match(r"compatibility/top200/\d+/", name):
            failures.append(f"External cartridge directory: {name}")
        if suffix in {".uf2", ".elf", ".bin", ".rom", ".o", ".pyc", ".p64"} or name.lower().endswith(".p8.png"):
            failures.append(f"Firmware/cartridge/binary content: {name}")
        if suffix == ".p8" and name not in OWN_CARTS:
            failures.append(f"Unreviewed cartridge source: {name}")
        data = (ROOT / path).read_bytes()
        if data[:4] == bytes.fromhex("5546320a"):
            failures.append(f"UF2 content under another extension: {name}")
        # Binary media and the licensed font are reviewed separately.
        if suffix in {".png", ".jpg", ".jpeg", ".ttf", ".mp4", ".webm"}:
            continue
        text = data.decode("utf-8", errors="replace")
        if re.search(r"/home/[^/\s]+/(?:Documents|\.codex)/", text):
            failures.append(f"Personal workspace path: {name}")
        if re.search(r"-----BEGIN (?:RSA |OPENSSH |EC )?PRIVATE KEY-----", text):
            failures.append(f"Private key: {name}")
        if re.search(r"\b(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{30,})\b", text):
            failures.append(f"GitHub credential: {name}")
    if failures:
        print("Publication audit failed:\n" + "\n".join(failures), file=sys.stderr)
        raise SystemExit(1)
    print(f"Publication audit passed: {len(files)} tracked files; only reviewed original .p8 fixtures; no bundled game firmware or private paths.")


if __name__ == "__main__":
    main()
