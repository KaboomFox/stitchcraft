#!/usr/bin/env python3
"""Build the browser demo: VectorCraft's web editor at the pinned release, with StitchCraft's plug-ins.

Why this file exists: the demo is 2 folders that are published apart, and one script must make both from
one build, or a preview's plug-ins would not be the ones its checks ran
(docs/src/design/vectorcraft-integration.md, Browser distribution).

- `target/web/demo` holds what a pull request changes: the plug-ins' modules, the loader's scripts, the
  starter drawing, its finished example and StitchCraft's licence texts. A preview publishes this folder.
- `target/web/app` holds the editor with a copy of that folder's files. Main publishes it as `/app/`.

Nothing here is downloaded but VectorCraft's source, at the commit `compat/vectorcraft.toml` records. Trunk
runs with `--offline`, so it uses the wasm-bindgen and wasm-opt the workflow installed by checksum
(`.github/actions/tools/action.yml`) and stops when one is missing.
"""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]
WEB = ROOT / 'web'
PINS = ROOT / 'compat/vectorcraft.toml'
OUT = ROOT / 'target/web'
# The filters that ask for a heavy budget, which VectorCraft installs only once a person allows it. The page
# offers them as downloads. The browser check installs every other module, so a wrong entry fails it.
HEAVY = ('stroke-to-satin', 'route-satins', 'route-runs')
# The scripts a preview brings with it. `start.mjs` belongs to the editor's page alone.
SCRIPTS = ('boot.mjs', 'loader.mjs', 'starter.mjs')
TITLE = 'StitchCraft in VectorCraft'
# GitHub refuses a push that holds a file of 100 MB or more, so the publisher could never publish one.
LARGEST_FILE = 100 * 1000 * 1000


def pin(pins=PINS):
    """The editor's source: the repository, the release's tag and the commit the tag must name."""
    table = tomllib.loads(pins.read_text())
    return table['repository'], table['web']['tag'], table['web']['revision']


def modules(manifest=ROOT / 'apps/stitchcraft-vc-plugin/Cargo.toml'):
    """The plug-in's modules, one for each feature of its crate: those the page installs, and the heavy ones."""
    features = list(tomllib.loads(manifest.read_text())['features'])
    unknown = [name for name in HEAVY if name not in features]
    if unknown:
        raise ValueError(f'HEAVY names features the plug-in does not have: {unknown}')
    return [name for name in features if name not in HEAVY], list(HEAVY)


def version(folder):
    """A name for this build of the scripts, the drawing and its finished example, so a browser never mixes 2 builds' files."""
    digest = hashlib.sha256()
    for name in (*SCRIPTS, 'start.mjs', 'templates/alpine-night.svg', 'templates/alpine-night.vctemplate'):
        digest.update((folder / name).read_bytes())
    return digest.hexdigest()[:16]


def page(html, start_version):
    """The editor's page with our title and the script that starts the loader. A page we can't change fails."""
    for old, new in (('<title>VectorCraft</title>', f'<title>{TITLE}</title>'),
                     ('</body>', f'<script type="module" src="./start.mjs?v={start_version}"></script>\n</body>')):
        if html.count(old) != 1:
            raise ValueError(f'The editor\'s page must hold {old!r} once')
        html = html.replace(old, new)
    return html


def demo(out, built, tag, revision):
    """Write the folder a preview publishes, from the modules built in `built`."""
    light, heavy = modules()
    shutil.rmtree(out, ignore_errors=True)
    (out / 'plugins').mkdir(parents=True)
    for name in light + heavy:
        shutil.copyfile(built / f'{name}.wasm', out / 'plugins' / f'{name}.wasm')
    for name in SCRIPTS:
        shutil.copyfile(WEB / name, out / name)
    shutil.copytree(WEB / 'templates', out / 'templates')
    for name in ('LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE'):
        shutil.copyfile(ROOT / name, out / f'StitchCraft-{name}.txt')
    (out / 'plugins.json').write_text(json.dumps({
        'plugins': light, 'optionalPlugins': heavy, 'version': version(WEB),
        'vectorcraft': {'tag': tag, 'revision': revision}}, indent=1) + '\n')


def app(out, editor, demo_folder, vectorcraft):
    """Write the folder main publishes: the editor Trunk built, our page, and the demo folder's files."""
    shutil.rmtree(out, ignore_errors=True)
    shutil.copytree(editor, out)
    (out / '.trunk-lock').unlink(missing_ok=True)
    shutil.copytree(demo_folder, out, dirs_exist_ok=True)
    shutil.copyfile(WEB / 'start.mjs', out / 'start.mjs')
    start_version = hashlib.sha256((WEB / 'start.mjs').read_bytes()).hexdigest()[:16]
    (out / 'index.html').write_text(page((out / 'index.html').read_text(), start_version))
    # VectorCraft's licence asks that copies carry its texts and notice (MIT, Apache-2.0 section 4).
    for name in ('LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE'):
        shutil.copyfile(vectorcraft / name, out / f'VectorCraft-{name}.txt')


def sizes(folder):
    """A table of the folder's size as stored and as gzip sends it, with each file of 1 MB or more."""
    rows, total, total_sent = [], 0, 0
    for path in sorted(p for p in folder.rglob('*') if p.is_file()):
        data = path.read_bytes()
        sent = len(gzip.compress(data, 6))
        total, total_sent = total + len(data), total_sent + sent
        if len(data) >= LARGEST_FILE:
            raise ValueError(f'{path} has {len(data)} bytes: GitHub refuses a file of 100 MB or more')
        if len(data) >= 1000 * 1000:
            rows.append(f'| `{path.relative_to(folder)}` | {len(data):,} | {sent:,} |')
    return '\n'.join([f'| {folder.name}/ | bytes | bytes, gzip -6 |', '|---|---:|---:|', *rows,
                      f'| **all files** | {total:,} | {total_sent:,} |'])


def run(*args, cwd=ROOT, **more):
    return subprocess.run(args, cwd=cwd, check=True, **more)


def checkout(folder):
    """VectorCraft's source at the pinned commit, never changed. A tag that names another commit fails."""
    repository, tag, revision = pin()
    if not folder.exists():
        run('git', 'clone', '--depth', '1', '--branch', tag, repository, str(folder))
    found = run('git', 'rev-parse', 'HEAD', cwd=folder, text=True, capture_output=True).stdout.strip()
    if found != revision:
        raise SystemExit(f'VectorCraft {tag} must be commit {revision}, and {folder} is at {found}')
    return tag, revision


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--prepare', action='store_true', help='fetch the pinned editor source, and build nothing')
    parser.add_argument('--sizes', action='store_true', help='print the sizes of what an earlier run built')
    args = parser.parse_args()
    if args.sizes:
        print('\n\n'.join(sizes(OUT / name) for name in ('app', 'demo')))
        return
    # VectorCraft has no toolchain file: its build uses StitchCraft's pinned compiler.
    os.environ['RUSTUP_TOOLCHAIN'] = tomllib.loads((ROOT / 'rust-toolchain.toml').read_text())['toolchain']['channel']
    vectorcraft = Path(os.environ.get('VECTORCRAFT_DIR', ROOT / 'target/vectorcraft-web'))
    tag, revision = checkout(vectorcraft)
    if args.prepare:
        return
    # Trunk passes --offline on to Cargo, so the crates are fetched first.
    run('cargo', 'fetch', '--locked', cwd=vectorcraft)
    run('trunk', 'build', '--release', '--locked', '--offline', cwd=vectorcraft / 'apps/vectorcraft-web')
    # Cargo says where it builds: inside a VectorCraft checkout that is not this folder's target/ (ADR-0011).
    metadata = run('cargo', 'metadata', '--no-deps', '--format-version', '1', text=True, capture_output=True)
    target = Path(json.loads(metadata.stdout)['target_directory'])
    built = OUT / 'modules'
    built.mkdir(parents=True, exist_ok=True)
    light, heavy = modules()
    for name in light + heavy:
        run('cargo', 'build', '--locked', '--release', '--target', 'wasm32-unknown-unknown',
            '-p', 'stitchcraft-vc-plugin', '--no-default-features', '--features', name)
        shutil.copyfile(target / 'wasm32-unknown-unknown/release/stitchcraft_vc_plugin.wasm', built / f'{name}.wasm')
    demo(OUT / 'demo', built, tag, revision)
    app(OUT / 'app', vectorcraft / 'dist/web', OUT / 'demo', vectorcraft)
    print(f'Built {OUT / "app"} and {OUT / "demo"}')


if __name__ == '__main__':
    main()
