#!/usr/bin/env python3
"""Compile a self-contained .p8 cartridge to native RP2040 UF2, optionally add it to the shelf."""
from pathlib import Path
import argparse, hashlib, json, os, re, shutil, struct, subprocess, sys

ROOT = Path(__file__).resolve().parent.parent


def compiler_binary():
    """Use a normal Cargo build, never a private bundled toolchain or binary."""
    configured = os.environ.get('POCKET_AOT_BIN')
    if configured:
        path = Path(configured).expanduser().resolve()
        if not path.is_file():
            raise RuntimeError('POCKET_AOT_BIN does not point to a compiler executable.')
        return path
    target = Path(os.environ.get('CARGO_TARGET_DIR', str(ROOT / 'target')))
    if not target.is_absolute():
        target = ROOT / target
    compiler = target / 'release/pocket-aot'
    # Cargo handles incremental builds and checks whether sources changed.
    if not shutil.which('cargo'):
        raise RuntimeError('Install Rust/Cargo, then run cargo build --release --locked --bins.')
    command(['cargo', 'build', '--release', '--locked', '--bin', 'pocket-aot'], cwd=ROOT, timeout=600)
    return compiler


def command(args, cwd=None, timeout=120):
    result = subprocess.run([str(x) for x in args], cwd=cwd, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout)
    if result.returncode:
        raise RuntimeError('Command failed: ' + str(args[0]) + '\n' + result.stdout[-3500:])
    return result.stdout


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('source', type=Path)
    parser.add_argument('--title')
    parser.add_argument('--library', action='store_true')
    parser.add_argument('--frames', type=int, default=1200)
    args = parser.parse_args()
    source = args.source.expanduser().resolve()
    if source.suffix.lower() not in ('.p8','.png'):
        raise ValueError('Choose a PICO-8 .p8 or .p8.png cartridge.')
    digest = hashlib.sha256(source.read_bytes()).hexdigest()
    slug = re.sub('[^a-z0-9]+', '-', source.stem.lower()).strip('-')[:32] or 'cartridge'
    cart_id = 'aot-' + slug + '-' + digest[:8]
    dest = ROOT / 'converted' / cart_id
    title = args.title or source.stem.replace('_', ' ').replace('-', ' ').title()
    compiler = compiler_binary()
    input_source = source
    output = ''
    if source.suffix.lower() == '.png':
        dest.mkdir(parents=True, exist_ok=True)
        input_source = dest / 'decoded.p8'
        decoder = ROOT / 'converter/vendor/shrinko8/shrinko8.py'
        print('Decoding the PICO-8 PNG cartridge...', flush=True)
        output += command([sys.executable, decoder, source, input_source], timeout=30)
    print('Translating PICO-8 source to native C...', flush=True)
    output += command([compiler, input_source, dest, title])
    runtime = ROOT / 'converter/runtime'
    sources = [runtime / 'p8.c', runtime / 'host.c', dest / 'game.c', dest / 'assets.c']
    print('Checking the generated game on this computer...', flush=True)
    output += command(['gcc', '-O1', '-g', '-fsanitize=address,undefined', '-fno-omit-frame-pointer',
                       '-I' + str(runtime), *sources, '-lm', '-o', dest / 'host-check'])
    output += command([dest / 'host-check', str(args.frames)], cwd=dest, timeout=45)
    sdk_env = os.environ.get('PICO_SDK_PATH')
    if not sdk_env:
        raise RuntimeError('Set PICO_SDK_PATH to a Raspberry Pi Pico SDK checkout. See converter/README.md.')
    sdk = Path(sdk_env).expanduser().resolve()
    if not (sdk / 'external/pico_sdk_import.cmake').is_file():
        raise RuntimeError('PICO_SDK_PATH is not a Pico SDK checkout.')
    toolchain_env = os.environ.get('PICO_TOOLCHAIN_PATH')
    cmake_toolchain = []
    if toolchain_env:
        cmake_toolchain = ['-DPICO_TOOLCHAIN_PATH=' + str(Path(toolchain_env).expanduser().resolve())]
    elif not shutil.which('arm-none-eabi-gcc'):
        raise RuntimeError('Put arm-none-eabi-gcc on PATH or set PICO_TOOLCHAIN_PATH to the Arm toolchain directory.')
    print('Compiling the RP2040 firmware...', flush=True)
    output += command(['cmake', '-S', ROOT / 'converter', '-B', dest / 'build', '-G', 'Ninja',
                       '-DCMAKE_BUILD_TYPE=Release', '-DCART_DIR=' + str(dest),
                       '-DPICO_SDK_PATH=' + str(sdk), *cmake_toolchain])
    output += command(['cmake', '--build', dest / 'build', '-j4'])
    # All generated calls pass through p8_dispatch. Bound one native transition
    # against the Pico runtime's 4 KB stack headroom check.
    stack_rows=[]
    for usage in (dest/'build').rglob('game.c.su'):
        for line in usage.read_text().splitlines():
            fields=line.split('\t')
            if len(fields)>=3:
                stack_rows.append(dict(function=fields[0].rsplit(':',1)[-1],bytes=int(fields[1]),kind=fields[2]))
    if not stack_rows:raise RuntimeError('ARM stack-usage evidence is missing')
    largest=max((r['bytes'] for r in stack_rows if r['function'].startswith('fn_')),default=0)
    dispatcher=max((r['bytes'] for r in stack_rows if r['function']=='p8_dispatch'),default=0)
    initial=max((r['bytes'] for r in stack_rows if r['function']=='p8_program_init'),default=0)
    if largest+dispatcher>3072 or initial>3072 or any(r['kind']!='static' for r in stack_rows):
        raise RuntimeError('Generated native stack frames exceed the bounded Pico stack budget')
    (dest/'stack-usage.json').write_text(json.dumps(dict(frames=stack_rows,max_call_transition=largest+dispatcher,initial_frame=initial,core0_stack=16384,headroom_guard=4096),indent=2)+'\n')
    # Emit the same contiguous family-tagged UF2 layout accepted by the flasher.
    data = (dest / 'build/pocket_aot.bin').read_bytes()
    if len(data) > 2 * 1024 * 1024:
        raise RuntimeError('Firmware exceeds the Pico flash capacity')
    count = (len(data) + 255) // 256
    firmware = dest / 'cartridge.uf2'
    with firmware.open('wb') as f:
        for i in range(count):
            f.write(struct.pack('<8I', 0x0a324655, 0x9e5d5157, 0x2000, 0x10000000 + 256 * i, 256, i, count, 0xe48bff56))
            f.write(data[256 * i:256 * (i + 1)].ljust(256, b'\xff') + bytes(220) + struct.pack('<I', 0x0ab16f30))
    (dest / 'build.log').write_text(output)
    report = json.loads((dest / 'conversion.json').read_text())
    report.update(original_source=str(source), original_sha256=digest, input_format='p8.png' if source.suffix.lower()=='.png' else 'p8', uf2_sha256=hashlib.sha256(firmware.read_bytes()).hexdigest(), host_frames=args.frames,
                  host_result=next((s for s in output.splitlines() if s.startswith('AOT_HOST_OK')), ''),
                  firmware_bytes=len(data), stack_max_call_transition=largest+dispatcher, stack_initial_frame=initial, hardware_tested=False)
    (dest / 'conversion.json').write_text(json.dumps(report, indent=2) + '\n')
    if args.library:
        # Only update the shelf after translation, sanitizer run and firmware build succeed.
        from PIL import Image
        palette=[(0,0,0),(29,43,83),(126,37,83),(0,135,81),(171,82,54),(95,87,79),(194,195,199),(255,241,232),
                 (255,0,77),(255,163,0),(255,236,39),(0,228,54),(41,173,255),(131,118,156),(255,119,168),(255,204,170)]
        (ROOT/'assets').mkdir(exist_ok=True)
        (ROOT/'firmware').mkdir(exist_ok=True)
        im=Image.new('P',(128,128));im.putpalette([v for c in palette for v in c]+[0]*720)
        preview=dest/'gameplay.idx' if (dest/'gameplay.idx').exists() else dest/'title.idx'
        im.frombytes(preview.read_bytes());im.save(ROOT/'assets'/f'{cart_id}.png')
        shutil.copyfile(firmware, ROOT/'firmware'/f'{cart_id}.uf2')
        path=ROOT/'catalog.json';catalog=json.loads(path.read_text())
        entry=dict(id=cart_id,title=title.upper(),subtitle='Converted from a PICO-8 cartridge.',kind='PICO-8 / EXPERIMENTAL AOT',
                   description='Native firmware generated from this cartridge by Pocket AOT. Conversion and a desktop smoke test passed; check gameplay on your Pico.',
                   controls=['STICK   PICO-8 directions','A / B   PICO-8 O / X buttons','X / CLICK   Pause / resume','Y       Change LCD speed'],
                   filename=f'firmware/{cart_id}.uf2',sha256=report['uf2_sha256'],accent=[124,222,245],
                   cover=f'assets/{cart_id}.png',credits='Cartridge: '+source.name+' / converted with Pocket AOT 0.2')
        entry.update(status='ready',source_file=str(source),thumbnail=f'assets/{cart_id}.png')
        if source.suffix.lower()=='.png':
            shutil.copyfile(source,ROOT/'assets'/f'{cart_id}.p8.png')
            entry['cart_image']=f'assets/{cart_id}.p8.png'
        index=next((i for i,c in enumerate(catalog['cartridges']) if c['id']==cart_id or (c.get('source_file') and (ROOT/c['source_file']).resolve()==source)),None)
        if index is not None:
            old=catalog['cartridges'][index]
            for field in ['id','rank','source_url','thumbnail','cart_image','credits']:
                if field in old:entry[field]=old[field]
        if index is None: catalog['cartridges'].append(entry)
        else: catalog['cartridges'][index]=entry
        temp=path.with_suffix('.tmp');temp.write_text(json.dumps(catalog,indent=2)+'\n');temp.replace(path)
        print('Added to Pocket Arcade: '+entry['id'],flush=True)
    print('UF2 ready: '+str(firmware),flush=True)


if __name__ == '__main__':
    try:
        main()
    except Exception as e:
        raise SystemExit(str(e))
