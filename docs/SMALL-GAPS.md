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

The new original [`numeric_strings.p8`](../converter/tests/numeric_strings.p8) asserts these behaviors for 2,500 sanitized frames. Six new controlled-error cases cover invalid selectors/index types and unsupported `ord` ranges. The public native profile now has 20 completed scenarios. [Container tests](../converter/test_containers.py) separately verify four original ROM forms (plain, legacy compression, modern compression, trailer), source/asset/generated-C identity, 600 sanitized frames per form, and eight malformed inputs. PNG decoding uses the same helper and retains its 600-frame check.

The numeric fixture also builds from a compressed raw ROM all the way to RP2040 UF2, with generated stack-budget checks. No device was flashed. A GUI test isolation fix removes launcher environment/executable-directory dependence; it now checks the current inventory rather than a frozen API total.

The only newly closed rubric gate in this batch is **raw ROM input**. API name counts remain 71 functional / 108; the improved string/numeric APIs retain partial credit. The 0.75-point increase uses the same scope and weights and is not a game-compatibility claim.

## Smallest remaining candidates

| Priority | Incomplete part | Useful next check |
| --- | --- | --- |
| 1 | Numeric `pal` previous-state return | Small: reuse the existing palette map, after establishing return shape and reset behavior; table and display-palette overloads remain larger gaps |
| 2 | Leading-dot hex/binary source literals | Small frontend change: accept valid fractional forms and reject empty/malformed literals; runtime numeric strings already support fractions |
| 3 | `ord` boundary/count behavior | Small implementation after a reference check: settle zero/negative counts, overruns and invalid-input result counts; the manual's multi-result example has an apparent string-length typo |
| 4 | Decimal `tonum` fidelity | Medium: replace host floating-point parsing/flooring with exact fixed-point parsing, including wrapping, rounding and exponent boundaries |
| 5 | Rounded rectangles / ellipses | Medium: reuse pixel/clipping primitives, but establish reference raster and fill-pattern behavior |

Audio, coroutines, metatables, full memory/register emulation and persistent saves are larger dependencies. A stub or a successful build would not close their acceptance gates.

## Semantics used

The [official PICO-8 manual](https://www.lexaloffle.com/dl/docs/pico-8_manual.html) documents these drawing overloads and advanced Lua functions. The [official 0.2 release notes](https://www.lexaloffle.com/bbs/?tid=37695) establish previous-state returns, default `sset` color and line pen reset. The [PICO-8 author’s string-indexing explanation](https://www.lexaloffle.com/bbs/?pid=116744) establishes positive/negative single-byte indexing with empty out-of-range reads. The [Lua 5.2 base-library source](https://www.lua.org/source/5.2/lbaselib.c.html) specifies raw access, `select` range handling and assertion returns. These informed implementation and hand-authored assertions; they are **not an independent reference execution suite**.
