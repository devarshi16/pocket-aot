"""Enter RP2040 BOOTSEL from Pocket AOT firmware or MicroPython."""
from pathlib import Path
import sys
import time

root = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(root / 'helpers/host_tools'))
port = sys.argv[1]
try:
    if 'MicroPython' in port:
        from mpremote.transport_serial import SerialTransport
        t = SerialTransport(port)
        t.enter_raw_repl(soft_reset=True)
        t.exec_raw_no_follow('import machine; machine.bootloader()')
        try:
            t.close()
        except OSError:
            pass
    else:
        import serial
        with serial.Serial(port, 115200, timeout=.3) as t:
            t.write(b'b')
            t.flush()
            time.sleep(.1)
    print('BOOT_REQUESTED')
except Exception as e:
    if 'Permission' in str(e) or 'access' in str(e):
        raise SystemExit('USB access is blocked. Install the Pico access rules with: sudo bash ' +
                         repr(str(root / 'helpers/install-usb-access.sh')))
    raise SystemExit('Could not enter recovery: ' + str(e) +
                     '. Hold BOOTSEL while reconnecting the Pico, then press FLASH again.')
