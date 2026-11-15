#!/usr/bin/env python3
"""Put a pull request's preview together: its book, with its demo folder under `demo/`.

Why this file exists: a preview is what the publisher puts under `/pr/<number>/`, and the checks must open
the same folder first. The book's own `demo/` page sends a reader to the editor main published. This script
adds the pull request's plug-ins, loader and starter beside that page, and never the editor, which stays at
`/app/` alone (docs/src/design/docs-pipeline.md, Publishing).
"""
import argparse
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def assemble(book, demo, output):
    """Write `output` afresh from the built book and the demo folder `web/build.py` made."""
    for needed in (book / 'index.html', book / 'demo/index.html', demo / 'plugins.json'):
        if not needed.is_file():
            raise ValueError(f'Missing {needed}')
    editor = [path.name for path in demo.rglob('*') if path.name == 'index.html' or path.name.endswith('_bg.wasm')]
    if editor:
        raise ValueError(f'A preview carries no editor, and {demo} holds {editor}')
    shutil.rmtree(output, ignore_errors=True)
    shutil.copytree(book, output)
    shutil.copytree(demo, output / 'demo', dirs_exist_ok=True)
    (output / '.nojekyll').touch()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--base-path', required=True, help='Pages path ending in /, e.g. /stitchcraft/pr/1/')
    args = parser.parse_args()
    if not args.base_path.startswith('/') or not args.base_path.endswith('/'):
        parser.error('--base-path must start and end with /')
    env = dict(os.environ, MDBOOK_OUTPUT__HTML__SITE_URL=args.base_path)
    subprocess.run(['mdbook', 'build', 'docs'], cwd=ROOT, env=env, check=True)
    assemble(ROOT / 'docs/book', ROOT / 'target/web/demo', ROOT / 'target/site')
    print(f'Built the book and its demo folder at {ROOT / "target/site"}')
