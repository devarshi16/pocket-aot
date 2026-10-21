#!/usr/bin/env python3
"""Private, frozen first-100 benchmark. Downloads and tests are separate commands.

Never flashes, modifies compiler sources, updates the GUI catalog or publishes.
"""
import argparse
from collections import Counter
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timezone
import hashlib
import html
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
from urllib.parse import urljoin
from urllib.request import urlopen

REPO = Path(__file__).resolve().parent.parent
DEFAULT_DEST = REPO / '.cache/top100'
URL = 'https://nerdyteachers.com/PICO-8/Games/Top200/'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(path, value):
    temp = path.with_suffix(path.suffix + '.tmp')
    temp.write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n')
    temp.replace(path)


def freeze(dest):
    selection = dest / 'selection.json'
    if selection.exists():
        return json.loads(selection.read_text())
    text = (dest / 'index.html').read_text(encoding='utf-8')
    pattern = (r'<h5>#(\d+)</h5>.*?<h3>(.*?)</h3>.*?'
               r'href=["\'](https://www\.lexaloffle\.com/bbs/\?pid=\d+)["\']'
               r'.*?<img[^>]+src=["\']([^"\']+)["\'].*?By:\s*(.*?)</a>')
    games = []
    for match in re.finditer(pattern, text, re.S):
        rank, title, page, thumbnail, author = match.groups()
        if int(rank) <= 100:
            clean = lambda s: html.unescape(re.sub('<[^>]*>', '', s)).strip()
            games.append(dict(rank=int(rank), title=clean(title), page=html.unescape(page),
                              thumbnail_url=html.unescape(thumbnail), author=clean(author)))
    assert [game['rank'] for game in games] == list(range(1, 101)), 'Expected exactly ranks 1–100 in order'
    assert len({game['page'] for game in games}) == 100, 'Duplicate game posts'
    result = dict(source=URL, selection_rule='First 100 ranked entries, no substitutions',
                  frozen_at=datetime.now(timezone.utc).isoformat(), index_sha256=sha(dest / 'index.html'), games=games)
    save(selection, result)
    return result


def fetch(url, target, limit):
    if target.exists():
        return sha(target)
    for attempt in range(2):
        try:
            with urlopen(url, timeout=30) as response:
                data = response.read(limit + 1)
            if len(data) > limit:
                raise ValueError('Download exceeds size limit')
            target.write_bytes(data)
            return sha(target)
        except Exception as exc:
            if attempt or getattr(exc, 'code', None) in (401, 403, 404):
                raise
            time.sleep(1)


def download_one(game, dest):
    from PIL import Image
    row = dict(game)
    folder = dest / f"{game['rank']:03d}"
    folder.mkdir(exist_ok=True)
    try:
        row['page_sha256'] = fetch(game['page'], folder / 'post.html', 4 * 1024 * 1024)
        text = (folder / 'post.html').read_text(errors='replace')
        links = re.findall(r'href=["\']([^"\']+\.p8\.png(?:\?[^"\']*)?)["\'][^>]*>\s*Cart File\s*</a>', text, re.I)
        if not links:
            links = re.findall(r'p8_run_cart\([^\n]*?["\']([^"\']+\.p8\.png)["\']', text)
        if not links:
            raise ValueError('No public Cart File link found in ranked author post')
        row['cartridge_candidates'] = list(dict.fromkeys(urljoin(game['page'], html.unescape(link)) for link in links))
        row['cartridge_url'] = row['cartridge_candidates'][0]
        row['cartridge_selection'] = 'First Cart File link in ranked author post; no alternate source substitution'
        cart = folder / 'cartridge.p8.png'
        row['cart_sha256'] = fetch(row['cartridge_url'], cart, 8 * 1024 * 1024)
        data = cart.read_bytes()
        if not data.startswith(b'\x89PNG\r\n\x1a\n'):
            raise ValueError('Cartridge response is not PNG')
        with Image.open(io.BytesIO(data)) as image:
            if image.size != (160, 205):
                raise ValueError(f'Unexpected cartridge PNG dimensions {image.size}')
            image.verify()
        license_match = re.search(r'License:\s*<a[^>]+href=["\']([^"\']+)["\'][^>]*>(.*?)</a>', text, re.S)
        if license_match:
            row['license'] = dict(url=html.unescape(license_match[1]), name=re.sub('<[^>]*>', '', license_match[2]).strip())
        row['download'] = 'ok'
    except Exception as exc:
        row.update(download='failed', download_error=f'{type(exc).__name__}: {exc}')
    return row


def downloads(dest):
    if not (dest / 'index.html').exists():
        fetch(URL, dest / 'index.html', 2 * 1024 * 1024)
    selection = freeze(dest)
    rows = []
    with ThreadPoolExecutor(max_workers=3) as pool:
        futures = [pool.submit(download_one, game, dest) for game in selection['games']]
        for future in as_completed(futures):
            row = future.result()
            rows.append(row)
            rows.sort(key=lambda r: r['rank'])
            save(dest / 'downloads.json', rows)
            print(f"DOWNLOAD {len(rows)}/100 #{row['rank']:03d} {row['title']}: {row['download']} {row.get('download_error', '')}", flush=True)
    print('DOWNLOAD_SUMMARY', dict(Counter(r['download'] for r in rows)), flush=True)


def fingerprint():
    paths = list((REPO / 'src/compiler').rglob('*.rs')) + [REPO / 'src/bin/pocket-aot.rs', REPO / 'Cargo.lock']
    paths += list((REPO / 'converter/runtime').glob('*'))
    paths += [REPO / ('converter/' + name) for name in ('build.py', 'cartridge_input.py', 'CMakeLists.txt', 'memmap_pocket.ld',
                                                        'memmap_pocket_rp2350.ld')]
    return {str(path.relative_to(REPO)): sha(path) for path in sorted(paths) if path.is_file()}


def run(command, log, timeout, env, cwd=None):
    try:
        result = subprocess.run([str(value) for value in command], env=env, cwd=cwd,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                text=True, timeout=timeout)
        output, code = result.stdout, result.returncode
    except subprocess.TimeoutExpired as exc:
        raw = exc.stdout or b''
        output = (raw.decode(errors='replace') if isinstance(raw, bytes) else raw) + '\nBENCHMARK_TIMEOUT\n'
        code = 124
    log.write_text(output)
    return code, output


def report(dest, rows, provenance):
    stage_names = ('downloaded', 'decoded', 'translated', 'host_passed', 'firmware_built')
    counts = {stage: sum(bool(row.get(stage)) for row in rows) for stage in stage_names}
    summary = dict(denominator=100, evaluated=len(rows), counts=counts, firmware_score=f"{counts['firmware_built']}/100",
                   outcomes=dict(Counter(row['outcome'] for row in rows)),
                   hardware_tested=0, host_frames_per_accepted_cart=1200, provenance=provenance,
                   blockers=dict(Counter(row.get('first_blocker', '') for row in rows if row.get('first_blocker'))))
    save(dest / 'summary.json', summary)
    save(dest / 'results.json', rows)
    lines = ['# Top 100 PICO-8 current-converter benchmark', '',
             f"Source: [Nerdy Teachers]({URL}). Exactly frozen ranks 1–100; no substitutions or source edits.", '',
             f"Converter commit: `{provenance['commit']}`. Firmware-build result: **{counts['firmware_built']}/100**.", '',
             '| Stage | Passed / 100 |', '| --- | ---: |']
    lines += [f'| {stage} | {counts[stage]} |' for stage in stage_names]
    lines += ['', 'Translation means generated C. Host checks run 1,200 scripted frames with ASan/UBSan. '
              'Leak detection alone is disabled because LeakSanitizer cannot operate under sandbox ptrace. '
              f"Firmware builds target board `{provenance.get('board', 'pico')}` + Waveshare LCD 1.3 and enforce the existing memory/stack checks. "
              'No flashing or physical gameplay tests were performed. A successful build is not a complete compatibility or fidelity test. '
              'Unavailable downloads remain in the denominator and are listed separately. '
              'Multi-cartridge/export/include dependencies are not assembled; the first public Cart File in the ranked post is tested as-is.', '',
              '| Rank | Game | Download | Decode | C | Host | UF2 | First blocker |',
              '| ---: | --- | :---: | :---: | :---: | :---: | :---: | --- |']
    for row in rows:
        mark = lambda field: 'yes' if row.get(field) else '—'
        clean = lambda text: str(text).replace('|', '\\|').replace('\n', ' ')
        lines.append(f"| {row['rank']} | {clean(row['title'])} | " + ' | '.join(mark(stage) for stage in stage_names) +
                     f" | {clean(row.get('first_blocker', ''))} |")
    (dest / 'REPORT.md').write_text('\n'.join(lines) + '\n')
    return summary


def tests(dest, compiler, board='pico'):
    spec = importlib.util.spec_from_file_location('cartridge_input', REPO / 'converter/cartridge_input.py')
    inputs = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(inputs)
    env = os.environ.copy()
    env['POCKET_AOT_BIN'] = str(compiler)
    if not env.get('PICO_SDK_PATH'):
        raise RuntimeError('Set PICO_SDK_PATH; see converter/README.md')
    env['ASAN_OPTIONS'] = 'detect_leaks=0:halt_on_error=1'
    env['UBSAN_OPTIONS'] = 'halt_on_error=1:print_stacktrace=1'
    original = fingerprint()
    provenance = dict(commit=subprocess.check_output(['git', '-C', str(REPO), 'rev-parse', 'HEAD'], text=True).strip(),
                      compiler_sha256=sha(compiler), implementation_sha256=original, benchmark_sha256=sha(Path(__file__)),
                      host_gcc=subprocess.check_output(['gcc', '--version'], text=True).splitlines()[0],
                      python=sys.version.splitlines()[0],
                      sanitizers=dict(address=True, undefined_behavior=True, leak=False,
                          reason='LeakSanitizer fails under sandbox ptrace; superseded initial attempt retained separately'),
                      board=board, started_at=datetime.now(timezone.utc).isoformat(), sdk_commit=subprocess.check_output(
                          ['git', '-C', env['PICO_SDK_PATH'], 'rev-parse', 'HEAD'], text=True).strip(),
                      criterion=f'Current generated C + 1200 ASan/UBSan host frames + completed {board} UF2 build; no source patches')
    save(dest / 'provenance.json', provenance)
    rows = []
    for downloaded in json.loads((dest / 'downloads.json').read_text()):
        row = dict(downloaded)
        row.update(downloaded=row['download'] == 'ok', decoded=False, translated=False, host_passed=False, firmware_built=False)
        folder = dest / f"{row['rank']:03d}"
        try:
            if not row['downloaded']:
                row['outcome'] = 'download_failed'
                row['first_blocker'] = row['download_error']
                continue
            cart = folder / 'cartridge.p8.png'
            assert sha(cart) == row['cart_sha256'], 'Frozen cartridge hash changed'
            source = inputs.prepare_source(cart, folder / 'decoded')
            row.update(decoded=True, decoded_sha256=sha(source))
            code, output = run([compiler, source, folder / 'generated', row['title']], folder / 'translation.log', 20, env)
            if code:
                row['outcome'] = 'translation_timeout' if code == 124 else 'translation_rejected'
                row['first_blocker'] = output.strip().splitlines()[-1][:1000] if output.strip() else f'Compiler exited {code}'
                continue
            row['translated'] = True
            code, output = run([sys.executable, REPO / 'converter/build.py', cart, '--title', row['title'], '--frames', '1200',
                                '--board', board], folder / 'build.log', 240, env)
            row['host_passed'] = re.search(r'^Compiling the RP2[0-9]+ firmware\.\.\.', output, re.M) is not None
            if code:
                row['outcome'] = ('build_timeout' if code == 124 else 'firmware_failed' if row['host_passed'] else 'host_check_failed')
                errors = [line.strip() for line in output.splitlines() if 'CARTRIDGE_ERROR:' in line or 'error:' in line]
                row['first_blocker'] = (errors[0] if errors else output.strip().splitlines()[-1])[:1000]
                continue
            match = re.search(r'^UF2 ready: (.+)$', output, re.M)
            assert match, 'Successful helper did not report artifact'
            firmware = Path(match[1])
            build_report = json.loads((firmware.parent / 'conversion.json').read_text())
            assert build_report['original_sha256'] == row['cart_sha256'], 'Built wrong input'
            assert build_report['uf2_sha256'] == sha(firmware), 'Firmware hash mismatch'
            assert build_report['host_frames'] == 1200 and build_report['host_result'].startswith('AOT_HOST_OK'), 'Missing host evidence'
            row.update(firmware_built=True, host_passed=True, outcome='firmware_built',
                       uf2_sha256=sha(firmware), firmware_bytes=build_report['firmware_bytes'],
                       firmware_path=str(firmware.relative_to(REPO)), host_result=build_report['host_result'])
        except Exception as exc:
            row.update(outcome='decode_failed' if not row['decoded'] else 'test_error', first_blocker=f'{type(exc).__name__}: {exc}')
        finally:
            rows.append(row)
            print(f"TEST {len(rows)}/100 #{row['rank']:03d} {row['title']}: {row['outcome']} {row.get('first_blocker', '')[:180]}", flush=True)
            report(dest, rows, provenance)
    assert len(rows) == 100 and fingerprint() == original, 'Incomplete selection or implementation changed during test'
    provenance.update(finished_at=datetime.now(timezone.utc).isoformat(), implementation_unchanged=True)
    save(dest / 'provenance.json', provenance)
    summary = report(dest, rows, provenance)
    print('FINAL_COUNTS', json.dumps(summary['counts']), flush=True)
    print('FIRMWARE_SCORE', summary['firmware_score'], flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('download', 'test'))
    parser.add_argument('--dest', type=Path, default=DEFAULT_DEST)
    parser.add_argument('--board', default='pico', help='PICO_BOARD for firmware builds, e.g. pico2_w for RP2350')
    parser.add_argument('--compiler', type=Path, default=Path(os.environ.get('POCKET_AOT_BIN', REPO / 'target/release/pocket-aot')))
    args = parser.parse_args()
    args.dest.mkdir(parents=True, exist_ok=True)
    downloads(args.dest.resolve()) if args.mode == 'download' else tests(args.dest.resolve(), args.compiler.resolve(), args.board)
