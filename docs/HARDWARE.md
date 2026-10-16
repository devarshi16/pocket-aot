# Hardware target and porting

The currently validated device is an **RP2040 Raspberry Pi Pico with 2 MB flash**, fitted with the **Waveshare Pico-LCD-1.3**. The display is a **240 × 240 ST7789 color LCD**, not an OLED. The module has four face buttons and a five-way stick: nine switches in total.

- [Waveshare module documentation](https://www.waveshare.com/wiki/Pico-LCD-1.3)
- [Raspberry Pi Pico documentation and model comparison](https://www.raspberrypi.com/documentation/microcontrollers/raspberry-pi-pico.html)

Pico W is an RP2040 model. Pico 2 / Pico 2 W use RP2350 and need a separate port; the current UF2 family, linker layout and build target must not be assumed to support them. Wi-Fi is not used.

## Connections

| Signal | GPIO |
| --- | ---: |
| SPI1 SCK | 10 |
| SPI1 MOSI | 11 |
| LCD CS | 9 |
| LCD DC | 8 |
| LCD reset | 12 |
| Backlight PWM | 13 |
| Left / right | 16 / 20 |
| Up / down | 2 / 18 |
| Stick press | 3 |
| A / B / X / Y | 15 / 17 / 19 / 21 |

Buttons are active-low with pull-ups. The framebuffer uses the PICO-8 16-color palette at 128 × 128. The device backend streams scaled RGB565 scanlines over SPI, using the second core for display transfer. Its 40/20/10 MHz settings are requested rates; the achievable rate depends on the peripheral clock. Press Y to select a slower setting if the picture is unstable.

## Controls for converted cartridges

| Physical control | Function |
| --- | --- |
| Stick directions | PICO-8 buttons 0–3 |
| A / B | PICO-8 buttons 4 / 5 |
| X or stick press | Pause / resume |
| Y | Cycle display SPI speed |
| Hold X + Y for 2 seconds | Enter BOOTSEL |

If firmware is unresponsive, hold the Pico's physical BOOTSEL button while connecting USB. Native firmware replaces MicroPython and any installed filesystem. Keep your own backups separately; this repository does not contain a private restoration image.

## USB access

Linux permissions may prevent serial access or BOOTSEL transfer. Inspect [`helpers/install-usb-access.sh`](../helpers/install-usb-access.sh), then run it once if appropriate for your machine:

```sh
sudo bash helpers/install-usb-access.sh
```

Reconnect the Pico after installing the rules. The helper grants the relevant USB/serial access to the existing `plugdev` group. BOOTSEL must also be mounted as an `RPI-RP2` drive for copying. The desktop currently detects Linux mount locations and serial devices; other desktop platforms need adaptation.

## Porting boundaries

| Layer | Reuse on another setup | Work required |
| --- | --- | --- |
| Cartridge reader, lexer, parser, C generation | Yes | Preserve source semantics and add missing language features as needed |
| Bounded C values / GC / API runtime | Mostly | Revisit pool sizes, stack guard and platform-specific assumptions |
| LCD and GPIO backend | Setup-specific | Replace controller initialization, transfer path, scaling and button mapping in `converter/runtime/pico.c` |
| Firmware packaging | RP2040-specific | Change SDK board, memory map, stack accounting, flash limit, UF2 family and flasher validation for another chip |
| Desktop launcher | Linux-specific integration today | Port USB enumeration, BOOTSEL mounting and dependency setup |

Changing a pin constant is not sufficient to qualify a new hardware target. Check colors/orientation, input edges, timing, RAM/stack/flash usage, error handling and BOOTSEL recovery on real hardware.
