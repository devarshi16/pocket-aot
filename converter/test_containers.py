#!/usr/bin/env python3
"""Check full-size ROM decoding/build inputs with original generated cartridges."""
from pathlib import Path
import argparse
import hashlib
import json
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "converter/vendor/shrinko8"))
from pico_cart import read_cart, write_cart_to_rom, write_cart_to_source
from pico_defs import k_cart_size, k_rom_size
from cartridge_input import prepare_source
from test_pipeline import command, find_compiler


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", help="compiled pocket-aot executable")
    args = parser.parse_args()
    compiler = find_compiler(args.compiler)
    cases, rejected = [], []
    fixture = ROOT / "converter/tests/semantics.p8"
    original = read_cart(str(fixture))
    # Original nonzero sprite/map/flag bytes exercise data identity too.
    for address, value in [(0, 0x21), (0x2000, 0x34), (0x3000, 0x56)]:
        original.rom.set8(address, value)
    runtime = ROOT / "converter/runtime"
    with tempfile.TemporaryDirectory(prefix="pocket-container-check-") as folder:
        base = Path(folder)
        source = base / "original.p8"
        source.write_text(write_cart_to_source(original), encoding="utf-8")
        expected = base / "expected"
        command([compiler, source, expected, "Original container fixture"])
        for name, options in [
            ("plain", {}),
            ("pxa", dict(force_compress=True)),
            ("legacy", dict(force_compress=True, old_compress=True)),
            ("trailer", dict(force_compress=True, with_trailer=True)),
        ]:
            cart = base / (name + ".p8.ROM")
            data = write_cart_to_rom(original, **options)
            cart.write_bytes(data)
            output = base / name
            decoded = prepare_source(cart, output)
            roundtrip = read_cart(str(decoded))
            assert original.code == roundtrip.code, name + ": code changed"
            assert bytes(original.rom) == bytes(roundtrip.rom), name + ": data changed"
            command([compiler, decoded, output, "Original container fixture"])
            for filename in ["game.c", "assets.c"]:
                assert (output / filename).read_bytes() == (expected / filename).read_bytes(), name + ": generated C changed"
            command(["gcc", "-O1", "-g", "-fsanitize=address,undefined", "-fno-omit-frame-pointer",
                     "-I" + str(runtime), runtime / "p8.c", runtime / "host.c", output / "game.c",
                     output / "assets.c", "-lm", "-o", output / "host-check"])
            process = command([output / "host-check", "600"], cwd=output)
            assert "AOT_HOST_OK" in process.stdout, process.stdout
            assert cart.read_bytes() == data, "source container was modified"
            cases.append(dict(format=name, bytes=len(data), source_unchanged=True, code_and_data_identical=True,
                              generated_c_identical=True, sanitized_frames=600))
        valid = write_cart_to_rom(original, force_compress=True, with_trailer=True)
        corrupt = bytearray(valid)
        corrupt[0] ^= 1
        bad_header = bytearray(k_cart_size)
        bad_header[k_rom_size:k_rom_size+8] = b":c:\0\0\1\0\1"
        malformed = [("size-" + str(size), valid[:size] if size <= len(valid) else valid + b"\0")
                     for size in [0, 1, k_cart_size-1, k_cart_size+1, len(valid)-1, len(valid)+1]]
        malformed += [("bad-hash", bytes(corrupt)), ("bad-compression", bytes(bad_header))]
        for name, data in malformed:
            path, output = base / (name + ".rom"), base / name
            path.write_bytes(data)
            try:
                prepare_source(path, output)
            except ValueError:
                assert not (output / "decoded.p8").exists(), name + ": partial source leaked"
                rejected.append(name)
            else:
                raise AssertionError("Invalid container accepted: " + name)
    names = ["converter/cartridge_input.py", "converter/test_containers.py", "converter/tests/semantics.p8",
             "converter/runtime/p8.c", "converter/runtime/p8.h", "converter/runtime/host.c",
             "src/compiler/emit.rs", "src/compiler/mod.rs", "src/compiler/parse.rs", "src/compiler/charset.rs"]
    report = dict(suite="original-rom-containers", cases=cases, rejected=rejected,
                  source_sha256={name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in names},
                  meaning="Source/data/generated-C identity and host assertions; no device flashing or independent PICO-8 reference comparison.")
    (ROOT / "converter/container-validation.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"ROM input: {len(cases)} original container forms preserved source/data/generated C and passed 600 sanitizer frames each; {len(rejected)} malformed inputs rejected.")


if __name__ == "__main__":
    main()
