# Small gaps, tackled first

Assessment: 2026-10-16. This prioritizes implementation effort and testability, rather than counting every missing API as equally expensive. The [Status page](STATUS.md) retains the same 173 gates, category weights and half-credit rule.

## Implemented in this pass

| Gap | Why it was small | Change and evidence | Remaining limit |
| --- | --- | --- | --- |
| `rawget`, `rawset`, `rawequal`, `rawlen` | Existing table lookup, mutation, identity and length primitives already did the work | Added compiler bindings and native dispatch, receiver return, nil deletion, identity/type checks, string length, alias calls and controlled errors | Existing bounded tables/strings still apply; this adds no metatable support |
| `select` | Existing native multiple-result transport could carry the selected tail | Count mode including nil; positive/negative indices; empty tails; alias calls and table/argument expansion | At most 16 total arguments; numeric-string index coercion and `...` syntax remain unsupported, so this API is **partial** |
| `camera`, `cursor` | Setters already existed; the missing work was returning the old position | Both coordinates now survive direct and first-class calls; cursor color is preserved | No independent PICO-8 reference comparison |
| `palt` | Transparency was already an array of 16 flags | Added the one-argument bitmask, reset/boolean updates and previous-mask return; tests verify both bit orders using sprite pixels | No independent reference comparison |
| `clip` | Rectangle clipping already existed | Added intersection mode and four previous-rectangle results; tests cover bounds, disjoint rectangles and empty clips | Integer screen clipping only; exact off-screen/fractional state fidelity remains unverified, so status stays **partial** |
| `sset`, `line` | Sprite writes and line rasterization already existed | `sset(x,y)` uses the draw color; line continuation, pen reset and color-only forms work | Remapped sprite memory, fill patterns and exact line raster fidelity remain incomplete; both APIs stay **partial** |
| `assert` results | The same native result bridge exposed an existing scalar-return omission | Successful assertions return all supplied values, including nil | Existing 16-argument/result limit and unverified error-format differences remain |

The original [`api_overloads.p8`](../converter/tests/api_overloads.p8) fixture asserts results and pixels in generated C, then runs for 2,500 frames with ASan/UBSan. Seven additional negative cases verify controlled failures. The existing semantic/closure tests and PNG roundtrip also pass. An RP2040 UF2 of the new fixture builds with the stack-budget checks enabled; it has **not** been flashed or device-confirmed.

## Smallest remaining candidates

These are suggested next tasks, not completed features or time estimates.

| Priority | Incomplete part | Relative scope and useful next check |
| --- | --- | --- |
| 1 | `select` numeric-string indices | Small: reuse a strict fixed-point numeric parser; test valid decimal/hex selectors, malformed strings, truncation and range errors without loosening other APIs |
| 2 | `ord(str,index,count)` | Small: reuse native multiple results; first establish PICO-8's exact end-of-string/count behavior, then test zero/negative/oversized counts and boundary positions |
| 3 | Raw `.rom` cartridge input | Small to medium: reuse the vendored decoder and existing cartridge pipeline; test original generated ROMs, headers, malformed/short inputs and source/asset identity |
| 4 | `tostr` hex flags / `tonum` numeric forms | Medium: shared parsing/formatting work, but wrapping and fractional bits need exact fixtures rather than a floating-point approximation |
| 5 | Rounded rectangles / ellipses | Medium: reuse pixel and clipping primitives; reference raster fixtures and fill-pattern behavior are needed before calling them complete |

Audio, coroutines, metatables, full memory/register emulation and persistent saves are larger dependencies. A stub or a successful build would not close their acceptance gates.

## Semantics used

The [official PICO-8 manual](https://www.lexaloffle.com/dl/docs/pico-8_manual.html) documents these drawing overloads and advanced Lua functions. The [official 0.2 release notes](https://www.lexaloffle.com/bbs/?tid=37695) establish previous-state returns, default `sset` color and line pen reset. The [Lua 5.2 base-library source](https://www.lua.org/source/5.2/lbaselib.c.html) specifies raw access, `select` range handling and assertion returns. These informed implementation and hand-authored assertions; they are **not an independent reference execution suite**.
