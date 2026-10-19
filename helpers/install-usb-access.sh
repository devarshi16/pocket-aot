#!/usr/bin/env bash
set -euo pipefail
if [ "$(id -u)" != 0 ]; then
  echo 'Run this script with sudo to install Pico USB access rules.' >&2
  exit 1
fi
cat > /etc/udev/rules.d/70-pocket-arcade.rules <<'RULES'
# Pocket Arcade: Raspberry Pi Pico MicroPython / SDK serial and RP2040 (0003) or RP2350 (000f) BOOTSEL.
SUBSYSTEM=="tty", ATTRS{idVendor}=="2e8a", ATTRS{idProduct}=="000[5a]", GROUP="plugdev", MODE="0660"
SUBSYSTEM=="usb", ATTR{idVendor}=="2e8a", ATTR{idProduct}=="000[3f]", GROUP="plugdev", MODE="0660"
RULES
udevadm control --reload-rules
udevadm trigger --subsystem-match=tty --action=change
udevadm trigger --subsystem-match=usb --action=change
echo 'Pico access rules installed. Accounts already in plugdev can flash after reconnects.'
