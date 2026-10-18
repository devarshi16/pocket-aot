#!/usr/bin/env python3
"""Recalculate the frozen capability rubric; never infer progress from code size."""

from __future__ import annotations

import argparse
from collections import Counter
from decimal import Decimal, ROUND_HALF_UP
import hashlib
import html
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parent.parent
DEST = ROOT / "compatibility/coverage"
LABELS = {"supported": "Supported", "partial": "Partial", "missing": "Missing"}


def read(name):
    return json.loads((DEST / name).read_text())


def percent(value):
    return str(value.quantize(Decimal("0.1"), rounding=ROUND_HALF_UP))


def score(entries, partial=Decimal("0.5")):
    credits = {"supported": Decimal(1), "partial": partial, "missing": Decimal(0)}
    return sum((credits[e["status"]] for e in entries), Decimal(0)) / len(entries) * 100


def load_assessment():
    manifest = read("capability-manifest.json")
    api = read("api-audit.json")
    language = read("language-audit.json")
    entries = {}

    def add(item):
        if item["id"] in entries:
            raise ValueError("Duplicate gate ID: " + item["id"])
        if item["status"] not in LABELS:
            raise ValueError("Unknown status: " + item["status"])
        entries[item["id"]] = item

    for item in language["features"] + manifest["custom_features"]:
        add(dict(item))
    for item in api["apis"]:
        add(dict(id="api." + item["name"], name=item["name"],
                 status=item["capability_status"], acceptance=item["reason"],
                 evidence=item["evidence"], tests=item["selected_regression_tests"],
                 reference_validated=item["reference_validated"]))

    if sum(c["weight"] for c in manifest["categories"]) != 100:
        raise ValueError("Core category weights must sum to 100")
    if manifest["status_credit"] != {"supported": 1, "partial": 0.5, "missing": 0}:
        raise ValueError("Changing the credit rule requires a new calculator/rubric version")
    previous_path = DEST / "progress-summary.json"
    if previous_path.exists():
        previous = json.loads(previous_path.read_text())
        if previous["rubric_version"] == manifest["rubric_version"]:
            structure = lambda groups: [(c["id"], c["weight"], c["members"]) for c in groups]
            if structure(previous["categories"]) != structure(manifest["categories"]):
                raise ValueError("Scope/weights changed; increment the rubric version")
    members = [key for c in manifest["categories"] for key in c["members"]]
    if len(members) != len(set(members)):
        raise ValueError("A gate contributes to multiple core categories")
    if set(members).intersection(manifest["tooling_members"]):
        raise ValueError("Supporting tooling must not inflate the core score")
    if set(members + manifest["tooling_members"]) != set(entries):
        raise ValueError("Unassigned or unknown acceptance entries")
    if {k for k in members if k.startswith("api.")} != {"api." + x["name"] for x in api["apis"]}:
        raise ValueError("Core rubric must account for every frozen API exactly once")
    for name in ["sfx", "music"]:
        if name in api["summary"]["accepted_noops"] and entries["api." + name]["status"] != "missing":
            raise ValueError("Silent audio no-ops cannot earn functional credit")

    stale = []
    for path, expected in manifest["source_hashes"].items():
        file = ROOT / path
        if not file.exists() or hashlib.sha256(file.read_bytes()).hexdigest() != expected:
            stale.append(path)
    for path, expected in api["source_evidence"].items():
        if (not (ROOT / path).exists() or hashlib.sha256((ROOT / path).read_bytes()).hexdigest() != expected) and path not in stale:
            stale.append(path)

    categories = []
    for category in manifest["categories"]:
        gates = [entries[k] for k in category["members"]]
        value = score(gates)
        categories.append(dict(category, score_percent=float(value),
                               weighted_points=float(value * category["weight"] / 100),
                               status_counts=dict(Counter(x["status"] for x in gates))))

    def total(partial):
        return sum((score([entries[k] for k in c["members"]], partial) * c["weight"] / 100
                    for c in manifest["categories"]), Decimal(0))

    sample = json.loads((ROOT / "compatibility/top200/results.json").read_text())
    sample_size = len(sample)
    if sample_size != len({x["rank"] for x in sample}):
        raise ValueError("Sample ranks are duplicated")
    built = sum(x.get("conversion") == "built" for x in sample)
    confirmed = sum(bool(x.get("hardware_confirmed")) for x in sample)
    api_count = len(api["apis"])
    implemented = sum(bool(x["implemented"]) for x in api["apis"])
    blockers = Counter()
    for row in sample:
        if row.get("conversion") == "built":
            continue
        error = row.get("error", "")
        match = re.search(r"unsupported function/API '([^']+)'", error)
        if match:
            blockers[match.group(1)] += 1
        elif "goto" in error:
            blockers["goto/labels"] += 1
        elif "varargs" in error:
            blockers["varargs"] += 1
        else:
            blockers["other"] += 1
    profile_path = DEST / "runtime-api-coverage.json"
    profile = json.loads(profile_path.read_text()) if profile_path.exists() else {}
    profile_hashes = profile.get("source_sha256", {})
    profile_fresh = bool(profile_hashes) and all((ROOT / path).exists() and hashlib.sha256((ROOT/path).read_bytes()).hexdigest()==digest for path,digest in profile_hashes.items())
    hits = profile.get("api_hits", {})
    observed = {row["name"] for row in api["apis"] if row["implemented"] and sum(hits.get(n,0) for n in [row["name"]]+row.get("aliases",[]))>0} if profile_fresh else set()
    api_subsystems = []
    for category in api["categories"]:
        records = [row for row in api["apis"] if row["category"]==category]
        api_subsystems.append(dict(name=category, total=len(records),status_counts=dict(Counter(row["capability_status"]for row in records)),exercised=sum(row["name"]in observed for row in records)))
    historical = read("historical-runtime-api-coverage.json")
    historical_observed = {row["name"] for row in api["apis"] if row["implemented"] and sum(historical.get("api_hits",{}).get(n,0) for n in [row["name"]]+row.get("aliases",[]))>0}
    rust_log=ROOT/"logs/public-rust-tests.txt"
    if not rust_log.exists():rust_log=ROOT/"logs/dashboard-rust-tests.txt"
    if not rust_log.exists():rust_log=ROOT/"logs/rust-retry-tests.txt"
    rust_tests=sum(int(n)for n in re.findall(r"test result: ok\. (\d+) passed",rust_log.read_text())) if rust_log.exists() else None
    summary = dict(
        rubric_version=manifest["rubric_version"], assessed_date=manifest["assessed_date"],
        label="Secondary weighted planning index; implementation and regression counts are primary",
        core_capability_score_percent=float(total(Decimal("0.5"))),
        supported_only_points=float(total(Decimal(0))),
        partial_credit_points=float(total(Decimal("0.5")) - total(Decimal(0))),
        sensitivity_partial_credit_25_percent=float(total(Decimal("0.25"))),
        sensitivity_partial_credit_75_percent=float(total(Decimal("0.75"))),
        core_gate_count=len(members), status_counts=dict(Counter(entries[k]["status"] for k in members)),
        supporting_tooling_score_percent=float(score([entries[k] for k in manifest["tooling_members"]])),
        functional_api_names=implemented, canonical_api_count=api_count,
        api_status_counts=dict(Counter(row["capability_status"]for row in api["apis"])),
        api_subsystems=api_subsystems, regression_profile_present=bool(profile),regression_profile_fresh=profile_fresh,
        regression_exercised_api_names=len(observed),regression_scenario_count=len(profile.get("scenarios",[])),
        regression_suite=profile.get("suite", "unknown"),
        historical_regression_exercised_api_names=len(historical_observed),
        historical_regression_scenario_count=len(historical.get("scenarios",[])),
        historical_evidence_date="2026-10-16",
        historical_scope="Private pre-publication suite; external cartridges are not redistributed and its source hashes are retained unchanged.",
        recorded_rust_tests=rust_tests,
        functional_api_name_presence_percent=implemented / api_count * 100,
        independent_reference_conformance="not measured; no independent PICO-8 oracle suite",
        sample_size=sample_size, sample_built=built, sample_owner_confirmed=confirmed,
        sample_build_rate_percent=built / sample_size * 100,
        sample_confirmation_rate_percent=confirmed / sample_size * 100,
        sample_first_blocker_counts=dict(blockers),
        audit_fresh=not stale, changed_since_audit=stale, categories=categories,
        source_hashes=manifest["source_hashes"],
    )
    return manifest, api, entries, summary


def make_overview(manifest, summary):
    total = Decimal(str(summary["core_capability_score_percent"]))
    blockers = summary["sample_first_blocker_counts"]
    memory_stops = sum(blockers.get(n, 0) for n in ("peek", "poke", "peek2", "poke2", "peek4", "poke4", "memcpy", "memset", "reload", "cstore"))
    save_stops = sum(blockers.get(n, 0) for n in ("cartdata", "dget", "dset"))
    rows = [
        "# Pocket AOT implementation dashboard",
        "",
        f"Audited snapshot: {manifest['assessed_date']}. Rubric: {manifest['rubric_version']}.",
        "",
        "Open Pocket Arcade and choose **PROGRESS** for the interactive subsystem tables, API matrix and game results.",
        "",
        f"**Functional API names: {summary['functional_api_names']}/{summary['canonical_api_count']} ({summary['functional_api_name_presence_percent']:.1f}%).** "
        f"The inventory contains {summary['api_status_counts'].get('supported',0)} supported, {summary['api_status_counts'].get('partial',0)} partial and {summary['api_status_counts'].get('missing',0)} missing entries. "
        "A functional name may implement only a subset of the official behavior. Silent audio stubs remain missing.",
        "",
        (f"**Observed in current public host regression: {summary['regression_exercised_api_names']}/{summary['canonical_api_count']} API names** across {summary['regression_scenario_count']} completed sanitizer scenarios. "
        "This records executed entry points and iterator steps, including controlled expected-error cases; it does not prove correctness of every overload."
        if summary['regression_profile_fresh'] else "**Host execution coverage: unavailable or stale.** Re-run converter/test_pipeline.py to collect current execution evidence."),
        "",
        f"**Historical frozen game sample: {summary['sample_built']}/{summary['sample_size']} native builds; {summary['sample_owner_confirmed']}/{summary['sample_size']} display/control confirmations.** "
        "The original cartridges and their firmware are not bundled. Independent PICO-8 reference conformance remains unmeasured.",
        "",
        "| API subsystem | Supported | Partial | Missing | Total | Exercised in host scenarios |",
        "| --- | ---: | ---: | ---: | ---: | ---: |",
    ]
    for group in summary['api_subsystems']:
        counts=group['status_counts'];observed=str(group['exercised']) if summary['regression_profile_fresh'] else 'Unmeasured'
        rows.append(f"| {group['name']} | {counts.get('supported',0)} | {counts.get('partial',0)} | {counts.get('missing',0)} | {group['total']} | {observed} |")
    rows += ["",f"Baseline: [{manifest['baseline']}]({manifest['baseline_url']}). {manifest['scope']}","",
        "## Entire translation pipeline", "",
        "These counts include language, input, build and validation capabilities as well as the API entries above.","",
        "| Component | Supported | Partial | Missing | Total | Main remaining work |",
        "| --- | ---: | ---: | ---: | ---: | --- |"]
    remaining = {
        "formats": "Raw ROM input; systematic version/malformed-cartridge coverage",
        "frontend": "Varargs, goto/labels, external includes; stricter identifier behavior",
        "semantics": "Custom iterators, coroutines, metatables; numeric/string/GC edge behavior",
        "graphics": "Palette modes, drawing overloads, text controls, shapes and textured lines",
        "libraries": "Advanced Lua APIs and remaining overload/edge behavior",
        "input": "Other players, configurable repeat, keyboard/mouse and timing fidelity",
        "services": "peek/poke and memory mapping, persistent saves, menu callbacks, cart services",
        "audio": "Stateful sequencing, sound ROM import, synthesis and audio state queries",
        "target": "Peak resource prediction, optimization and sustained performance qualification",
        "validation": "Independent PICO-8 comparisons, golden frames/traces and Pico stress tests",
    }
    for category in summary["categories"]:
        c=category['status_counts']
        rows.append(f"| {category['name']} | {c.get('supported',0)} | {c.get('partial',0)} | {c.get('missing',0)} | {sum(c.values())} | {remaining[category['id']]} |")
    rows += ["", "## Secondary planning index", "",
        f"The original weighted capability index is **about {total.quantize(Decimal(1), rounding=ROUND_HALF_UP)}%**. "
        "It is an engineering planning estimate. It does not measure remaining time or the probability that an unfamiliar game works.", "",
        "| Component | Weight | Weighted-rubric score |", "| --- | ---: | ---: |"]
    for category in summary['categories']:
        rows.append(f"| {category['name']} | {category['weight']}% | {category['score_percent']:.1f}% |")
    rows += ["",
        f"The frozen checklist contains {summary['core_gate_count']} core acceptance entries. "
        "Each component has a fixed importance weight; entries share that component's weight equally. "
        "Supported earns 1, partial earns 0.5, and missing earns 0. Silent no-ops earn 0. "
        "Supported means the described capability exists within its stated limits with no specific omission "
        "identified by this audit; it does not mean complete reference equivalence.",
        "",
        "Component score = 100 × sum(entry credits) / number of component entries. "
        "Overall score = sum(component weight × component score / 100).",
        "",
        f"Supported entries contribute {summary['supported_only_points']:.1f} points; partial entries "
        f"contribute {summary['partial_credit_points']:.1f} more, giving {summary['core_capability_score_percent']:.1f}/100. "
        "The weights and half-credit rule are explicit judgment calls, frozen in rubric v1. "
        "They are not estimates of equal engineering effort. With quarter-credit versus three-quarter-credit "
        f"for partial support, the same inventory scores {summary['sensitivity_partial_credit_25_percent']:.1f}% "
        f"versus {summary['sensitivity_partial_credit_75_percent']:.1f}%. This is scoring sensitivity, not a confidence interval.",
        "",
        "The GUI, thumbnail browsing, import and USB flashing are supporting tooling. Their separate checklist "
        f"score is {summary['supporting_tooling_score_percent']:.0f}%; they contribute zero to the translation score. "
        "This prevents a finished-looking launcher from hiding missing game behavior.",
        "",
        "## Separate outcome and evidence measures",
        "",
        f"- Historical frozen random sample: {summary['sample_built']}/{summary['sample_size']} built native firmware "
        f"({summary['sample_build_rate_percent']:.0f}%). {summary['sample_owner_confirmed']}/{summary['sample_size']} "
        "also has owner-confirmed display and controls. This sample result is not universal game compatibility "
        "or proof that every scene in a game is correct.",
        f"- Canonical API-name presence: {summary['functional_api_names']}/{summary['canonical_api_count']} "
        f"({summary['functional_api_name_presence_percent']:.1f}%) has some functional implementation. "
        "Aliases are counted once and silent audio stubs are excluded. Presence is not full API semantics.",
        "- Independent PICO-8 conformance: **not measured**. Existing assertions, sanitizers and hardware checks "
        "are useful evidence but do not compare against a separate PICO-8 reference implementation.",
        f"- Public test evidence: {summary['recorded_rust_tests']} recorded passing Rust tests; native semantic/function/GC fixtures and controlled failures. "
        "Historical private evidence includes 20 fixed sampled cartridges and owner confirmation for Dusk Child. "
        "The dashboard uses recorded tests and host execution profiles; it does not reflash hardware when viewed or refreshed.",
        "",
        "A game needs every capability it uses. One missing memory or save API can reject an otherwise "
        "well-supported game, so a roughly half-built capability profile can coexist with a 5% sampled pass rate. "
        "Completing the checklist would still not make games exceeding the Pico's RAM, flash or processing "
        "capacity fit automatically.",
        "",
        "## Next measurable milestones",
        "",
        f"1. Memory mapping and peek/poke/memcpy/memset: {memory_stops} sampled games first stop at memory APIs. "
        "The full implementation must preserve draw-state, shared asset and screen-memory behavior.",
        f"2. Persistent cartridge data: {save_stops} sampled games first stop at save APIs. Add namespaced saves, "
        "reload/reset persistence and tests before crediting cartdata/dget/dset.",
        f"3. Custom menu callbacks: {blockers.get('menuitem', 0)} sampled games first stop at menuitem. Clearing that diagnostic "
        "does not guarantee those games will pass their next blocker.",
        f"4. Language extensions: {blockers.get('goto/labels', 0)} sampled games first stop at goto/labels; "
        f"{blockers.get('varargs', 0)} at varargs and {blockers.get('costatus', 0)} at coroutine status. "
        "Require behavioral fixtures, not syntax-only acceptance.",
        "5. Reference comparisons: establish deterministic language, API and framebuffer results before "
        "describing the implementation as faithfully compatible.",
        "",
        "## Maintain the score",
        "",
        "Run python3 converter/coverage_report.py --check from the repository root to validate the rubric and "
        "source snapshot. Run it with --write to regenerate this report and the machine-readable summary. "
        "Translator code changes make the core audit stale; UI-only code is tracked separately. Re-audit affected entries, update their evidence/status and "
        "snapshot hashes, then recalculate. Do not award progress automatically for added lines or passing "
        "test counts. Adding/removing scope or changing weights requires a new rubric version.",
        "",
        "Detailed evidence: [full checklist](../compatibility/coverage/CHECKLIST.md), "
        "[frozen rubric](../compatibility/coverage/capability-manifest.json), "
        "[language audit](../compatibility/coverage/language-audit.json), "
        "[API audit](../compatibility/coverage/api-audit.json), "
        "[historical sample metadata](../compatibility/top200/results.json).",
        "",
    ]
    return "\n".join(rows)


def make_checklist(manifest, entries, summary):
    rows = ["# Frozen capability checklist", "", manifest["status_policy"], "",
            "Historical external-fixture paths in evidence are provenance pointers; those games, source and firmware are not bundled. "
            "These are implementation/acceptance entries, not equal-sized work estimates. "
            "No entry currently has independent PICO-8 reference-equivalence evidence.", ""]
    groups = list(manifest["categories"]) + [dict(name="Supporting tooling (excluded from core score)",
                                                  members=manifest["tooling_members"])]
    for group in groups:
        rows += ["## " + group["name"], "", "| ID / capability | Status | Acceptance or known limitation | Evidence |",
                 "| --- | --- | --- | --- |"]
        for key in group["members"]:
            entry = entries[key]
            detail = entry.get("acceptance", entry.get("limits", ""))
            evidence = "; ".join(entry.get("evidence", []) + entry.get("tests", []))
            cells = [key + " — " + entry["name"], LABELS[entry["status"]], detail, evidence]
            rows.append("| " + " | ".join(str(x).replace("|", "\\|").replace("\n", " ") for x in cells) + " |")
        rows.append("")
    return "\n".join(rows)


def make_status(manifest, summary):
    """Public, source-derived counterpart to the GUI dashboard."""
    count = summary["api_status_counts"]
    fresh = summary["regression_profile_fresh"]
    execution = str(summary["regression_exercised_api_names"]) if fresh else "Unmeasured (profile stale)"
    rows = [
        "# Status", "",
        f"Snapshot: **{manifest['assessed_date']}** · rubric `{manifest['rubric_version']}`.", "",
        "![Implementation status](status.svg)", "",
        "Inspired by the subsystem reporting style of [AnyPS5](https://boykopovar.github.io/AnyPS5/progress.html). "
        "These numbers measure separate things; none is a universal game-compatibility percentage.", "",
        "| Measure | Result | Meaning |", "| --- | ---: | --- |",
        f"| Functional API names | {summary['functional_api_names']}/{summary['canonical_api_count']} ({summary['functional_api_name_presence_percent']:.1f}%) | Some implemented behavior, including partial support |",
        f"| API inventory | {count.get('supported',0)} supported · {count.get('partial',0)} partial · {count.get('missing',0)} missing | Fixed set of canonical names; aliases counted once |",
        f"| Current public host execution | {execution}/{summary['canonical_api_count']} | Entry points reached in {summary['regression_scenario_count']} completed original-fixture sanitizer scenarios |",
        f"| Historical private host execution | {summary['historical_regression_exercised_api_names']}/{summary['canonical_api_count']} | Earlier {summary['historical_regression_scenario_count']}-scenario run, including external cartridges that are not bundled |",
        f"| Historical random game sample | {summary['sample_built']}/{summary['sample_size']} built; {summary['sample_owner_confirmed']}/{summary['sample_size']} owner-confirmed | Earlier RP2040/Waveshare display and controls check; not a new public-suite result |",
        "| Independent PICO-8 reference conformance | **Unmeasured** | No independent oracle, framebuffer or audio differential suite |",
        f"| Secondary weighted planning index | {summary['core_capability_score_percent']:.1f}% | Judgment-based rubric; not remaining time or compatibility |", "",
        "## Low-hanging work", "",
        "Three small-gap batches have added table helpers, drawing overloads, numeric/string behavior and raw ROM input. "
        "The latest adds draw-palette state/table/reset support, leading-dot radix literals and bitwise function defaults. "
        "These subfeature changes do not on their own earn an additional whole-gate credit. "
        "See [completed work and remaining small gaps](SMALL-GAPS.md) for evidence and scope.", "",
        "| Next candidate | Work still needed |", "| --- | --- |",
        "| `ord` boundary/count cases | Establish zero/negative counts and overruns with a trustworthy reference |",
        "| Decimal `tonum` fidelity | Exact fixed-point wrapping, rounding and exponent parsing |",
        "| Drawing-state completion | Verify table/reset returns and fractional clipping; display/secondary palettes need backend/fill-pattern work |",
        "| Rounded rectangles / ellipses | Reuse drawing primitives with reference raster and fill-pattern fixtures |", "",
        "Audio, coroutines, metatables, complete memory emulation and persistent saves require larger changes. "
        "These priorities are an effort assessment, not measured remaining time.", "",
        "## API coverage", "",
        "Supported means implemented within the stated audit gate, with no specific omission identified. "
        "Partial means a known limitation. Missing includes silent no-ops. Audio stubs receive no functional credit. "
        "Host calls establish that code ran; they do not establish output equivalence or cover every argument and overload.", "",
        "| Subsystem | Supported | Partial | Missing | Total | Public host exercised |",
        "| --- | ---: | ---: | ---: | ---: | ---: |",
    ]
    for group in summary["api_subsystems"]:
        c = group["status_counts"]
        observed = str(group["exercised"]) if fresh else "—"
        rows.append(f"| {group['name']} | {c.get('supported',0)} | {c.get('partial',0)} | {c.get('missing',0)} | {group['total']} | {observed} |")
    rows += ["", "## Whole translation pipeline", "",
             f"The frozen scope has **{summary['core_gate_count']} acceptance gates** across ten components. "
             "The GUI, browsing and flashing checklist is separate and contributes zero to the translation index.", "",
             "| Component | Supported | Partial | Missing | Gates | Weight | Component score |",
             "| --- | ---: | ---: | ---: | ---: | ---: | ---: |"]
    for group in summary["categories"]:
        c = group["status_counts"]
        rows.append(f"| {group['name']} | {c.get('supported',0)} | {c.get('partial',0)} | {c.get('missing',0)} | {sum(c.values())} | {group['weight']}% | {group['score_percent']:.1f}% |")
    rows += ["", "Component score = 100 × (supported + 0.5 × partial) / gates. "
             "The overall index is the weighted sum of component scores. Weights and the half-credit rule are fixed judgment calls. "
             f"Changing partial credit from 0.25 to 0.75 changes the same snapshot from {summary['sensitivity_partial_credit_25_percent']:.1f}% to "
             f"{summary['sensitivity_partial_credit_75_percent']:.1f}%; that is scoring sensitivity, not a confidence interval.", "",
             "## Evidence and reproducibility", "",
             f"The current committed public profile is `{summary['regression_suite']}`. "
             f"Its source hashes are **{'fresh' if fresh else 'stale or unavailable'}**. "
             "The runner profiles only scenarios whose success or expected controlled failure was checked. "
             "Two additional original fixtures check explicit rejection of unsupported APIs. "
             "Separate container tests verify four original ROM forms and eight malformed inputs; each positive form executes for 600 sanitized frames. "
             "A PNG test roundtrips the original semantic fixture through the shared decoder and executes for 600 sanitized frames.", "",
             "```sh", "cargo test --locked", "cargo build --release --locked --bin pocket-aot",
             "python3 converter/test_pipeline.py", "python3 converter/test_png.py", "python3 converter/test_containers.py",
             "python3 converter/coverage_report.py --check", "```", "",
             "The public suite needs no external cartridges, network downloads, Pico SDK or connected board. "
             "The optional `test_png.py --arm-build` adds a UF2 build when the SDK and Arm compiler are installed. "
             "Supply additional local source explicitly with `test_pipeline.py --external-cart /path/to/game.p8`; "
             "this produces a separately labeled profile and never downloads or flashes it.", "",
             "The earlier private profile is kept byte-for-byte as a dated provenance record. Its external source and firmware are excluded, "
             "so it is not presented as reproducible current public execution coverage. Historical game metadata records "
             "authors, links, source hashes and first blockers without redistributing cartridges. The 1/20 result does not "
             "mean every scene, performance target or behavior was verified.", "",
             "- [Current public execution profile](../compatibility/coverage/runtime-api-coverage.json)",
             "- [Historical private execution profile](../compatibility/coverage/historical-runtime-api-coverage.json)",
             "- [API limits and evidence](../compatibility/coverage/api-audit.json)",
             "- [Full 173-gate checklist](../compatibility/coverage/CHECKLIST.md)",
             "- [Frozen rubric](../compatibility/coverage/capability-manifest.json)",
             "- [Historical 20-game metadata](../compatibility/top200/results.json)",
             "- [Detailed planning report](../converter/PROGRESS.md)", "",
             "Open **PROGRESS** in Pocket Arcade for the searchable API matrix and game-result explorer. "
             "Refreshing that view reloads evidence; it does not run tests or flash firmware. "
             "Re-audit affected gates after implementation changes, refresh source hashes, rerun the public suite and use "
             "`python3 converter/coverage_report.py --write` to regenerate this page and graph. "
             "Adding scope or changing weights requires a new rubric version.", ""]
    return "\n".join(rows)


def make_status_svg(manifest, summary):
    """Repository-native SVG: no remote assets, generated from the frozen audit."""
    escape = html.escape
    width, height = 1120, 1090
    c = summary["api_status_counts"]
    current = str(summary["regression_exercised_api_names"]) if summary["regression_profile_fresh"] else "—"
    svg = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img" aria-labelledby="title description">',
           '<title id="title">Pocket AOT implementation status</title>',
           '<desc id="description">Supported, partial and missing implementation gates by subsystem. API implementation, current public execution, historical game outcomes and the weighted planning index are separate measures.</desc>',
           '<defs><linearGradient id="bg" x2="1" y2="1"><stop stop-color="#091827"/><stop offset="1" stop-color="#111322"/></linearGradient></defs>',
           '<rect width="1120" height="1090" rx="22" fill="url(#bg)"/>',
           '<g font-family="ui-monospace, SFMono-Regular, Consolas, monospace">',
           '<path d="M38 36h32v8H46v24h-8z" fill="#55e5c1"/>',
           '<text x="86" y="61" fill="#ecf4fa" font-size="29" font-weight="700">POCKET AOT</text>',
           '<text x="40" y="104" fill="#9ab1c7" font-size="17">PICO-8 → native RP2040 · implementation status</text>',
           f'<text x="1080" y="57" text-anchor="end" fill="#8298b1" font-size="13">{escape(manifest["assessed_date"])}</text>',
           '<path d="M40 126h1040" stroke="#273749"/>']
    metrics = [
        ("FUNCTIONAL API NAMES", f'{summary["functional_api_names"]}/{summary["canonical_api_count"]}', "Includes known partial behavior", "#55e5c1"),
        ("PUBLIC HOST EXERCISED", f'{current}/{summary["canonical_api_count"]}', f'{summary["regression_scenario_count"]} original-fixture scenarios', "#78b8ff"),
        ("HISTORICAL DEVICE SAMPLE", f'{summary["sample_owner_confirmed"]}/{summary["sample_size"]}', "Display + controls confirmed", "#c59bff"),
        ("SECONDARY PLANNING INDEX", f'{summary["core_capability_score_percent"]:.1f}%', "Weighted estimate · not compatibility", "#f8c875"),
    ]
    for i, (label, value, detail, color) in enumerate(metrics):
        x = 40 + i * 263
        svg += [f'<rect x="{x}" y="151" width="249" height="137" rx="12" fill="#122436" stroke="#2b4054"/>',
                f'<text x="{x+16}" y="178" fill="#9bb0c6" font-size="11" font-weight="700">{label}</text>',
                f'<text x="{x+16}" y="226" fill="{color}" font-size="36" font-weight="700">{value}</text>',
                f'<text x="{x+16}" y="260" fill="#a7bacb" font-size="10">{detail}</text>']
    svg += ['<text x="40" y="333" fill="#e3edf7" font-size="19" font-weight="700">API INVENTORY</text>',
            f'<text x="1080" y="332" text-anchor="end" fill="#a5b7c8" font-size="14">{c.get("supported",0)} supported · {c.get("partial",0)} partial · {c.get("missing",0)} missing</text>']
    x = 40
    colors = {"supported": "#50d7b1", "partial": "#f3bd68", "missing": "#38485e"}
    for status in ("supported", "partial", "missing"):
        w = c.get(status, 0) / summary["canonical_api_count"] * 1040
        svg.append(f'<rect x="{x:.2f}" y="350" width="{w:.2f}" height="22" fill="{colors[status]}"/>')
        x += w
    svg += ['<text x="40" y="420" fill="#e3edf7" font-size="19" font-weight="700">ENTIRE TRANSLATION PIPELINE</text>',
            '<text x="40" y="445" fill="#91a6bc" font-size="13">173 fixed gates · counts describe implementation, not equal-sized tasks</text>',
            '<text x="865" y="477" text-anchor="end" fill="#91a6bc" font-size="12">SUPPORTED / PARTIAL / MISSING</text>',
            '<text x="1080" y="477" text-anchor="end" fill="#91a6bc" font-size="12">GATES</text>']
    for i, group in enumerate(summary["categories"]):
        y = 511 + i * 43
        count = group["status_counts"]
        total = sum(count.values())
        svg.append(f'<text x="40" y="{y}" fill="#c1d1e0" font-size="13">{escape(group["name"])}</text>')
        x = 493
        for status in ("supported", "partial", "missing"):
            w = count.get(status, 0) / total * 380
            svg.append(f'<rect x="{x:.2f}" y="{y-13}" width="{w:.2f}" height="17" fill="{colors[status]}"/>')
            x += w
        triple = f'{count.get("supported",0)} / {count.get("partial",0)} / {count.get("missing",0)}'
        svg += [f'<text x="1006" y="{y}" text-anchor="end" fill="#a5b7c8" font-size="12">{triple}</text>',
                f'<text x="1080" y="{y}" text-anchor="end" fill="#d1dfeb" font-size="13">{total}</text>']
    svg += ['<path d="M40 951h1040" stroke="#273749"/>',
            '<text x="40" y="983" fill="#d1dfeb" font-size="13">Independent PICO-8 reference conformance: UNMEASURED</text>',
            f'<text x="40" y="1011" fill="#91a6bc" font-size="12">Historical private execution: {summary["historical_regression_exercised_api_names"]}/108 names. External game sources and firmware are not bundled.</text>',
            '<text x="40" y="1037" fill="#91a6bc" font-size="12">Silent audio calls earn zero functional credit. GUI / flashing tooling does not inflate the core index.</text>',
            '<text x="40" y="1063" fill="#6e899e" font-size="11">Generated from versioned audits · docs/STATUS.md contains limits, evidence and methodology</text>',
            '</g></svg>']
    return "\n".join(svg) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--write", action="store_true", help="write Markdown and JSON reports")
    modes.add_argument("--check", action="store_true", help="validate the snapshot without writing files")
    args = parser.parse_args()
    manifest, api, entries, summary = load_assessment()
    if not summary["audit_fresh"]:
        raise SystemExit("Audit is stale; re-audit these source files: " + ", ".join(summary["changed_since_audit"]))
    print(json.dumps({k: v for k, v in summary.items() if k not in ("categories", "source_hashes")}, indent=2))
    if args.write:
        (ROOT / "converter/PROGRESS.md").write_text(make_overview(manifest, summary))
        (DEST / "CHECKLIST.md").write_text(make_checklist(manifest, entries, summary))
        (DEST / "progress-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
        (DEST / "assessment-check.json").write_text(json.dumps({k: v for k, v in summary.items() if k not in ("categories", "source_hashes")}, indent=2) + "\n")
        (ROOT / "docs").mkdir(exist_ok=True)
        (ROOT / "docs/STATUS.md").write_text(make_status(manifest, summary))
        (ROOT / "docs/status.svg").write_text(make_status_svg(manifest, summary))
        print("Wrote converter/PROGRESS.md, docs/STATUS.md, docs/status.svg and compatibility/coverage reports.")


if __name__ == "__main__":
    main()
