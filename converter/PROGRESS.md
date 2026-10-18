# Pocket AOT implementation dashboard

Audited snapshot: 2026-10-16. Rubric: pocket-aot-capabilities-v1.

Open Pocket Arcade and choose **PROGRESS** for the interactive subsystem tables, API matrix and game results.

**Functional API names: 105/108 (97.2%).** The inventory contains 42 supported, 63 partial and 3 missing entries. A functional name may implement only a subset of the official behavior. Silent audio stubs remain missing.

**Observed in current public host regression: 87/108 API names** across 42 completed sanitizer scenarios. This records executed entry points and iterator steps, including controlled expected-error cases; it does not prove correctness of every overload.

**Historical frozen game sample: 1/20 native builds; 1/20 display/control confirmations.** The original cartridges and their firmware are not bundled. Independent PICO-8 reference conformance remains unmeasured.

| API subsystem | Supported | Partial | Missing | Total | Exercised in host scenarios |
| --- | ---: | ---: | ---: | ---: | ---: |
| system | 4 | 6 | 2 | 12 | 5 |
| graphics | 6 | 20 | 0 | 26 | 23 |
| tables | 7 | 0 | 0 | 7 | 6 |
| input | 2 | 0 | 0 | 2 | 1 |
| audio | 0 | 2 | 0 | 2 | 2 |
| map | 0 | 4 | 0 | 4 | 3 |
| memory | 0 | 9 | 1 | 10 | 8 |
| math | 16 | 6 | 0 | 22 | 16 |
| menu | 0 | 1 | 0 | 1 | 1 |
| strings | 0 | 7 | 0 | 7 | 7 |
| persistence | 0 | 3 | 0 | 3 | 3 |
| gpio | 0 | 1 | 0 | 1 | 1 |
| advanced_lua | 7 | 4 | 0 | 11 | 11 |

Baseline: [PICO-8 v0.2.7 manual](https://www.lexaloffle.com/dl/docs/pico-8_manual.html). Documented cartridge-runtime behavior for desktop ahead-of-time conversion to the current RP2040/Waveshare target, under explicit resource limits. Host/editor authoring tools and Splore are excluded. Runtime audio state remains in scope even without a speaker. A 100% checklist score does not imply all cartridges fit the RP2040 or run at their target frame rate.

## Entire translation pipeline

These counts include language, input, build and validation capabilities as well as the API entries above.

| Component | Supported | Partial | Missing | Total | Main remaining work |
| --- | ---: | ---: | ---: | ---: | --- |
| Cartridge formats and assets | 7 | 1 | 0 | 8 | Raw ROM input; systematic version/malformed-cartridge coverage |
| Source language and frontend | 9 | 1 | 1 | 11 | Varargs, goto/labels, external includes; stricter identifier behavior |
| Values, functions and native semantics | 9 | 5 | 0 | 14 | Custom iterators, coroutines, metatables; numeric/string/GC edge behavior |
| Graphics, maps and text | 6 | 24 | 0 | 30 | Palette modes, drawing overloads, text controls, shapes and textured lines |
| Math, tables, strings and advanced Lua APIs | 30 | 17 | 0 | 47 | Advanced Lua APIs and remaining overload/edge behavior |
| Input and program lifecycle | 6 | 2 | 0 | 8 | Other players, configurable repeat, keyboard/mouse and timing fidelity |
| Memory, saves and system services | 4 | 20 | 3 | 27 | peek/poke and memory mapping, persistent saves, menu callbacks, cart services |
| Audio behavior and observable timing | 3 | 3 | 2 | 8 | Stateful sequencing, sound ROM import, synthesis and audio state queries |
| RP2040 build and resource handling | 6 | 4 | 0 | 10 | Peak resource prediction, optimization and sustained performance qualification |
| Correctness and compatibility validation | 4 | 1 | 5 | 10 | Independent PICO-8 comparisons, golden frames/traces and Pico stress tests |

## Secondary planning index

The original weighted capability index is **about 73%**. It is an engineering planning estimate. It does not measure remaining time or the probability that an unfamiliar game works.

| Component | Weight | Weighted-rubric score |
| --- | ---: | ---: |
| Cartridge formats and assets | 6% | 93.8% |
| Source language and frontend | 12% | 86.4% |
| Values, functions and native semantics | 18% | 82.1% |
| Graphics, maps and text | 18% | 60.0% |
| Math, tables, strings and advanced Lua APIs | 8% | 81.9% |
| Input and program lifecycle | 8% | 87.5% |
| Memory, saves and system services | 8% | 51.9% |
| Audio behavior and observable timing | 8% | 56.2% |
| RP2040 build and resource handling | 8% | 80.0% |
| Correctness and compatibility validation | 6% | 45.0% |

The frozen checklist contains 173 core acceptance entries. Each component has a fixed importance weight; entries share that component's weight equally. Supported earns 1, partial earns 0.5, and missing earns 0. Silent no-ops earn 0. Supported means the described capability exists within its stated limits with no specific omission identified by this audit; it does not mean complete reference equivalence.

Component score = 100 × sum(entry credits) / number of component entries. Overall score = sum(component weight × component score / 100).

Supported entries contribute 52.7 points; partial entries contribute 20.1 more, giving 72.9/100. The weights and half-credit rule are explicit judgment calls, frozen in rubric v1. They are not estimates of equal engineering effort. With quarter-credit versus three-quarter-credit for partial support, the same inventory scores 62.8% versus 82.9%. This is scoring sensitivity, not a confidence interval.

The GUI, thumbnail browsing, import and USB flashing are supporting tooling. Their separate checklist score is 75%; they contribute zero to the translation score. This prevents a finished-looking launcher from hiding missing game behavior.

## Separate outcome and evidence measures

- Historical frozen random sample: 1/20 built native firmware (5%). 1/20 also has owner-confirmed display and controls. This sample result is not universal game compatibility or proof that every scene in a game is correct.
- Canonical API-name presence: 105/108 (97.2%) has some functional implementation. Aliases are counted once and silent audio stubs are excluded. Presence is not full API semantics.
- Independent PICO-8 conformance: **not measured**. Existing assertions, sanitizers and hardware checks are useful evidence but do not compare against a separate PICO-8 reference implementation.
- Public test evidence: None recorded passing Rust tests; native semantic/function/GC fixtures and controlled failures. Historical private evidence includes 20 fixed sampled cartridges and owner confirmation for Dusk Child. The dashboard uses recorded tests and host execution profiles; it does not reflash hardware when viewed or refreshed.

A game needs every capability it uses. One missing memory or save API can reject an otherwise well-supported game, so a roughly half-built capability profile can coexist with a 5% sampled pass rate. Completing the checklist would still not make games exceeding the Pico's RAM, flash or processing capacity fit automatically.

## Next measurable milestones

1. Memory mapping and peek/poke/memcpy/memset: 4 sampled games first stop at memory APIs. The full implementation must preserve draw-state, shared asset and screen-memory behavior.
2. Persistent cartridge data: 5 sampled games first stop at save APIs. Add namespaced saves, reload/reset persistence and tests before crediting cartdata/dget/dset.
3. Custom menu callbacks: 4 sampled games first stop at menuitem. Clearing that diagnostic does not guarantee those games will pass their next blocker.
4. Language extensions: 3 sampled games first stop at goto/labels; 1 at varargs and 1 at coroutine status. Require behavioral fixtures, not syntax-only acceptance.
5. Reference comparisons: establish deterministic language, API and framebuffer results before describing the implementation as faithfully compatible.

## Maintain the score

Run python3 converter/coverage_report.py --check from the repository root to validate the rubric and source snapshot. Run it with --write to regenerate this report and the machine-readable summary. Translator code changes make the core audit stale; UI-only code is tracked separately. Re-audit affected entries, update their evidence/status and snapshot hashes, then recalculate. Do not award progress automatically for added lines or passing test counts. Adding/removing scope or changing weights requires a new rubric version.

Detailed evidence: [full checklist](../compatibility/coverage/CHECKLIST.md), [frozen rubric](../compatibility/coverage/capability-manifest.json), [language audit](../compatibility/coverage/language-audit.json), [API audit](../compatibility/coverage/api-audit.json), [historical sample metadata](../compatibility/top200/results.json).
