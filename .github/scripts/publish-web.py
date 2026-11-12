#!/usr/bin/env python3
"""Replace one app/preview directory, retrying pushes without losing other publishers' work."""
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile


def git(*args, cwd, check=True):
    return subprocess.run(['git', *args], cwd=cwd, check=check, text=True, capture_output=True)


def publish(destination, source):
    if not re.fullmatch(r'app|dev|site-root|pr/[1-9][0-9]*', destination):
        raise ValueError('Destination must be app, dev, site-root or pr/<positive number>')
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
        for attempt in range(5):
            found = git('ls-remote', '--heads', 'origin', 'gh-pages', cwd=work).stdout.strip()
            if not found and source is None:
                return  # Closing a PR that never published must not create a Pages branch.
            if found:
                git('fetch', '--depth=1', 'origin', 'gh-pages', cwd=work)
                git('checkout', '-B', 'pages', 'FETCH_HEAD', cwd=work)
                git('clean', '-fdx', cwd=work)
            elif attempt == 0:
                git('checkout', '--orphan', 'pages', cwd=work)
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
            if not git('status', '--porcelain', cwd=work).stdout.strip():
                return
            git('commit', '-m', f'Web: {destination} from {os.environ.get("GITHUB_SHA", "local build")}', cwd=work)
            pushed = git('push', 'origin', 'HEAD:gh-pages', cwd=work, check=False)
            if pushed.returncode == 0:
                return
        raise RuntimeError('Pages push failed after five attempts; check branch permissions and competing publishers')


if __name__ == '__main__':
    publish(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else None)
