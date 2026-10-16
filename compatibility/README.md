# Compatibility evidence

This directory contains implementation audits, test profiles and metadata. It contains no third-party game source, cartridge PNG payloads, generated game C, ROM assets or firmware.

- `coverage/runtime-api-coverage.json` is produced by the public original-fixture suite. Its source hashes determine freshness.
- `coverage/historical-runtime-api-coverage.json` is the unchanged 2026-10-16 pre-publication profile. It exercised external cartridges not bundled here. Its original hashes are provenance, not a claim that those files exist in this repository or that its 48 API names were retested by the smaller public suite.
- `top200/results.json` records the earlier frozen random 20-game experiment. Titles/authors/source links, source hashes, first blockers and the historical 1/20 outcome are retained. Local cartridge, artwork and build paths in this metadata refer to the original private experiment and are intentionally absent.

Detailed interpretation: [Status](../docs/STATUS.md). Tests and counts represent bounded evidence, not universal PICO-8 compatibility or independent reference conformance.
