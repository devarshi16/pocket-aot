"""Verify that the flashed application starts, leaving it running afterwards."""
from pathlib import Path
import json
import sys
import time
sys.path.insert(0, str(Path(__file__).resolve().parent / 'host_tools'))
port, cart = sys.argv[1:3]
aot = cart.startswith('aot-')
if not aot:
    catalog=json.loads((Path(__file__).resolve().parent.parent/'catalog.json').read_text())
    aot=any(c['id']==cart and 'AOT' in c.get('kind','') for c in catalog['cartridges'])
if aot:
    import serial
    with serial.Serial(port, 115200, timeout=.2) as t:
        t.reset_input_buffer()
        t.write(b's')
        deadline = time.monotonic() + 6
        data = b''
        while time.monotonic() < deadline:
            data += t.read(2048)
            marker = b'AOT_STATUS'
            if marker in data:
                if aot and b'error=0' not in data:
                    raise SystemExit('Converted cartridge reported a runtime error: ' + data.decode(errors='replace'))
                print(data.decode(errors='replace').strip())
                break
        else:
            raise SystemExit('Pico restarted, but the native game did not answer its health check.')
elif cart in ('nomad', 'prism', 'buttons'):
    from mpremote.transport_serial import SerialTransport
    module, marker = {
        'nomad': ('pocket_starship', b'NOMAD_READY'),
        'prism': ('prism_garden', b'PRISM_READY'),
        'buttons': ('button_tester', b'BUTTON_TEST_READY'),
    }[cart]
    t = SerialTransport(port)
    try:
        t.enter_raw_repl(soft_reset=True)
        output = t.exec("import os; assert %r in open('main.py').read(); assert %r in os.listdir(); print('FILES_OK')" %
                        (module, module + '.py'))
        if b'FILES_OK' not in output:
            raise RuntimeError('Application files were not found')
        t.exit_raw_repl()
        t.serial.write(b'\x04')
        deadline = time.monotonic() + 14
        data = b''
        while time.monotonic() < deadline:
            data += t.serial.read(t.serial.in_waiting or 1)
            if marker in data:
                print(marker.decode() + ' / startup verified')
                break
            if b'Traceback' in data:
                raise RuntimeError(data.decode(errors='replace').strip())
        else:
            raise RuntimeError('Application startup was not confirmed: ' + data.decode(errors='replace')[-700:])
    finally:
        t.close()
else:
    # A locally configured image may have its own startup protocol.
    print('USB restart verified; no application-specific health protocol configured.')
