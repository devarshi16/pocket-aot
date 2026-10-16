# Contributing

Start with the [architecture](docs/ARCHITECTURE.md), [status](docs/STATUS.md), and the known limitations for the feature you want to improve.

For language or API changes, add a small original behavioral fixture with expected results. Include relevant overloads and boundaries; passing an entry point is not enough to claim complete behavior. Run Rust tests and the public sanitizer suite. For device changes, report the board, display, pin map, firmware build, resource use and physical test evidence.

Keep implementation counts, actual execution evidence and game outcomes separate. Update the relevant audited entries only after reviewing their behavior and limits. Source hashes identify the assessed snapshot. Changing checklist scope or weighting requires a new rubric version.

Do not commit other people's games, source-bearing cartridge PNGs, ROMs, generated code/assets from external games, device dumps, firmware, secrets, personal paths or cached toolchains. Use small fixtures you authored. External game reports can include source links, hashes, failure messages and attributed screenshots; users obtain those cartridges separately.

Before a pull request:

```sh
cargo fmt --check
cargo test --locked
python3 converter/test_pipeline.py
python3 converter/test_png.py
python3 converter/coverage_report.py --check
python3 scripts/check_publication.py
```

Describe the trigger, resulting behavior, remaining limits and checks you actually ran. New hardware or compiler versions should be described as unvalidated until measured on the relevant device.
