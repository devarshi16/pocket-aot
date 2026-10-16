# Pocket AOT converter

A self-contained `.p8` source cartridge or decoded `.p8.png` becomes generated C, links against a bounded C runtime, and compiles to an RP2040 UF2. Rust handles parsing and code generation on the computer. After flashing, the program runs independently on the Pico, without a Lua interpreter, desktop game process or network service.

This is an experimental PICO-8 language/API subset. The [Status page](../docs/STATUS.md) separates implementation coverage, executed tests, historical game outcomes and the weighted planning estimate. Unsupported syntax/APIs stop conversion; unsupported runtime arguments and exhausted resource pools produce controlled errors.

## Build the converter

Install Rust/Cargo, Python 3, GCC and the [Python dependencies](../requirements.txt). From the repository root:

```sh
cargo build --release --locked --bins
python3 -m pip install -r requirements.txt
```

The Rust stage can be used without the Pico SDK:

```sh
target/release/pocket-aot /path/to/game.p8 /path/to/generated 'My Game'
```

It writes `game.c`, `assets.c` and `conversion.json`; the input is not rewritten. The frontend accepts decoded `.p8` text. `build.py` accepts `.p8.png` through the vendored, MIT-licensed Shrinko8 format decoder; it does not execute or minify cartridge Lua.

## Build RP2040 firmware

Install CMake, Ninja, an Arm bare-metal GCC toolchain and a Raspberry Pi Pico SDK checkout with its required submodules. Set the SDK path and make `arm-none-eabi-gcc` available on `PATH`:

```sh
export PICO_SDK_PATH=/path/to/pico-sdk
python3 converter/build.py /path/to/game.p8 --title 'My Game' --library
```

If the Arm compiler is outside `PATH`, set `PICO_TOOLCHAIN_PATH` to its installation directory. `POCKET_AOT_BIN` can point to a prebuilt compiler executable; otherwise the build helper asks Cargo to build the release compiler. `CARGO_TARGET_DIR` is honored.

The helper translates, runs generated C with ASan/UBSan, compiles firmware, checks generated stack usage, and records source/firmware hashes. `--library` adds a shelf card only after these stages succeed. Omit it to build without changing the shelf. Conversion does not flash hardware. Output goes to ignored `converted/` and `firmware/` directories.

In Pocket Arcade, use **+ ADD CARTRIDGE**, choose a local `.p8` or `.p8.png`, then **CONVERT + ADD TO COLLECTION**. Drag and drop also works. Select a successfully built card and press **FLASH TO PICO**. The grid/list browser supports search, status filters, sorting and artwork fallback. Use **PROGRESS** for the subsystem dashboard and searchable API matrix.

## Hardware scope

The cartridge parser, native C generator and core runtime are largely independent of the display. The shipped firmware backend is currently specific to **RP2040 Raspberry Pi Pico/Pico W with Waveshare Pico LCD 1.3**, a 240×240 ST7789 IPS LCD over SPI. It is an LCD, not an OLED. It is not a Pico 2/RP2350 target.

`converter/runtime/pico.c` supplies display transfer, palette expansion/scaling, GPIO input, frame scheduling, USB diagnostics and recovery. Supporting another display or button board requires a backend adaptation and hardware testing. There is no hardware-profile selector yet. The current firmware also assumes 2 MB flash and the fixed stack/memory layout.

| Signal | GPIO |
| --- | ---: |
| LCD SPI1 clock / MOSI | 10 / 11 |
| LCD CS / DC / reset / backlight | 9 / 8 / 12 / 13 |
| Left / right / up / down | 16 / 20 / 2 / 18 |
| Stick centre | 3 |
| A / B / X / Y | 15 / 17 / 19 / 21 |

Directions map to PICO-8 buttons 0–3, A to 4 and B to 5. X or centre click pauses; Y changes LCD speed; holding X+Y for two seconds requests BOOTSEL. Audio is silent: the current runtime discards `sfx`/`music`, and neither synthesis nor stateful silent audio is implemented.

## Implemented starting set

- Signed 16.16 fixed-point values, wrapping arithmetic and basic math/bit/string functions. Decimal literals round to fixed point. Numeric edge behavior and RNG sequences are not independently PICO-8-verified.
- Nil, booleans, strings, arrays/record tables, insertion/deletion/counting, and frame-boundary garbage collection, including cycles through tables, closures and lexical cells.
- Named, local and anonymous native functions; function values, methods, mutable captures, top-level locals, multiple returns and last-expression expansion. Up to 16 call arguments/results; no varargs.
- Raw table access/equality/length helpers, bounded `select` result tails, previous camera/cursor/clip state returns, transparency bitmasks and line continuation. See [small-gap details and remaining limits](../docs/SMALL-GAPS.md).
- Lexical scopes, parallel/compound assignments, numeric `for`, built-in `all`/`pairs`/`ipairs` loops, `foreach`, `while`, `repeat`, `break`, conditionals and short-circuit logic.
- `_init`, `_update`/`_update60`, `_draw`; basic pixel, rectangle, circle, line, text, sprite, map, clipping, camera and palette operations. Only documented implemented overloads are accepted.
- Sprite/map/flag import and shared lower-sprite/map memory; player-one `btn`/`btnp` with default held-button repeat.
- P8SCII source-byte conversion. Text uses an independently authored compact ASCII font; extended glyph and advanced text behavior are approximate.

See [API audit](../compatibility/coverage/api-audit.json) and [full checklist](../compatibility/coverage/CHECKLIST.md) for per-entry limitations. The fixed API inventory has **33 supported, 38 partial and 37 missing entries**; a functional name is not a claim of complete behavior.

## Limits and recovery

Missing capabilities include varargs, `goto`/labels, active includes, metatables, coroutines, custom generic iterators, persistent saves, menu callbacks, raw PICO-8 memory APIs, complete graphics/text behavior and audio. `stat(1)` is a coarse native instruction-budget fraction, not emulated PICO-8 CPU usage. Other unsupported queries stop explicitly.

The runtime has fixed pools: **512 tables, 3,072 table entries, 1,024 lexical cells, 256 closures and 2,048 captured-cell links**. A closure captures at most 64 locals. Strings use a 32 KB arena with 256 slots and an 8,191-byte individual limit. Allocation within one frame can exhaust a pool before frame-boundary collection.

Instruction/call-depth budgets, stack-headroom guards and compiler stack-usage checks limit runaway code. Core 0 reserves 16 KB of main RAM for stack; core 1 reserves 4 KB in scratch RAM. Builds reject oversized or non-static generated stack transitions. These checks do not guarantee that every accepted cartridge fits its intended frame deadline. Keep the board's physical BOOTSEL recovery accessible.

## Public tests

All default tests use original fixtures included in this repository. No external cartridges, Pico SDK or device are required:

```sh
cargo test --locked
cargo build --release --locked --bin pocket-aot
python3 converter/test_pipeline.py
python3 converter/test_png.py
python3 converter/coverage_report.py --check
```

| Original fixture | Check |
| --- | --- |
| Runtime/closure C assertions | 2,100 frames; cells, iterators, multiple returns and string arena |
| `semantics.p8` | 5,000 generated-native frames; arithmetic, ordering, scopes, tables, cyclic GC, graphics/shared memory |
| `functions.p8` | 5,000 frames; captures, function values, methods and return expansion |
| `api_overloads.p8` | 2,500 frames; raw helpers, select/native result lists, clipping/transparency pixels and line continuation |
| Seven generated invalid API calls | Controlled select index/type, raw table/key/type and aliased arity failures |
| Generated infinite loop | Expected controlled instruction-budget failure |
| Generated oversized table | Expected controlled table-entry failure |
| Generated unsupported `reload` / `poke` | Explicit conversion rejection, no generated game output |
| Semantic-fixture PNG roundtrip | Source/ROM identity plus 600 sanitized generated-native frames |

The native runner records only completed positive or expected-error cases in [runtime-api-coverage.json](../compatibility/coverage/runtime-api-coverage.json). Its current public suite reaches **42/108 functional API names across 13 sanitizer scenarios**. Two rejection checks and the PNG check are separate. Entry hits do not prove correct outputs, all overloads or independent reference conformance. Profiling is compiled out of Pico firmware.

`test_png.py --arm-build` additionally builds UF2 when the SDK and Arm compiler are installed. `test_pipeline.py --external-cart /path/to/game.p8` explicitly adds a private local source check; repeated flags add more. This never downloads, publishes or flashes the supplied game, and the resulting coverage is labeled as including external sources.

Earlier private testing reached **48/108 API names across eight scenarios**, including third-party games. That profile is retained unchanged as [historical evidence](../compatibility/coverage/historical-runtime-api-coverage.json), separate from fresh public tests. The earlier frozen 20-game sample built and owner-confirmed **Dusk Child, 1/20**, on the original RP2040/LCD setup. Only metadata, source hashes, links and permitted screenshots are published; third-party source, cartridge PNG payloads and firmware are excluded. These historical outcomes are not a complete playthrough or current public regression pass.

## Maintain status

`python3 converter/coverage_report.py --write` regenerates the public [Status page](../docs/STATUS.md), [SVG chart](../docs/status.svg), detailed [planning report](PROGRESS.md), checklist and machine-readable summary. `--check` validates rubric consistency and source freshness. Re-audit changed implementation before updating evidence hashes; never award progress automatically for code size or test count. Scope/weight changes require a new rubric version.

## Source layout and credits

- `src/compiler/{parse,emit,mod,charset}.rs`: source parsing, code generation, cartridge assets and source character mapping.
- `src/bin/pocket-aot.rs`: standalone Rust frontend.
- `converter/runtime/p8.{c,h}`: bounded values, memory management, functions and API behavior.
- `converter/runtime/{pico,host}.c`: physical board backend and sanitizer harness.
- `converter/runtime/font.h`: independently authored compact font.
- `converter/build.py`: host check, firmware build and shelf packaging.
- `converter/vendor/shrinko8/`: pinned third-party decoder source, under its included MIT license.

Behavior is informed by the [official PICO-8 manual](https://www.lexaloffle.com/dl/docs/pico-8_manual.html). The LCD initialization lineage includes Waveshare's published driver; project and dependency notices are in [NOTICE.md](../NOTICE.md). PICO-8 is a trademark of Lexaloffle Games. This is an independent project.
