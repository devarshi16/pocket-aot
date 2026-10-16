# Frozen first-100 benchmark

On 2026-10-16, the unmodified converter at commit `7ad2e3f` tested ranks 1–100 from the [Nerdy Teachers Top 200](https://nerdyteachers.com/PICO-8/Games/Top200/) page. No random sampling or title substitution was used. The selection file records the page hash, source links and authors.

**Result: 2/100 RP2040 firmware builds**, after 100 successful downloads/decodes, 7 C translations and 2 successful 1,200-frame ASan/UBSan host checks. The passing titles were Dusk Child (#19) and Hug Arena (#63). Five translated games failed runtime checks; 93 stopped at translation. No device was flashed in this benchmark. Neither build success nor the scripted smoke test establishes complete gameplay or independent PICO-8 conformance.

See [REPORT.md](REPORT.md) for all outcomes, [blocker-summary.json](blocker-summary.json) for first-blocker groups and [provenance.json](provenance.json) for implementation/tool hashes. Each failed title contributes only its first observed blocker; implementing that feature can expose further failures. This benchmark is separate from the earlier random 20-game sample and the public original-fixture regression suite.

AddressSanitizer and UndefinedBehaviorSanitizer stayed enabled. LeakSanitizer cannot run under the test host's sandbox ptrace, so leak detection alone was disabled in the final run. The initial tooling-failed attempt is retained privately and is not a compatibility result.

Only metadata is distributed: no cartridges, decoded sources, generated C, page HTML, per-game artifacts or firmware. Original private artifact paths are omitted. The recorded original benchmark-runner hash identifies the private execution script; `converter/benchmark_top100.py` is its portable adaptation, using configured SDK/toolchain paths and ignored output directories.

## Repeat locally

Build the converter and configure the SDK/Arm toolchain as described in the [converter guide](../../converter/README.md). Downloads happen only on explicit invocation, never in public CI:

```sh
cargo build --release --locked --bin pocket-aot
python3 converter/benchmark_top100.py download
python3 converter/benchmark_top100.py test
```

The default output directory is ignored `.cache/top100/`. `POCKET_AOT_BIN` or `--compiler` selects a built compiler; `PICO_SDK_PATH` and optional `PICO_TOOLCHAIN_PATH` configure the existing build helper. Neither command adds shelf entries or flashes USB.

To repeat this exact frozen selection rather than the latest list, copy this directory's `selection.json` to the chosen private output directory before `download`. The downloader obtains current public cartridge links from those author posts; compare cartridge/source hashes with the recorded results because authors can update their uploads. It tests the first public Cart File link as-is and does not assemble multi-cart exports or includes. Unavailable downloads remain explicit failures within the fixed denominator of 100.
