#!/usr/bin/env python3
"""Check PNG source/ROM roundtrip and native execution using an original fixture."""
from pathlib import Path
import argparse
import subprocess
import sys
import tempfile
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "converter/vendor/shrinko8"))
from pico_cart import read_cart
from test_pipeline import find_compiler
from cartridge_input import prepare_source


def run(args, cwd=None):
    process = subprocess.run([str(a) for a in args], cwd=cwd, capture_output=True, text=True, timeout=120)
    if process.returncode:
        raise AssertionError(process.stdout + process.stderr)
    return process


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", help="compiled pocket-aot executable")
    parser.add_argument("--arm-build", action="store_true", help="also build UF2; requires the Pico SDK and Arm compiler")
    args = parser.parse_args()
    compiler = find_compiler(args.compiler)
    with tempfile.TemporaryDirectory(prefix="pocket-png-check-") as folder:
        folder = Path(folder)
        source = ROOT / "converter/tests/semantics.p8"
        png, decoded = folder / "semantics.p8.png", folder / "decoded.p8"
        # Original blank artwork avoids redistributing Lexaloffle's template/font.
        template = folder / "blank-template.png"
        Image.new("RGBA", (160, 205), (22, 27, 40, 255)).save(template)
        tool = ROOT / "converter/vendor/shrinko8/shrinko8.py"
        run([sys.executable, tool, source, png, "--template-image", template, "--template-only"])
        decoded = prepare_source(png, folder / "decoded")
        original, roundtrip = read_cart(str(source)), read_cart(str(decoded))
        assert original.code == roundtrip.code, "Fixture source changed during container conversion"
        assert bytes(original.rom) == bytes(roundtrip.rom), "Cartridge ROM data changed"
        dest = folder / "generated"
        run([compiler, decoded, dest, "Original PNG fixture"])
        runtime = ROOT / "converter/runtime"
        run(["gcc", "-O1", "-g", "-fsanitize=address,undefined", "-fno-omit-frame-pointer", "-I" + str(runtime),
             runtime / "p8.c", runtime / "host.c", dest / "game.c", dest / "assets.c", "-lm", "-o", dest / "host-check"])
        result = run([dest / "host-check", "600"], cwd=dest)
        assert "AOT_HOST_OK" in result.stdout, result.stdout
        if args.arm_build:
            run([sys.executable, ROOT / "converter/build.py", png, "--title", "Original PNG fixture", "--frames", "600"])
        print("Original-fixture PNG roundtrip preserved source and ROM; generated C passed 600 ASan/UBSan host frames."
              + (" Optional UF2 build passed." if args.arm_build else " UF2 build was not requested."))


if __name__ == "__main__":
    main()
