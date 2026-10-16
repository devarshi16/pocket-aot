#!/usr/bin/env python3
"""Build the three original MicroPython demos; never flash a connected device.

Requires littlefs-python. The pinned MicroPython image is downloaded to the
ignored .cache directory. It is not vendored or committed with the demos.
"""
from pathlib import Path
from urllib.request import urlopen
import argparse
import hashlib
import json
import struct

ROOT = Path(__file__).resolve().parent
STOCK_URL = 'https://micropython.org/resources/firmware/RPI_PICO-20231005-v1.21.0.uf2'
STOCK_SHA256 = 'a1166281fd87886e5d755e577e8eaf207881e20dac1d76f82161f37133547be3'
MODULES = {'nomad': 'pocket_starship', 'prism': 'prism_garden', 'buttons': 'button_tester'}
FS_OPTIONS = dict(block_size=4096, block_count=352, read_size=256,
                  prog_size=256, lookahead_size=32, disk_version=0x00020000)


def stock_prefix(path):
    data = path.read_bytes()
    if hashlib.sha256(data).hexdigest() != STOCK_SHA256:
        raise ValueError('MicroPython firmware checksum does not match the pinned release.')
    prefix = bytearray(b'\xff' * 0xa0000)
    if len(data) % 512:
        raise ValueError('Invalid MicroPython UF2 length.')
    for offset in range(0, len(data), 512):
        block = data[offset:offset + 512]
        header = struct.unpack('<8I', block[:32])
        address = header[3] - 0x10000000
        if (header[:2] != (0x0a324655, 0x9e5d5157) or header[7] != 0xe48bff56
                or struct.unpack('<I', block[508:])[0] != 0x0ab16f30
                or not 0 < header[4] <= 476
                or address < 0 or address + header[4] > len(prefix)):
            raise ValueError('Unexpected MicroPython UF2 block.')
        prefix[address:address + header[4]] = block[32:32 + header[4]]
    return prefix


def write_uf2(data, path):
    count = (len(data) + 255) // 256
    with path.open('wb') as output:
        for index in range(count):
            payload = data[index * 256:(index + 1) * 256].ljust(256, b'\xff')
            output.write(struct.pack('<8I', 0x0a324655, 0x9e5d5157, 0x2000,
                                     0x10000000 + index * 256, 256, index, count, 0xe48bff56))
            output.write(payload + bytes(220) + struct.pack('<I', 0x0ab16f30))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--stock', type=Path, help='Use a local copy of the pinned MicroPython UF2.')
    args = parser.parse_args()
    try:
        from littlefs import LittleFS, UserContext
    except ImportError:
        raise SystemExit('Install demo build dependencies: python3 -m pip install littlefs-python')
    stock = args.stock or ROOT / '.cache/micropython/RPI_PICO-20231005-v1.21.0.uf2'
    if not stock.exists():
        stock.parent.mkdir(parents=True, exist_ok=True)
        print('Downloading pinned MicroPython 1.21.0...', flush=True)
        data = urlopen(STOCK_URL, timeout=60).read()
        if hashlib.sha256(data).hexdigest() != STOCK_SHA256:
            raise ValueError('Downloaded MicroPython checksum mismatch.')
        temporary = stock.with_suffix('.tmp')
        temporary.write_bytes(data)
        temporary.replace(stock)
    prefix = stock_prefix(stock)
    scripts = {name + '.py': ROOT / 'demos' / (name + '.py') for name in MODULES.values()}
    scripts['waveshare_lcd_driver.py'] = ROOT / 'demos/waveshare_lcd_driver.py'
    destination = ROOT / 'firmware'
    destination.mkdir(exist_ok=True)
    hashes = {}
    for name, module in MODULES.items():
        fs = LittleFS(**FS_OPTIONS)
        for filename, source in scripts.items():
            with fs.open(filename, 'wb') as output:
                output.write(source.read_bytes())
        # The three demos share a filesystem; boot shortcuts can switch apps.
        main_source = '''from machine import Pin
import time
x = Pin(19, Pin.IN, Pin.PULL_UP)
y = Pin(21, Pin.IN, Pin.PULL_UP)
c = Pin(3, Pin.IN, Pin.PULL_UP)
time.sleep_ms(180)
app = %r
if not x.value() and not y.value():
    import machine
    machine.bootloader()
elif not x.value(): app = 'pocket_starship'
elif not y.value(): app = 'button_tester'
elif not c.value(): app = 'prism_garden'
for _ in range(150):
    if x.value() and y.value() and c.value(): break
    time.sleep_ms(10)
__import__(app).run()
''' % module
        with fs.open('main.py', 'w') as output:
            output.write(main_source)
        fs.unmount()
        image = bytearray(fs.context.buffer)
        check = LittleFS(context=UserContext(buffer=image), mount=True, **FS_OPTIONS)
        if sorted(check.listdir('/')) != sorted([*scripts, 'main.py']):
            raise ValueError('Demo filesystem verification failed.')
        with check.open('main.py') as source:
            if source.read() != main_source:
                raise ValueError('Demo startup file verification failed.')
        check.unmount()
        output = destination / (name + '.uf2')
        write_uf2(prefix + image, output)
        hashes[name] = hashlib.sha256(output.read_bytes()).hexdigest()
        print('Built original demo: ' + name, flush=True)
    catalog_path = ROOT / 'catalog.json'
    catalog = json.loads(catalog_path.read_text())
    for cart in catalog['cartridges']:
        if cart['id'] in hashes:
            cart.update(filename='firmware/' + cart['id'] + '.uf2', sha256=hashes[cart['id']], status='ready', reason='')
    temporary = catalog_path.with_suffix('.tmp')
    temporary.write_text(json.dumps(catalog, indent=2) + '\n')
    temporary.replace(catalog_path)
    (destination / 'sha256.json').write_text(json.dumps(hashes, indent=2) + '\n')
    print('Three original demos are ready. Restart the GUI to refresh the shelf.')


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        raise SystemExit(str(error))
