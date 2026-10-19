# Small gaps, tackled first

Assessment: 2026-10-16. This prioritizes implementation effort and testability, rather than counting every missing API as equally expensive. The [Status page](STATUS.md) retains the same 173 gates, category weights and half-credit rule.

## First batch (49.8% to 51.4%)

| Gap | Why it was small | Change and evidence | Remaining limit |
| --- | --- | --- | --- |
| `rawget`, `rawset`, `rawequal`, `rawlen` | Existing table lookup, mutation, identity and length primitives already did the work | Added compiler bindings and native dispatch, receiver return, nil deletion, identity/type checks, string length, alias calls and controlled errors | Existing bounded tables/strings still apply; this adds no metatable support |
| `select` | Existing native multiple-result transport could carry the selected tail | Count mode including nil; positive/negative indices; empty tails; alias calls and table/argument expansion | At that checkpoint numeric-string index coercion was absent; the second batch adds it. At most 16 total arguments and no `...` syntax remain |
| `camera`, `cursor` | Setters already existed; the missing work was returning the old position | Both coordinates now survive direct and first-class calls; cursor color is preserved | No independent PICO-8 reference comparison |
| `palt` | Transparency was already an array of 16 flags | Added the one-argument bitmask, reset/boolean updates and previous-mask return; tests verify both bit orders using sprite pixels | No independent reference comparison |
| `clip` | Rectangle clipping already existed | Added intersection mode and four previous-rectangle results; tests cover bounds, disjoint rectangles and empty clips | Integer screen clipping only; exact off-screen/fractional state fidelity remains unverified, so status stays **partial** |
| `sset`, `line` | Sprite writes and line rasterization already existed | `sset(x,y)` uses the draw color; line continuation, pen reset and color-only forms work | Remapped sprite memory, fill patterns and exact line raster fidelity remain incomplete; both APIs stay **partial** |
| `assert` results | The same native result bridge exposed an existing scalar-return omission | Successful assertions return all supplied values, including nil | Existing 16-argument/result limit and unverified error-format differences remain |

The original [`api_overloads.p8`](../converter/tests/api_overloads.p8) fixture asserts results and pixels in generated C, then runs for 2,500 frames with ASan/UBSan. Seven additional negative cases verify controlled failures. The existing semantic/closure tests and PNG roundtrip also pass. An RP2040 UF2 of the new fixture builds with the stack-budget checks enabled; it has **not** been flashed or device-confirmed.

## Second batch (51.4% to 52.2%)

| Gap | Implemented now | Still incomplete |
| --- | --- | --- |
| Numeric-string selectors | `select` accepts decimal, hex and binary text with signed/truncated indices | Existing 16-argument limit and missing vararg syntax |
| Numeric conversion | Shared `tonum`/`split` radix parser; exact hex/binary fixed-point bits; `tonum` raw integer/hex flags, failure fallback and zero-result failure | Decimal rounding/overflow, unusual coercions and complete flag behavior remain partial |
| Numeric formatting | `tostr` fixed/raw hex and signed raw-integer flags, empty argument list and nil representation | Default decimal precision and raw object-address formatting remain partial |
| String byte indexing | Positive/negative `text[index]`, empty out-of-range results and P8SCII byte handling | Embedded NUL and full index coercion semantics |
| Multiple byte results | `ord` returns 1–16 in-range byte codes, including through aliases and table-tail expansion | Unsupported counts and multi-result overruns stop explicitly. Exact boundary behavior needs reference evidence |
| Sprite flag reads | `fget` returns zero/false outside sprite/bit bounds instead of wrapping into another entry; valid `fset` modes have assertions | Exact write-boundary behavior remains outside this pass |
| Raw ROM input | `.rom`/`.p8.rom` accepted by the build helper and GUI; shared container path with PNG | Full 32 KB containers, optionally with a 32-byte trailer; code-only tiny ROMs and multi-cart exports are excluded |

The new original [`numeric_strings.p8`](../converter/tests/numeric_strings.p8) asserts these behaviors for 2,500 sanitized frames. Six new controlled-error cases cover invalid selectors/index types and unsupported `ord` ranges. At that checkpoint the public native profile had 20 completed scenarios. [Container tests](../converter/test_containers.py) separately verify four original ROM forms (plain, legacy compression, modern compression, trailer), source/asset/generated-C identity, 600 sanitized frames per form, and eight malformed inputs. PNG decoding uses the same helper and retains its 600-frame check.

The numeric fixture also builds from a compressed raw ROM all the way to RP2040 UF2, with generated stack-budget checks. No device was flashed. A GUI test isolation fix removes launcher environment/executable-directory dependence; it now checks the current inventory rather than a frozen API total.

The only newly closed rubric gate in this batch is **raw ROM input**. API name counts remain 71 functional / 108; the improved string/numeric APIs retain partial credit. The 0.75-point increase uses the same scope and weights and is not a game-compatibility claim.

## Third batch (52.2% unchanged)

| Gap | Implemented now | Still incomplete |
| --- | --- | --- |
| Numeric palette state | `pal(c0,c1[,0])` returns the previous draw register, including its transparency bit; remapping preserves transparency and existing pixels | Display/secondary palettes and memory-mapped palette state remain unsupported |
| Palette tables and reset | Sparse/list draw tables with integer keys 0–16; index 16 addresses color 0; `pal()` and `pal(0)` restore default mapping/transparency | Conflicting keys 0 and 16, unsupported key/value forms and other palettes fail explicitly. Exact table/reset return semantics remain unverified |
| Fractional source literals | Leading-dot hex/binary forms such as `0x.8` and `0b.1`, with exact fixed-point bits; empty, malformed and excess-precision forms are rejected | Existing limits of four hex/16 binary fractional digits and a 32-bit whole part remain; decimal fidelity is unchanged |
| Native bitwise defaults | Each operand of the nine function forms falls back to zero when conversion fails; valid numeric strings use the existing parser. Operator forms retain strict type errors | Numeric-string conversion inherits the partial decimal/coercion behavior; this adds no independent reference validation |

Original [`palette_state.p8`](../converter/tests/palette_state.p8), [`fraction_literals.p8`](../converter/tests/fraction_literals.p8) and [`bitwise_defaults.p8`](../converter/tests/bitwise_defaults.p8) cover saved mappings, pixel/sprite effects, reset, collection, exact fraction bits, numeric-string operands and zero fallback in each operand position. Negative cases cover unsupported palette forms, malformed literals and nonnumeric operator operands.

Confirmed validation: **37 Rust tests and 34 completed sanitizer scenarios**, plus the existing PNG and four-form ROM checks. Each new original fixture passed 2,500 host frames and built to RP2040 UF2 with another 600 sanitized host frames and the stack-budget checks. No device was flashed.

These close small behavior gaps within entries that already had implementation credit. The planning index stays **52.2%**: palette and numeric language coverage remain **partial**, with no new fully closed rubric gate or game-compatibility claim.

## Fourth batch (52.2% to 72.6%)

A broad pass over the APIs and language features that most often stopped real cartridges from converting:

- Memory: `peek`/`poke` (1, 2 and 4 bytes, with the `@`, `%` and `$` operators), `memcpy`, `memset` and `reload` over the full base-RAM map, including draw-state registers and screen memory. Upper memory 0x8000+ is not mapped.
- Saves and system: `cartdata`/`dget`/`dset` (not persisted across power-off), `menuitem` with a Pico pause menu, `flip`, `run`, `reset`, `stop` and `extcmd`.
- Graphics: `oval`/`ovalfill`, `rrect`/`rrectfill`, `fillp` and `tline`, plus screen and secondary palettes.
- Language: `goto`/labels, varargs, metatables, first-class iterators (`next`, `pairs`, `ipairs`, `all`), coroutines on native stacks, NUL-safe strings, `//` comments and the rotate/logical-shift operators.
- Runtime: allocation-triggered garbage collection mid-frame, and stateful silent `sfx`/`music` visible through `stat()`.

All are exercised by the original fixture `converter/tests/extended_features.p8` and new expected-error scenarios. Pixel output for the new shapes is not checked against a PICO-8 reference.

## Fifth batch (72.6% to 72.9%)

Aimed at the blockers measured on the frozen top-100 run (22/100 firmware builds), not at the planning index:

- Pico stack budget: generated C copies globals field by field, binds new locals directly and gives trailing single-result builtins one slot instead of a 16-value expansion area. Long functions in stress fixtures shrink from 1.5–3.5 KB to 0.25–1.8 KB of stack.
- Pools: 768 string slots (was 256), 1280 tables (768), 4352 table entries (4096), with free-slot searches that resume where the last one stopped. The pause menu now saves only its own box, which frees 12 KB of RAM for this, and string compaction reuses the collector's grey stack instead of its own 2 KB buffer. A first try at 1024 string slots left 10 of the top-100 games a few hundred bytes over the Pico's RAM.
- The per-frame instruction guard is 1,000,000 ticks (was 200,000); it only stops runaway loops.
- `load` converts and stops the cart only if it is called; `ls` returns an empty table and `serial` transfers nothing.

The top-100 effect is unmeasured until the benchmark is re-run on a machine that can download the cartridges.

Follow-up: lexical cells (12 to 8 bytes) and closure records (12 to 8 bytes) are packed tighter. The 9 KB this frees goes to table entries, 4352 to 5120, so RAM use is unchanged.

## Smallest remaining candidates

| Priority | Incomplete part | Useful next check |
| --- | --- | --- |
| 1 | `ord` boundary/count behavior | Small implementation after obtaining a trustworthy reference oracle: settle zero/negative counts, overruns and invalid-input result counts; the manual's multi-result example has an apparent string-length typo |
| 2 | Decimal `tonum` fidelity | Medium: replace host floating-point parsing/flooring with exact fixed-point parsing, including wrapping, rounding and exponent boundaries |
| 3 | Drawing state and palette completion | Establish table/reset return behavior and exact off-screen/fractional clipping state; display palettes require backend work, while secondary palettes depend on fill patterns |

Audio synthesis, upper memory, flash-backed saves and `#include` are larger dependencies. A stub or a successful build would not close their acceptance gates.

## Semantics used

The [official PICO-8 manual](https://www.lexaloffle.com/dl/docs/pico-8_manual.html) documents these drawing overloads and advanced Lua functions. The [official 0.2 release notes](https://www.lexaloffle.com/bbs/?tid=37695) establish previous-state returns, default `sset` color and line pen reset. The [PICO-8 author’s string-indexing explanation](https://www.lexaloffle.com/bbs/?pid=116744) establishes positive/negative single-byte indexing with empty out-of-range reads. The [Lua 5.2 base-library source](https://www.lua.org/source/5.2/lbaselib.c.html) specifies raw access, `select` range handling and assertion returns. These informed implementation and hand-authored assertions; they are **not an independent reference execution suite**.

The [PICO-8 contributor's explanation of `pal` register returns](https://www.lexaloffle.com/bbs/?pid=76285) supplies the transparency-bit detail. The manual distinguishes native bitwise zero fallback from strict operator errors. Numeric-string conversion is corroborated by z8lua's [bitwise functions](https://github.com/samhocevar/z8lua/blob/zepto8/lpico8lib.c#L116-L155), [conversion helper](https://github.com/samhocevar/z8lua/blob/zepto8/lvm.c#L32-L40) and [API conversion](https://github.com/samhocevar/z8lua/blob/zepto8/lapi.c#L318-L327); this emulator implementation is supporting evidence, **not an independent PICO-8 oracle**.
