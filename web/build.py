#!/usr/bin/env python3
"""Build pinned VectorCraft and StitchCraft modules into one relocatable static site."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tomllib

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--prepare', action='store_true', help='check out the pinned editor without building')
args = parser.parse_args()

ROOT = Path(__file__).resolve().parents[1]
CONFIG = json.loads((ROOT / 'web/config.json').read_text())
# VectorCraft has no toolchain file; keep its build on StitchCraft's reviewed compiler too.
os.environ['RUSTUP_TOOLCHAIN'] = tomllib.loads((ROOT / 'rust-toolchain.toml').read_text())['toolchain']['channel']
VC = Path(os.environ.get('VECTORCRAFT_DIR', ROOT / 'target/vectorcraft-web'))
OUT = ROOT / 'target/web'


def run(*args, cwd=ROOT):
    subprocess.run(args, cwd=cwd, check=True)


if not VC.exists():
    run('git', 'clone', '--no-checkout', CONFIG['vectorcraftRepository'], str(VC))
    run('git', 'checkout', '--detach', CONFIG['vectorcraftRevision'], cwd=VC)
revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=VC, text=True).strip()
if revision != CONFIG['vectorcraftRevision']:
    raise SystemExit(f'VectorCraft must be at {CONFIG["vectorcraftRevision"]}, got {revision}')
if args.prepare:
    raise SystemExit(0)
run('trunk', 'build', '--release', '--locked', cwd=VC / 'apps/vectorcraft-web')
shutil.rmtree(OUT, ignore_errors=True)
shutil.copytree(VC / 'dist/web', OUT)
(OUT / '.trunk-lock').unlink(missing_ok=True)
(OUT / 'plugins').mkdir()
for feature in CONFIG['plugins'] + CONFIG['optionalPlugins']:
    run('cargo', 'build', '--locked', '--release', '--target', 'wasm32-unknown-unknown',
        '-p', 'stitchcraft-vc-plugin', '--no-default-features', '--features', feature)
    shutil.copyfile(ROOT / 'target/wasm32-unknown-unknown/release/stitchcraft_vc_plugin.wasm',
                    OUT / 'plugins' / f'{feature}.wasm')
for name in ['loader.mjs', 'boot.mjs']:
    shutil.copyfile(ROOT / 'web' / name, OUT / name)
(OUT / 'plugins.json').write_text(json.dumps(CONFIG))
html = (OUT / 'index.html').read_text()
html = html.replace('<title>VectorCraft</title>', '<title>StitchCraft + VectorCraft</title>')
html = html.replace('</body>', '<script type="module" src="./boot.mjs"></script>\n</body>')
(OUT / 'index.html').write_text(html)
(OUT / '.nojekyll').touch()
for name in ['LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE']:
    shutil.copyfile(ROOT / name, OUT / f'StitchCraft-{name}.txt')
    shutil.copyfile(VC / name, OUT / f'VectorCraft-{name}.txt')
print(f'Built {OUT}')
