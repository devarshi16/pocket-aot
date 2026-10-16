# Notices and provenance

Pocket AOT project code and the original demos are distributed under **GPL-3.0-only**, except for third-party components carrying their own notices. Copyright 2026 Pocket AOT contributors. See [LICENSE](LICENSE).

## Included dependencies and adaptations

| Component | Use | License / notice |
| --- | --- | --- |
| [Waveshare Pico_code](https://github.com/waveshare/Pico_code) | `demos/waveshare_lcd_driver.py`, adapted from `Python/Pico-LCD-1.3/Pico-LCD-1.3.py`; SPI reduced to 10 MHz. The native LCD backend uses the same module's initialization and pin mapping. | GPL v3; [retained license](licenses/Waveshare-GPL-3.0.txt). Modifications identified in the driver. |
| [Shrinko8](https://github.com/thisismypassport/shrinko8) | Trimmed tool sources for cartridge container decoding. Upstream games, test cartridges, Git history, optional font/template assets and site files are omitted. Pin: `9c6f5ba03cdda92f0527145e9ce589f1af24c9cb`. | MIT; [upstream notice](converter/vendor/shrinko8/LICENSE) |
| Press Start 2P | Desktop pixel typeface, `assets/PressStart2P-Regular.ttf` | Copyright 2012 The Press Start 2P Project Authors; [SIL OFL 1.1](assets/OFL.txt) |
| MicroPython / mpremote | Original demo firmware (downloaded at build time) and vendored host helper | [MicroPython MIT](licenses/MicroPython-MIT.txt), [mpremote MIT](licenses/mpremote-MIT.txt) |
| pySerial | Vendored serial helper | [BSD notice](licenses/pyserial-BSD.txt) |
| platformdirs | Vendored host helper dependency | [MIT notice](licenses/platformdirs-MIT.txt) |

Rust crates are fetched by Cargo and retain their respective licenses. The Pico SDK and Arm toolchain are external build dependencies; their source and binaries are not included. The runtime's compact bitmap font was newly authored for this public repository; no ccleste game font is included.

## Original projects and fixtures

NOMAD-01, Prism Garden, Button Lab and the synthetic compiler/runtime fixtures were created in this project workspace. The three interactive demos use MicroPython and code-drawn graphics. Their inclusion does not imply that the PICO-8 compiler generated them.

The native hardware frontend grew out of hardware work in this workspace. The repository does not include the Celeste game engine, levels, sprites, sound or compiled firmware.

## Screenshots and historical game results

The GUI screenshots show a development collection containing external games. They illustrate the application; they are ordinary screen captures, not source-bearing cartridge PNG files. External game content belongs to its respective creators and is not covered by this project's license.

- **Celeste Classic:** Maddy Thorson and Noel Berry; the separately used C source port was by lemon32767. Neither that port nor the game's content is bundled.
- **Pico Hero Breakout** and **Neighborhood Chase** (a shelf title for `basic_game2.p8`): [Coemgen/pico-8](https://github.com/Coemgen/pico-8).
- **Dusk Child** and the other sampled titles: each original creator/source page is linked in [`compatibility/top200/results.json`](compatibility/top200/results.json). The frozen sample was drawn from the [Nerdy Teachers Top 200](https://nerdyteachers.com/PICO-8/Games/Top200/).

The historical regression record also names SCUMM-8 and Halloween as explicitly rejected inputs. Their source and assets are omitted. Historical source hashes are retained for provenance; excluded inputs do not become fresh public validation merely because the measurements are copied here.

PICO-8 is a Lexaloffle product. Raspberry Pi and Waveshare names identify hardware and dependencies. This project is independent of those organizations.
