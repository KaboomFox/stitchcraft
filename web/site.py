#!/usr/bin/env python3
"""Build a complete docs preview with its own browser demo under demo/."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def assemble(book, demo, output):
    for source in (book, demo):
        if not (source / 'index.html').is_file():
            raise ValueError(f'Missing index.html in {source}')
    shutil.rmtree(output, ignore_errors=True)
    shutil.copytree(book, output)
    # The published book's launcher must not send a PR reader to the permanent app.
    shutil.rmtree(output / 'demo', ignore_errors=True)
    shutil.copytree(demo, output / 'demo')
    (output / '.nojekyll').touch()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base-path', required=True, help='Pages path ending in /, e.g. /stitchcraft/pr/1/')
    args = parser.parse_args()
    if not args.base_path.startswith('/') or not args.base_path.endswith('/'):
        parser.error('--base-path must start and end with /')
    env = dict(os.environ, MDBOOK_OUTPUT__HTML__SITE_URL=args.base_path)
    subprocess.run(['mdbook', 'build', 'docs'], cwd=ROOT, env=env, check=True)
    assemble(ROOT / 'docs/book', ROOT / 'target/web', ROOT / 'target/site')
    print(f'Built docs and demo at {ROOT / "target/site"}')
