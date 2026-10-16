#!/usr/bin/env python3
"""Run original public fixtures against generated C with ASan and UBSan.

Optional external cartridges are supplied explicitly. Nothing is downloaded,
executed as Lua, added to the shelf, or flashed by this test runner.
"""
from pathlib import Path
import argparse
import hashlib
import json
import os
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent
RUNTIME = ROOT / "converter/runtime"


def find_compiler(value=None):
    value = value or os.environ.get("POCKET_AOT_BIN") or os.environ.get("POCKET_AOT")
    if value:
        path = Path(value).expanduser().resolve()
        if not path.is_file():
            raise SystemExit(f"Compiler does not exist: {path}")
        return path
    target = Path(os.environ.get("CARGO_TARGET_DIR", str(ROOT / "target")))
    if not target.is_absolute():
        target = ROOT / target
    for path in (target / "release/pocket-aot", target / "debug/pocket-aot"):
        if path.is_file():
            return path
    raise SystemExit("Build the compiler first: cargo build --release --locked --bin pocket-aot")


def command(args, cwd=None, timeout=60, ok=True):
    process = subprocess.run([str(a) for a in args], cwd=cwd, text=True,
                             stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout)
    if ok and (process.returncode or "runtime error:" in process.stdout):
        raise AssertionError(process.stdout)
    return process


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", default=os.environ.get("POCKET_AOT_BIN") or os.environ.get("POCKET_AOT"), help="compiled pocket-aot executable")
    parser.add_argument("--external-cart", type=Path, action="append", default=[],
                        help="also check this local .p8; may be repeated; never downloaded or redistributed")
    parser.add_argument("--external-frames", type=int, default=1200)
    args = parser.parse_args()
    if args.external_frames < 1:
        parser.error("--external-frames must be positive")
    compiler = find_compiler(args.compiler)
    results, profiles, source_hashes = [], [], {}

    def profile(name, dest, source=None, frames=0, expected_error=None, origin="original-public-fixture"):
        data = json.loads((dest / "api-profile.json").read_text())
        canonical = {}
        for key, value in data.items():
            key = "time" if key == "t" else key
            canonical[key] = canonical.get(key, 0) + value
        profiles.append(dict(name=name, frames=frames, api_hits=canonical, expected_error=expected_error,
                             passed=True, origin=origin, sanitizers=["address", "undefined"]))
        if source is not None and source.is_relative_to(ROOT):
            source_hashes[str(source.relative_to(ROOT))] = hashlib.sha256(source.read_bytes()).hexdigest()

    with tempfile.TemporaryDirectory(prefix="pocket-aot-test-") as folder:
        base = Path(folder)
        command(["gcc", "-DP8_API_PROFILE", "-std=c11", "-g", "-fsanitize=address,undefined",
                 "-fno-omit-frame-pointer", "-I" + str(RUNTIME), ROOT / "converter/tests/runtime_closures.c",
                 RUNTIME / "p8.c", "-lm", "-o", base / "runtime-check"])
        process = command([base / "runtime-check"], cwd=base)
        assert "RUNTIME_CLOSURE_ITERATOR_OK" in process.stdout, process.stdout
        profile("runtime-closures", base, frames=2100)
        print(process.stdout.strip(), flush=True)
        results.append(dict(test="runtime-closures", frames=2100, result=process.stdout.strip()))

        def check(name, source, frames, expected_error=None, origin="original-public-fixture"):
            dest = base / name
            before = hashlib.sha256(source.read_bytes()).hexdigest()
            command([compiler, source, dest, name])
            command(["gcc", "-DP8_API_PROFILE", "-O1", "-g", "-fsanitize=address,undefined",
                     "-fno-omit-frame-pointer", "-I" + str(RUNTIME), RUNTIME / "p8.c", RUNTIME / "host.c",
                     dest / "game.c", dest / "assets.c", "-lm", "-o", dest / "host-check"])
            process = command([dest / "host-check", frames], cwd=dest, ok=expected_error is None)
            if expected_error:
                assert process.returncode == 1 and expected_error in process.stdout, process.stdout
                assert "AddressSanitizer" not in process.stdout and "runtime error:" not in process.stdout, process.stdout
            else:
                assert "AOT_HOST_OK" in process.stdout, process.stdout
            assert hashlib.sha256(source.read_bytes()).hexdigest() == before
            profile(name, dest, source, frames, expected_error, origin)
            results.append(dict(test=name, frames=frames, result=process.stdout.strip(),
                                source_sha256=before, source_modified=False, origin=origin))
            print(name + ": " + process.stdout.strip(), flush=True)

        check("semantics", ROOT / "converter/tests/semantics.p8", 5000)
        check("functions", ROOT / "converter/tests/functions.p8", 5000)
        check("api-overloads", ROOT / "converter/tests/api_overloads.p8", 2500)
        for name, body, expected in [
            ("loop-budget", "function _update() while true do end end", "frame instruction budget exceeded"),
            ("select-zero", "function _init() select(0,1) end", "select index out of range"),
            ("select-negative", "function _init() select(-3,1,2) end", "select index out of range"),
            ("select-type", 'function _init() select("bad",1) end', "number expected"),
            ("rawget-type", "function _init() rawget(1,2) end", "table expected"),
            ("rawset-nil-key", "function _init() rawset({},nil,1) end", "nil table key"),
            ("rawlen-type", "function _init() rawlen(false) end", "table expected"),
            ("raw-alias-arity", "function _init() local get=rawget get({}) end", "rawget requires two arguments"),
            ("heap-bound", "function _init() local a={} for i=1,5000 do a[i]=i end end", "table entries exhausted"),
        ]:
            source = base / (name + ".p8")
            source.write_text("pico-8 cartridge\n__lua__\n" + body + "\nfunction _draw() cls() end\n")
            check(name, source, 1, expected)
        for name, body, expected in [
            ("unsupported-reload", "reload()", "unsupported function/API 'reload'"),
            ("unsupported-poke", "poke(0,1)", "unsupported function/API 'poke'"),
        ]:
            source = base / (name + ".p8")
            source.write_text("pico-8 cartridge\n__lua__\nfunction _draw() " + body + " end\n")
            process = command([compiler, source, base / name, name], ok=False)
            assert process.returncode == 1 and expected in process.stdout, process.stdout
            assert not (base / name / "game.c").exists()
            results.append(dict(test=name, result="rejected explicitly", detail=process.stdout.strip(), flashed=False))
            print(name + ": " + process.stdout.strip(), flush=True)
        for index, path in enumerate(args.external_cart):
            source = path.expanduser().resolve(strict=True)
            check(f"external-{index + 1}", source, args.external_frames, origin="explicit-local-external-cartridge")

    (ROOT / "converter/validation.json").write_text(json.dumps(results, indent=2) + "\n")
    # Hits record only completed positive or explicitly expected-error scenarios.
    # An executable hash is provenance, not a committed-source freshness gate.
    for name in ["converter/runtime/p8.c", "converter/runtime/p8.h", "converter/runtime/host.c",
                 "converter/runtime/font.h", "converter/tests/runtime_closures.c", "converter/test_pipeline.py",
                 "src/compiler/parse.rs", "src/compiler/emit.rs", "src/compiler/mod.rs", "src/compiler/charset.rs"]:
        path = ROOT / name
        if path.exists():
            source_hashes[name] = hashlib.sha256(path.read_bytes()).hexdigest()
    hits = {}
    for scenario in profiles:
        for name, count in scenario["api_hits"].items():
            hits[name] = hits.get(name, 0) + count
    coverage = dict(schema_version=1, suite="public-original-fixtures" if not args.external_cart else "public-plus-explicit-external",
                    source_sha256=source_hashes, compiler_sha256=hashlib.sha256(compiler.read_bytes()).hexdigest(),
                    api_hits=hits, scenarios=profiles,
                    meaning="Observed host execution hits in completed sanitizer scenarios. No-op calls and reference conformance are separate from functional coverage.")
    coverage_path = ROOT / "compatibility/coverage/runtime-api-coverage.json"
    coverage_path.parent.mkdir(parents=True, exist_ok=True)
    temp = coverage_path.with_suffix(".tmp")
    temp.write_text(json.dumps(coverage, indent=2) + "\n")
    temp.replace(coverage_path)
    print(f"All native-code and rejection checks passed; recorded {len(profiles)} completed scenarios.")


if __name__ == "__main__":
    main()
