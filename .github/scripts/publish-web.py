#!/usr/bin/env python3
"""Publish one folder of the Pages site, and leave every other folder as it is.

Why this file exists: 2 workflows publish to the `gh-pages` branch, the book (docs.yml) and the browser demo
with its previews (web.yml), and runs of both can end at the same moment. Each one publishes through this
script, which is the one way to write to the branch (docs/src/design/docs-pipeline.md, Publishing):

- It replaces the one folder it is given on the branch's newest state, and pushes only if the branch is
  still at that state (`--force-with-lease`). When another run got there first, it starts again from that
  run's state, so neither loses the other's folder.
- It writes the branch as one commit without a parent. The editor and each preview's modules are tens of
  megabytes that change with every push, and a history of them would grow without bound.
"""
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile


# Each attempt starts from the branch's newest state, so 8 lost races in a row mean something else is wrong.
ATTEMPTS = 8


def git(*args, cwd, check=True):
    return subprocess.run(['git', *args], cwd=cwd, check=check, text=True, capture_output=True)


def publish(destination, source):
    if not re.fullmatch(r'app|dev|sheet|site-root|v[0-9]+\.[0-9]+|pr/[1-9][0-9]*', destination):
        raise ValueError('Destination must be app, dev, sheet, site-root, v<major>.<minor> or pr/<positive number>')
    source = Path(source).resolve() if source else None
    if source and not (source / 'index.html').is_file():
        raise ValueError('The built site must contain index.html')
    if source is None and not destination.startswith('pr/'):
        raise ValueError('Only PR previews can be removed')
    if destination == 'site-root' and any(p.name not in {'index.html', '404.html', '.nojekyll'} or not p.is_file() for p in source.iterdir()):
        raise ValueError('Site root accepts only index.html, 404.html and .nojekyll')
    repository = Path.cwd()
    remote = git('remote', 'get-url', 'origin', cwd=repository).stdout.strip()
    with tempfile.TemporaryDirectory(prefix='stitchcraft-pages-') as folder:
        work = Path(folder)
        git('init', cwd=work)
        git('remote', 'add', 'origin', remote, cwd=work)
        # Checkout v7 keeps its scoped header behind includeIf. Resolve it in the
        # source checkout before copying the header to this temporary repository.
        config = git('config', '--local', '--includes', '--get-regexp', r'http\..*\.extraheader', cwd=repository, check=False)
        for line in config.stdout.splitlines():
            key, value = line.split(' ', 1)
            git('config', '--local', key, value, cwd=work)
        git('config', 'user.name', 'github-actions[bot]', cwd=work)
        git('config', 'user.email', '41898282+github-actions[bot]@users.noreply.github.com', cwd=work)
        refused = ''
        for attempt in range(ATTEMPTS):
            found = git('ls-remote', '--heads', 'origin', 'gh-pages', cwd=work).stdout.strip()
            if not found and source is None:
                return  # Closing a PR that never published must not create a Pages branch.
            # The state this attempt builds on. An empty name means the branch must still not exist.
            seen = found.split()[0] if found else ''
            if found:
                git('fetch', '--depth=1', 'origin', 'gh-pages', cwd=work)
                git('checkout', '--detach', 'FETCH_HEAD', cwd=work)
                git('clean', '-fdx', cwd=work)
            else:
                git('checkout', '--orphan', 'pages', cwd=work)
                git('rm', '-r', '-q', '--cached', '--ignore-unmatch', '.', cwd=work)
                git('clean', '-fdx', cwd=work)
            if destination == 'site-root':
                # Refresh landing files without replacing docs, releases or previews.
                for path in source.iterdir():
                    shutil.copyfile(path, work / path.name)
            else:
                target = work / destination
                shutil.rmtree(target, ignore_errors=True)
                if source:
                    shutil.copytree(source, target)
            (work / '.nojekyll').touch()
            git('add', '-A', cwd=work)
            tree = git('write-tree', cwd=work).stdout.strip()
            if found and tree == git('rev-parse', 'FETCH_HEAD^{tree}', cwd=work).stdout.strip():
                return
            message = f'The site, after {destination} from {os.environ.get("GITHUB_SHA", "a local build")}'
            commit = git('commit-tree', tree, '-m', message, cwd=work).stdout.strip()
            pushed = git('push', f'--force-with-lease=refs/heads/gh-pages:{seen}', 'origin',
                         f'{commit}:refs/heads/gh-pages', cwd=work, check=False)
            if pushed.returncode == 0:
                return
            refused = pushed.stderr.strip()
        raise RuntimeError(f'The push to gh-pages was refused {ATTEMPTS} times. The last answer: {refused}')


if __name__ == '__main__':
    publish(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else None)
