# Architecture

Pocket AOT compiles supported PICO-8 source to native C ahead of time. The desktop performs decoding, translation, host validation, cross-compilation and packaging. A flashed cartridge contains generated game code plus its compatibility runtime.

| Part | Source | Responsibility |
| --- | --- | --- |
| Cartridge and data sections | `src/compiler/mod.rs`, `charset.rs` | Source, sprites, maps, flags, cartridge character conversion and fixed-point literals |
| Frontend | `src/compiler/parse.rs` | Lexer, AST, PICO-8 syntax and explicit syntax rejection |
| Code generation | `src/compiler/emit.rs` | Native C functions, lexical cells, closures, evaluation order, multiple returns and API checks |
| Runtime | `converter/runtime/p8.c`, `p8.h` | 16.16 values, bounded table/string/closure pools, GC, graphics, input and supported API behavior |
| Host adapter | `converter/runtime/host.c` | Scripted input, frame execution, renders, sanitizer checks and optional API hit profiling |
| Device adapter | `converter/runtime/pico.c` | ST7789 SPI, frame scaling, GPIO polling, two-core display scheduling, timing, USB diagnostics and recovery |
| Firmware build | `converter/build.py`, `CMakeLists.txt`, `memmap_pocket.ld` | Host check, Pico SDK build, stack-usage checks, UF2 packaging and provenance |
| Desktop | `src/main.rs`, `flasher.rs` | Shelf, artwork, import/build jobs, UF2/device validation, flashing and startup checks |
| Status | `progress_data.rs`, `progress_view.rs`, `compatibility/coverage/` | Audited capabilities, independent freshness checks, execution evidence and historical sample outcomes |

The frontend is generic within its supported language/API subset. The device adapter and packaging currently target one RP2040/Waveshare setup. The runtime does not provide a full virtual PICO-8 machine or faithful timing for all cartridges.

## Limits and failures

The runtime bounds tables, entries, closures, lexical cells, captured links and strings. Frame instruction and call-depth limits, a native stack-headroom guard, and compiler stack-usage inspection provide explicit failure paths. These checks establish useful bounds; they do not prove a cartridge meets its target frame rate or matches a PICO-8 reference.

Missing syntax and API names reject conversion. Some supported APIs implement only selected overloads. Audio calls are currently silent stubs and are classified as missing in functional coverage. See the converter guide and API audit for exact limits.

## Evidence

Status is derived from a fixed, versioned checklist. Supported, partial and missing are separate counts. Runtime hit profiling is enabled only in the host tests, and a reached entry point does not establish correct output. Profiles carry source hashes so stale results cannot silently become current coverage.

The original 20-game experiment and eight-scenario development regression used external cartridges. Their metadata is preserved without bundling the cartridges. The public test suite uses original fixtures and produces its own execution evidence. Independent PICO-8 reference conformance remains unmeasured.
