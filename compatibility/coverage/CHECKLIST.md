# Frozen capability checklist

Supported means implemented within the stated gate with no specific missing behavior found by the audit; it does not mean independently reference-verified. Partial means a known limitation or incomplete acceptance gate. Missing includes silent no-ops. Evidence is local implementation/regression/device evidence unless explicitly reference-validated.

Historical external-fixture paths in evidence are provenance pointers; those games, source and firmware are not bundled. These are implementation/acceptance entries, not equal-sized work estimates. No entry currently has independent PICO-8 reference-equivalence evidence.

## Cartridge formats and assets

| ID / capability | Status | Acceptance or known limitation | Evidence |
| --- | --- | --- | --- |
| formats.text — Read self-contained .p8 source | Supported | Extract Lua and cartridge sections without executing or rewriting source. | src/compiler/mod.rs; src/bin/pocket-aot.rs; converter/test_pipeline.py |
| formats.png — Read compressed .p8.png containers | Supported | Decode the frozen sample and a source/ROM-preserving positive control through pinned Shrinko8. | converter/build.py; converter/vendor/shrinko8; converter/cartridge_input.py; converter/test_png.py; compatibility/top200/results.json; converter/test_containers.py |
| formats.sprites — Import sprite pixels | Supported | Unpack gfx nibbles into native ROM assets with dimension checks. | src/compiler/mod.rs; converter/tests/semantics.p8 |
| formats.mapflags — Import map and sprite flags | Supported | Preserve map/flag bytes and expose them to the native runtime. | src/compiler/mod.rs; converter/runtime/p8.c; converter/tests/semantics.p8; compatibility/top200/019/hardware-check.json |
| formats.shared — Preserve shared sprite/map memory | Supported | mset/mget and sset/sget observe the overlapping lower sprite/map bytes. | converter/runtime/p8.c; converter/tests/semantics.p8 |
| formats.provenance — Source preservation and build provenance | Supported | Record original container and decoded-source hashes separately with generated code and firmware evidence. | converter/build.py; src/bin/pocket-aot.rs; converter/test_png.py; compatibility/top200/results.json |
| formats.validation — Cartridge version and malformed-input coverage | Partial | Validate documented cartridge versions and malformed sections/containers with systematic negative cases. | src/compiler/mod.rs; converter/build.py; converter/cartridge_input.py; converter/test_png.py; converter/test_containers.py |
| formats.rom — Read raw .p8.rom cartridges | Supported | Accept full-size 32768-byte .rom/.p8.rom containers, optionally with a 32-byte trailer, through the shared text/PNG decode path. Code-only tiny ROMs and multi-cart exports are excluded. | converter/cartridge_input.py; converter/build.py; converter/vendor/shrinko8; converter/container-validation.json; converter/test_containers.py |

## Source language and frontend

| ID / capability | Status | Acceptance or known limitation | Evidence |
| --- | --- | --- | --- |
| language.lexing — Comments and quoted/long literals | Supported | Line comments, long bracket comments/literals and documented control escapes are parsed. String value restrictions are counted separately. | src/compiler/parse.rs:50; src/compiler/parse.rs:112; official_pico8_escapes_and_long_literals |
| language.identifiers — ASCII and P8 glyph identifiers | Partial | Glyph identifiers parse and generated C names are encoded. Accepts a broad set of non-ASCII characters rather than validating an exact reference identifier alphabet. | src/compiler/parse.rs:43; src/compiler/emit.rs; functions_generic_loops_methods_and_shorthand_calls |
| language.evaluation — Expression evaluation and short circuit | Supported | Emits temporaries in source order, and/or retain operand values and short-circuit effects. | src/compiler/emit.rs; src/compiler/emit.rs; semantics.p8: side/take/_init; functions.p8: _init |
| language.assignment — Local/global/parallel/compound assignment | Supported | RHS temporaries, indexed destinations and one-time compound indexed evaluation tested. Expression-list limit is counted separately. | src/compiler/emit.rs; src/compiler/emit.rs; semantics.p8: _init; functions.p8: _init |
| language.scope — Lexical blocks, locals and shadowing | Supported | Stacked scope maps and boxed captured locals. Semantic assertion fixtures cover shadowing and module captures. | src/compiler/emit.rs; src/compiler/emit.rs; src/compiler/emit.rs; semantics.p8: _init; functions.p8: _init |
| language.branch — If/elseif/else and PICO-8 short statements | Supported | Normal and same-line conditional/while forms plus ? shorthand print parsed; no exhaustive reference syntax suite. | src/compiler/parse.rs:466; src/compiler/parse.rs:650; src/compiler/emit.rs; short_return_stops_at_its_line; functions_generic_loops_methods_and_shorthand_calls; semantics.p8: _init |
| language.loops — Numeric for, while, repeat and break | Supported | Numeric fixed-point iteration, reverse steps, repeat scope and runtime budget exist. | src/compiler/emit.rs; src/compiler/emit.rs; semantics.p8: _init; loop-budget |
| language.native_codegen — Native C code emission and lifecycle | Supported | Generates C closures, dispatch, _init/_update/_update60/_draw hooks and root enumeration; actual generated code runs under ASAN/UBSan. Not an on-device Lua interpreter. | src/compiler/emit.rs; real_breakout_generates_native_functions; breakout; chase; functions; semantics |
| language.varargs — Variable arguments (...) | Missing | Explicitly rejected in declarations and expressions. | src/compiler/parse.rs:628; src/compiler/parse.rs:742 |
| language.goto — Goto and labels | Missing | Explicitly rejected; current blocker in Picohot, Lab Cat and SUPER World of Goo. | src/compiler/parse.rs:565 |
| language.includes — External include preprocessing | Missing | Comments containing #include are ignored, but real external includes have no resolution or expansion pipeline. | src/compiler/parse.rs:112; src/compiler/mod.rs:87; comments_containing_includes_are_ignored |

## Values, functions and native semantics

| ID / capability | Status | Acceptance or known limitation | Evidence |
| --- | --- | --- | --- |
| language.truth — Nil, booleans, truth and equality | Supported | Tagged nil/number/boolean/string/table/function values; zero is truthy. No separate independent PICO-8 oracle. | converter/runtime/p8.c; converter/runtime/p8.c; converter/runtime/p8.c; semantics.p8: _init; functions.p8: _init |
| language.numeric — 16.16 numeric literals and arithmetic | Partial | Decimal/hex/binary literals and wrapping fixed arithmetic exist; power has a restricted finite range, and library math uses host libm rather than a PICO-8 accuracy oracle. | src/compiler/mod.rs:36; converter/runtime/p8.c; fixed_literals_wrap_hex_and_keep_fractions; lex_binary_and_concat_compound_without_ambiguity; semantics.p8: _init |
| language.strings — String values, concatenation and byte encoding | Partial | P8SCII byte indexing with positive/negative positions and empty out-of-range reads is tested, alongside roundtrips/long strings. NUL bytes remain unsupported, strings max8191 bytes, arena32KB/256slots. Fractional/non-numeric index coercion and glyph fidelity are not fully modeled. | src/compiler/charset.rs:276; src/compiler/emit.rs; converter/runtime/p8.c; every_cartridge_character_maps_back_to_its_byte; functions.p8: _init; runtime_closures.c; converter/tests/numeric_strings.p8 |
| language.functions — Named/anonymous/local functions and first-class calls | Supported | Generated native C functions dispatch through function-tagged values; recursion and callbacks asserted, call depth capped32. | src/compiler/parse.rs:444; src/compiler/parse.rs:586; src/compiler/emit.rs; closures_generate_native_dispatch; functions.p8: _init |
| language.closures — Mutable lexical captures and closure lifetime | Supported | Shared mutable cells, sibling and factory isolation, recursion, closure/table cycles and frame-boundary retention tested. Max64captures/closure,1024cells,256closures,2048capturelinks. | src/compiler/emit.rs; converter/runtime/p8.c; functions.p8: _init/_update; runtime_closures.c |
| language.multi_return — Multiple return expansion and scalar parentheses | Partial | Final-call expansion in assignments/args/return/table tails and native builtin result lists tested, but at most 16 values per expression list/result. Varargs remain a separate missing feature. tonum failure has zero results; ord now transports bounded in-range byte lists. | src/compiler/emit.rs; src/compiler/emit.rs; converter/runtime/p8.c; multiple_returns_parentheses_and_table_tail; functions.p8: _init; converter/tests/api_overloads.p8: native_results; converter/tests/numeric_strings.p8 |
| language.methods — Table functions and colon method syntax | Supported | Method self insertion and receiver-once evaluation tested, including expanded tail arguments. | src/compiler/parse.rs:444; src/compiler/emit.rs; functions_generic_loops_methods_and_shorthand_calls; functions.p8: _init |
| language.tables — Array/record/mixed tables and identity | Supported | Dynamic keys, nil deletion and identity exist within bounded pools; max512tables/3072entries. Metatable semantics separate missing family. | src/compiler/parse.rs:703; converter/runtime/p8.c; semantics.p8: _init; functions.p8: _init |
| language.iteration_builtin — Built-in all/pairs/ipairs and foreach | Partial | Direct generic-for built-in forms supported, including all+del shift handling. Not first-class iterator values, arbitrary iterator triples or custom iterators. | src/compiler/emit.rs; converter/runtime/p8.c; functions.p8: _init; runtime_closures.c |
| language.bitwise — Bitwise operators, shifts and rotations | Partial | & \| ^^ ~ << >> and builtin bit/shift/rotate APIs exist. Operator shifts reject counts outside0..31; full reference edge semantics and all operator spellings not covered. | src/compiler/parse.rs:799; converter/runtime/p8.c; converter/runtime/p8.c; lex_binary_and_concat_compound_without_ambiguity; runtime_closures.c |
| language.gc — Object lifetime and bounded memory management | Partial | Frame-boundary tracing handles cycles and compacts strings. No mid-callback allocation-triggered collection; fixed pools and max graph traversal depth128 can reject valid larger cartridges. | converter/runtime/p8.c; converter/runtime/p8.c; functions.p8: _update; runtime_closures.c; heap-bound |
| language.custom_iterators — Custom generic iteration and iterator values | Missing | Generic-for emitter requires exactly one direct all/pairs/ipairs call with one argument. | src/compiler/emit.rs |
| language.coroutines — Coroutine/yield execution state | Missing | Coroutine APIs are absent and costatus is a live Picowars blocker; generated native stack has no resumable continuation mechanism. | src/compiler/emit.rs |
| language.metatables — Metatables and metamethods | Missing | No metatable storage/dispatch or metatable APIs; table access and arithmetic are direct. | src/compiler/emit.rs; converter/runtime/p8.c |

## Graphics, maps and text

| ID / capability | Status | Acceptance or known limitation | Evidence |
| --- | --- | --- | --- |
| api.clip — clip | Partial | Integer screen clipping/reset, fifth-argument intersection and four previous-rectangle results. Exact off-screen/fractional clipping state fidelity has not been established. | src/compiler/emit.rs:6; converter/runtime/p8.c:271; converter/tests/api_overloads.p8; converter/tests/api_overloads.p8 |
| api.pset — pset | Partial | Core behavior implemented in bounded runtime; complete overload, coercion, numeric edge-case and reference-equivalence coverage unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:245; converter/tests/semantics.p8 |
| api.pget — pget | Partial | Reads framebuffer pixels; custom out-of-bounds register setting and complete memory-mapped draw state absent. | src/compiler/emit.rs:6; converter/runtime/p8.c:246; converter/tests/semantics.p8 |
| api.sget — sget | Partial | Reads sprite pixels; custom out-of-bounds register setting and remapped sprite memory absent. | src/compiler/emit.rs:6; converter/runtime/p8.c:289; converter/tests/semantics.p8 |
| api.sset — sset | Partial | Explicit/default draw-colour sprite-pixel writes are implemented; remapped sprite memory remains absent. | src/compiler/emit.rs:6; converter/runtime/p8.c:290; converter/tests/api_overloads.p8; converter/tests/semantics.p8; converter/tests/api_overloads.p8 |
| api.fget — fget | Supported | Reads sprite flag bitfields/booleans; invalid sprite or bit indices return zero/false rather than wrapping. Independent reference equivalence remains unmeasured. | src/compiler/emit.rs:6; converter/runtime/p8.c:293; converter/tests/numeric_strings.p8 |
| api.fset — fset | Supported | Writes flags/bitfield; bounded subset lacks reference edge-case comparison. | src/compiler/emit.rs:6; converter/runtime/p8.c:294; converter/tests/numeric_strings.p8 |
| api.print — print | Partial | ASCII raster font/basic cursor and newline implemented; extended P8SCII/control commands/custom fonts and exact return metrics incomplete. | src/compiler/emit.rs:6; converter/runtime/p8.c:257; converter/tests/semantics.p8 |
| api.cursor — cursor | Supported | Sets/resets integer cursor position with optional draw colour and returns both previous coordinates; independent reference fidelity unmeasured. | src/compiler/emit.rs:6; converter/runtime/p8.c:269; converter/tests/api_overloads.p8; converter/tests/semantics.p8; converter/tests/api_overloads.p8 |
| api.color — color | Partial | Basic draw colour get/set; extended colour/fill-pattern state unsupported. | src/compiler/emit.rs:6; converter/runtime/p8.c:268; converter/tests/semantics.p8 |
| api.cls — cls | Supported | Clears indexed framebuffer and resets clip/cursor; complete draw-state fidelity unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:244; converter/tests/semantics.p8 |
| api.camera — camera | Supported | Sets/resets integer camera coordinates and returns both previous coordinates through direct or first-class native calls; independent reference fidelity unmeasured. | src/compiler/emit.rs:6; converter/runtime/p8.c:270; converter/tests/api_overloads.p8; converter/tests/api_overloads.p8 |
| api.circ — circ | Partial | Basic integer circle outline; fill patterns/extended draw state absent and raster fidelity unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:256 |
| api.circfill — circfill | Partial | Basic integer filled circle; fill patterns/extended draw state absent and raster fidelity unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:256 |
| api.oval — oval | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.ovalfill — ovalfill | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.line — line | Partial | Explicit endpoints, continuation, no-argument and colour-only pen reset implemented; fill patterns and exact raster fidelity remain incomplete/unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:248; converter/tests/api_overloads.p8; converter/tests/api_overloads.p8 |
| api.rect — rect | Partial | Integer rectangle present; fill patterns/extended draw state absent and raster fidelity unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:247 |
| api.rectfill — rectfill | Partial | Integer filled rectangle present; fill patterns/extended draw state absent and raster fidelity unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:247; converter/tests/semantics.p8 |
| api.rrect — rrect | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.rrectfill — rrectfill | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.pal — pal | Partial | Only numeric draw-palette remapping (palette 0); table, display, secondary palette overloads unsupported. | src/compiler/emit.rs:6; converter/runtime/p8.c:279 |
| api.palt — palt | Supported | Reset, colour/boolean and 16-bit mask overloads with previous-mask return; high bit controls colour 0, low bit colour 15. Independent reference fidelity unmeasured. | src/compiler/emit.rs:6; converter/runtime/p8.c:280; converter/tests/api_overloads.p8; converter/tests/api_overloads.p8 |
| api.spr — spr | Partial | Integer whole-sprite dimensions/flips present; fractional size/fill-pattern/complete draw-state semantics incomplete. | src/compiler/emit.rs:6; converter/runtime/p8.c:287 |
| api.sspr — sspr | Partial | Positive-dimension scaling/flips present; resource limits and raster edge cases differ or unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:288 |
| api.fillp — fillp | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.mget — mget | Partial | Default map plus shared gfx half supported; remapped map/width registers unsupported. | src/compiler/emit.rs:6; converter/runtime/p8.c:291; converter/tests/semantics.p8 |
| api.mset — mset | Partial | Default map plus shared gfx half supported; remapped map/width registers unsupported. | src/compiler/emit.rs:6; converter/runtime/p8.c:292; converter/tests/semantics.p8 |
| api.map — map | Partial | Default map drawing/layer mask present; remapped memory, fill patterns and exact draw state incomplete. | src/compiler/emit.rs:6; converter/runtime/p8.c:295 |
| api.tline — tline | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |

## Math, tables, strings and advanced Lua APIs

| ID / capability | Status | Acceptance or known limitation | Evidence |
| --- | --- | --- | --- |
| api.add — add | Supported | Core behavior implemented in bounded runtime; complete overload, coercion, numeric edge-case and reference-equivalence coverage unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:310; converter/tests/semantics.p8 |
| api.del — del | Supported | Core behavior implemented in bounded runtime; complete overload, coercion, numeric edge-case and reference-equivalence coverage unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:311; converter/tests/semantics.p8; converter/tests/functions.p8 |
| api.deli — deli | Supported | Core behavior implemented in bounded runtime; complete overload, coercion, numeric edge-case and reference-equivalence coverage unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:311; converter/tests/semantics.p8 |
| api.count — count | Supported | Core behavior implemented in bounded runtime; complete overload, coercion, numeric edge-case and reference-equivalence coverage unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:312 |
| api.all — all | Partial | Compiler lowers direct generic-for all(table); first-class iterator values/protocol are unsupported. | src/compiler/emit.rs:390; converter/runtime/p8.c; converter/tests/functions.p8 |
| api.foreach — foreach | Supported | Invokes native function values over the array portion; bounded table pool and mutation edge cases require broader equivalence tests. | src/compiler/emit.rs:6; converter/runtime/p8.c:313; converter/tests/functions.p8 |
| api.pairs — pairs | Partial | Compiler lowers direct generic-for pairs(table); first-class iterator values/protocol are unsupported. | src/compiler/emit.rs:390; converter/runtime/p8.c; converter/tests/functions.p8 |
| api.max — max | Supported | Core behavior implemented in bounded runtime; complete overload, coercion, numeric edge-case and reference-equivalence coverage unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:343 |
| api.min — min | Supported | Core behavior implemented in bounded runtime; complete overload, coercion, numeric edge-case and reference-equivalence coverage unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:343 |
| api.mid — mid | Supported | Core behavior implemented in bounded runtime; complete overload, coercion, numeric edge-case and reference-equivalence coverage unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:344; converter/tests/semantics.p8 |
| api.flr — flr | Supported | Core behavior implemented in bounded runtime; complete overload, coercion, numeric edge-case and reference-equivalence coverage unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:340; converter/tests/semantics.p8 |
| api.ceil — ceil | Supported | Core behavior implemented in bounded runtime; complete overload, coercion, numeric edge-case and reference-equivalence coverage unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:341; converter/tests/semantics.p8 |
| api.cos — cos | Partial | Uses host libm then fixed-point conversion; bit-exact PICO-8 trig results unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:348 |
| api.sin — sin | Partial | Uses host libm then fixed-point conversion; bit-exact PICO-8 trig results unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:347; converter/tests/semantics.p8 |
| api.atan2 — atan2 | Partial | Uses host libm then fixed-point conversion; bit-exact PICO-8 results unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:349 |
| api.sqrt — sqrt | Partial | Uses host libm then fixed-point conversion; bit-exact PICO-8 edge cases unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:346 |
| api.abs — abs | Supported | Core behavior implemented in bounded runtime; complete overload, coercion, numeric edge-case and reference-equivalence coverage unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:342 |
| api.rnd — rnd | Partial | Range/table choices use xorshift; RNG sequence differs from PICO-8 and negative ranges rejected. | src/compiler/emit.rs:6; converter/runtime/p8.c:351 |
| api.srand — srand | Partial | Seeds xorshift; does not reproduce PICO-8 seeded random sequence. | src/compiler/emit.rs:6; converter/runtime/p8.c:350 |
| api.sgn — sgn | Supported | Core behavior implemented in bounded runtime; complete overload, coercion, numeric edge-case and reference-equivalence coverage unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:345 |
| api.band — band | Supported | Core behavior implemented in bounded runtime; complete overload, coercion, numeric edge-case and reference-equivalence coverage unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:331 |
| api.bor — bor | Supported | Core behavior implemented in bounded runtime; complete overload, coercion, numeric edge-case and reference-equivalence coverage unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:331 |
| api.bxor — bxor | Supported | Core behavior implemented in bounded runtime; complete overload, coercion, numeric edge-case and reference-equivalence coverage unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:331 |
| api.bnot — bnot | Supported | Core behavior implemented in bounded runtime; complete overload, coercion, numeric edge-case and reference-equivalence coverage unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:332 |
| api.shl — shl | Supported | Core behavior implemented in bounded runtime; complete overload, coercion, numeric edge-case and reference-equivalence coverage unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:333 |
| api.shr — shr | Supported | Core behavior implemented in bounded runtime; complete overload, coercion, numeric edge-case and reference-equivalence coverage unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:333; converter/tests/runtime_closures.c |
| api.lshr — lshr | Supported | Core behavior implemented in bounded runtime; complete overload, coercion, numeric edge-case and reference-equivalence coverage unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:333; converter/tests/runtime_closures.c |
| api.rotl — rotl | Supported | Core behavior implemented in bounded runtime; complete overload, coercion, numeric edge-case and reference-equivalence coverage unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:333; converter/tests/runtime_closures.c |
| api.rotr — rotr | Supported | Core behavior implemented in bounded runtime; complete overload, coercion, numeric edge-case and reference-equivalence coverage unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:333 |
| api.tostr — tostr | Partial | Numeric hexadecimal fixed/raw and signed raw-integer format flags, no-argument empty string and nil display implemented. Default decimal formatting is approximate; raw object-address formatting is rejected. | src/compiler/emit.rs:6; converter/runtime/p8.c:353; converter/tests/numeric_strings.p8 |
| api.tonum — tonum | Partial | Decimal text/numeric identity, exact fixed-point hexadecimal/binary forms, integer/hex flags and zero-on-failure flag implemented. Invalid conversion publishes zero results. Decimal rounding/overflow and other coercions remain incomplete; numeric identity retains its existing value. | src/compiler/emit.rs:6; converter/runtime/p8.c:369; converter/tests/semantics.p8; converter/tests/numeric_strings.p8 |
| api.chr — chr | Partial | Byte construction present; NUL byte rejected; at most 16 input arguments. | src/compiler/emit.rs:6; converter/runtime/p8.c:391; converter/tests/runtime_closures.c |
| api.ord — ord | Partial | Scalar byte lookup and bounded in-range multi-result lookup up to 16 P8SCII bytes implemented. Zero/negative/oversized counts and multi-result ranges beyond the string are explicitly rejected; embedded-NUL strings remain unsupported. | src/compiler/emit.rs:6; converter/runtime/p8.c:381; converter/tests/runtime_closures.c; converter/tests/functions.p8; converter/tests/numeric_strings.p8 |
| api.sub — sub | Partial | Byte substring basics present; NUL-containing strings unsupported. | src/compiler/emit.rs:6; converter/runtime/p8.c:392; converter/tests/semantics.p8; converter/tests/functions.p8 |
| api.split — split | Partial | Separator/chunk splitting with shared decimal/hex/binary numeric conversion and preserved invalid/empty tokens. NUL-containing strings and exact decimal conversion fidelity remain unsupported/incomplete. | src/compiler/emit.rs:6; converter/runtime/p8.c:314; converter/tests/runtime_closures.c; converter/tests/numeric_strings.p8 |
| api.type — type | Partial | Types supported by native value model recognized; coroutine/thread type unavailable. | src/compiler/emit.rs:6; converter/runtime/p8.c:380; converter/tests/semantics.p8; converter/tests/functions.p8 |
| api.setmetatable — setmetatable | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.getmetatable — getmetatable | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.rawset — rawset | Supported | Raw table mutation, nil-value deletion, non-nil key checks and receiver return. Existing bounded table/value model; no metatable implementation is implied. | src/compiler/emit.rs:6; converter/runtime/p8.c:298; converter/tests/api_overloads.p8; converter/tests/api_overloads.p8 |
| api.rawget — rawget | Supported | Raw table lookup for numeric, string and identity keys, including nil/missing reads. String indexing is a separate operation; rawget still requires a table. Existing bounded table/value model; no metatable implementation is implied. | src/compiler/emit.rs:6; converter/runtime/p8.c:297; converter/tests/api_overloads.p8; converter/tests/api_overloads.p8; converter/tests/numeric_strings.p8 |
| api.rawequal — rawequal | Supported | Type-sensitive raw equality of values, interned strings and table/function identities within the existing bounded value model. | src/compiler/emit.rs:6; converter/runtime/p8.c:299; converter/tests/api_overloads.p8; converter/tests/api_overloads.p8 |
| api.rawlen — rawlen | Supported | Raw length of bounded strings and contiguous table sequences; hash entries are excluded. Sparse-table borders are not uniquely defined by Lua. | src/compiler/emit.rs:6; converter/runtime/p8.c:300; converter/tests/api_overloads.p8; converter/tests/api_overloads.p8 |
| api.select — select | Partial | Numeric/count selectors, numeric-string decimal/hex/binary coercion and positive/negative tails implemented. At most 16 total arguments; vararg syntax remains a separate missing feature. | src/compiler/emit.rs:6; converter/runtime/p8.c:301; converter/tests/api_overloads.p8; converter/tests/api_overloads.p8; converter/tests/numeric_strings.p8 |
| api.cocreate — cocreate | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.coresume — coresume | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.costatus — costatus | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.yield — yield | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |

## Input and program lifecycle

| ID / capability | Status | Acceptance or known limitation | Evidence |
| --- | --- | --- | --- |
| api.btn — btn | Partial | Single-player six-button reads present; players 1..7 rejected. | src/compiler/emit.rs:6; converter/runtime/p8.c:296 |
| api.btnp — btnp | Partial | Single-player presses/repeat present; players 1..7 and configurable repeat registers unsupported. | src/compiler/emit.rs:6; converter/runtime/p8.c:296 |
| input.gpio — LCD-board physical button mapping | Supported | Read and debounce the nine known GPIO inputs and map the six PICO-8 gameplay buttons. | converter/runtime/pico.c; compatibility/top200/019/hardware-check.json |
| input.callbacks — Program init/update/draw callback lifecycle | Supported | Initialize and invoke normal _init/_update/_update60/_draw callbacks, including mutable callback values. | src/compiler/emit.rs; converter/runtime/pico.c; converter/tests/functions.p8; converter/test_pipeline.py |
| input.scheduler — 30/60 Hz scheduling and frame pacing | Partial | Match callback order, catch-up and time progression when the game or display cannot sustain its requested rate. | converter/runtime/pico.c; converter/runtime/p8.c; compatibility/top200/019/hardware-check.json |
| input.multiplayer — Additional controller players | Missing | Return supported controller states for documented player indices instead of rejecting every nonzero player. | converter/runtime/p8.c |
| input.extended — Mouse/keyboard input adaptation | Missing | Adapt the documented mouse/keyboard enable and stat query behavior. | converter/runtime/p8.c |
| input.repeat — Configurable held-button repeat | Missing | Honor documented repeat configuration rather than only the fixed default delay/rate. | converter/runtime/p8.c |

## Memory, saves and system services

| ID / capability | Status | Acceptance or known limitation | Evidence |
| --- | --- | --- | --- |
| api.load — load | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.save — save | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.ls — ls | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.run — run | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.stop — stop | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.assert — assert | Supported | Truth condition/message and all supplied successful return values, including nil, within the 16-argument/result limit. Error-format equivalence has no independent reference suite. | src/compiler/emit.rs:6; converter/runtime/p8.c:396; converter/tests/api_overloads.p8; converter/tests/semantics.p8; converter/tests/functions.p8; converter/tests/api_overloads.p8 |
| api.reset — reset | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.flip — flip | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.printh — printh | Partial | Single-string host console output present; filename/overwrite/desktop/clipboard forms rejected. | src/compiler/emit.rs:6; converter/runtime/p8.c:394 |
| api.time — time | Supported | Frame-count based seconds present (alias t); reset/timing/wrap edge cases unverified. | src/compiler/emit.rs:6; converter/runtime/p8.c:352 |
| api.stat — stat | Partial | Only stat(1), reporting native instruction-budget usage rather than PICO-8 CPU utilization. All other indices fail. | src/compiler/emit.rs:6; converter/runtime/p8.c:395 |
| api.extcmd — extcmd | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.peek — peek | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.poke — poke | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.peek2 — peek2 | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.poke2 — poke2 | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.peek4 — peek4 | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.poke4 — poke4 | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.memcpy — memcpy | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.reload — reload | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.cstore — cstore | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.memset — memset | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.menuitem — menuitem | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.cartdata — cartdata | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.dget — dget | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.dset — dset | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |
| api.serial — serial | Missing | No compiler binding or native runtime implementation. | src/compiler/emit.rs:6 (absent from API bindings) |

## Audio behavior and observable timing

| ID / capability | Status | Acceptance or known limitation | Evidence |
| --- | --- | --- | --- |
| api.sfx — sfx | Missing | Accepted as a silent no-op; no audio synthesis, sequencing or channel state. Zero functional credit. | src/compiler/emit.rs:6; converter/runtime/p8.c:393 |
| api.music — music | Missing | Accepted as a silent no-op; no audio synthesis, sequencing or channel state. Zero functional credit. | src/compiler/emit.rs:6; converter/runtime/p8.c:393 |
| audio.assets — Decode SFX/music ROM assets | Missing | Preserve sound and pattern data from cartridge sections. | src/compiler/mod.rs |
| audio.sequencer — Music/SFX sequencer timing | Missing | Advance patterns/notes/channel state with documented loop and stop behavior. | converter/runtime/p8.c |
| audio.synthesis — Four-channel waveform synthesis | Missing | Produce documented channel sample waveforms and instruments. | converter/runtime/p8.c |
| audio.effects — Audio effects and custom instruments | Missing | Implement note effects, envelopes and custom instruments. | converter/runtime/p8.c |
| audio.queries — Audio state accessible to game code | Missing | Expose current pattern/note/channel state through the relevant stat queries. | converter/runtime/p8.c |
| audio.silentmode — Stateful silent audio mode | Missing | With no attached speaker, still emulate audio sequencing and observable state instead of discarding sound calls. | converter/runtime/p8.c |

## RP2040 build and resource handling

| ID / capability | Status | Acceptance or known limitation | Evidence |
| --- | --- | --- | --- |
| build.arm — Native Arm compilation and linking | Supported | Turn generated C and the bounded support runtime into RP2040 firmware using an explicitly configured Pico SDK and Arm toolchain. | converter/build.py; converter/CMakeLists.txt; converted/aot-cartridge-p8-4ab7f32c/conversion.json |
| build.uf2 — UF2 packaging and flash bounds | Supported | Emit contiguous RP2040-family UF2 blocks and reject firmware beyond this board flash capacity. | converter/build.py; src/flasher.rs; src/flasher.rs |
| build.autonomous — Standalone execution after flashing | Supported | Run the converted cartridge without a desktop game process, Lua VM or network service. | converter/runtime/pico.c; compatibility/top200/019/hardware-check.json |
| build.pools — Bounded allocations and runaway execution | Supported | Enforce explicit allocation, instruction and call-depth limits with controlled errors. | converter/runtime/p8.c; converter/test_pipeline.py; converter/tests/runtime_closures.c |
| build.stack — Separate stacks and stack-use checks | Supported | Reserve nonoverlapping core stacks and reject oversized generated transitions using compiler evidence and runtime headroom checks. | converter/memmap_pocket.ld; converter/build.py; converter/runtime/pico.c; converted/aot-cartridge-p8-4ab7f32c/stack-usage.json |
| build.recovery — Runtime diagnostics and USB recovery | Supported | Report conversion/runtime errors and provide bootloader recovery from the known buttons/USB commands. | converter/runtime/pico.c; helpers/bootloader.py; logs/last-flash.txt |
| build.preflight — Per-cartridge RAM/stack peak prediction | Partial | Predict peak live data and worst native call chain before deployment, rather than rely on bounded failures and linker/static-frame checks alone. | converter/build.py; converter/runtime/p8.c |
| build.performance — Per-cartridge performance qualification | Partial | Measure logic/display timing and qualify the intended frame rate over representative play traces. | converter/runtime/pico.c; converter/runtime/host.c; compatibility/top200/019/hardware-check.json |
| build.optimization — Resource-aware game optimization | Partial | Reduce generated code/heap/stack costs sufficiently for a varied corpus, with measured fit decisions. | src/compiler/emit.rs; converter/runtime/p8.c; converted/aot-cartridge-p8-4ab7f32c/stack-usage.json |
| build.bootstrap — Reproducible clean-machine toolchain bootstrap | Partial | Rebuild from a clean documented Linux installation with pinned complete host dependencies, SDK and compiler. | converter/build.py; Cargo.lock; converter/README.md |

## Correctness and compatibility validation

| ID / capability | Status | Acceptance or known limitation | Evidence |
| --- | --- | --- | --- |
| validation.unit — Compiler/unit regression runner | Supported | Run recorded parser/compiler checks with the pinned Rust lockfile. | src/compiler; Cargo.lock; logs/rust-retry-tests.txt |
| validation.sanitizers — Generated-native semantic and sanitizer runner | Supported | Execute actual generated C fixtures under ASan/UBSan, not just compare generated text. | converter/test_pipeline.py; converter/validation.json |
| validation.failures — Controlled failure regression cases | Supported | Exercise runaway loops and bounded heap failure paths. | converter/test_pipeline.py; converter/validation.json |
| validation.hardware — Hardware frame/input/startup checks | Supported | Capture real-device frames/status and obtain owner confirmation for converted games. | converter/runtime/pico.c; compatibility/top200/019/hardware-check.json |
| validation.corpus — Broad unbiased game corpus coverage | Partial | Maintain a frozen diverse corpus with stage-by-stage results, enough breadth and traces to characterize compatibility. | converter/sample_top200.py; compatibility/top200/results.json |
| validation.numericoracle — PICO-8 numeric/language differential oracle | Missing | Compare generated program outputs with a frozen official PICO-8 reference suite. | converter/tests/semantics.p8; converter/tests/functions.p8 |
| validation.graphicsoracle — Graphics/text/API overload golden results | Missing | Compare deterministic reference framebuffers and return values across graphics overloads. | converter/runtime/host.c |
| validation.audiooracle — Audio-state/sample reference comparisons | Missing | Compare sequencer states and generated samples with reference vectors. | converter/runtime/p8.c |
| validation.gametraces — Deterministic full-game input trace comparisons | Missing | Replay identical recorded input against reference and converted games and compare observable states/frames. | converter/runtime/host.c |
| validation.soak — Real Pico stress and sustained-frame benchmarks | Missing | Automate long-running resource, stack and frame-deadline benchmarks over varied game traces. | converter/runtime/pico.c |

## Supporting tooling (excluded from core score)

| ID / capability | Status | Acceptance or known limitation | Evidence |
| --- | --- | --- | --- |
| tooling.grid — Grid/list cartridge navigation and artwork fallback | Supported | Browse original-demo thumbnails or imported local artwork, search/filter/sort and inspect conversion failures. | src/main.rs; logs/rust-retry-tests.txt |
| tooling.import — Desktop conversion/import workflow | Supported | Import a local cartridge and only add firmware after host checks and building succeed. | src/main.rs; converter/build.py; logs/rust-retry-tests.txt |
| tooling.flash — Verified one-click flashing and startup checks | Supported | Validate firmware/hash/chip, enter BOOTSEL, transfer and verify startup. | src/flasher.rs; helpers/check_startup.py; logs/last-flash.txt |
| tooling.restore — Saved originals and recovery workflow | Supported | Restore an optional user-provided, verified RP2040 recovery image; recovery is hidden when no image is configured. | src/flasher.rs; catalog.json; logs/rust-retry-tests.txt |
| tooling.batch — General GUI batch conversion workflow | Partial | Run batch selection/download/conversion and export detailed results from an easy desktop flow. | converter/sample_top200.py; src/main.rs |
| tooling.install — Portable application installer | Missing | Install toolchain/helper/USB dependencies on a clean machine with a supported installer. | launch.sh; helpers/install-usb-access.sh |
