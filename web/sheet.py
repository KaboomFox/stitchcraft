#!/usr/bin/env python3
"""Make the before and after sheet: the standard pictures as this revision sews them, beside main's.

Why this file exists: a change to the engine moves what the pictures sew, and a reviewer should see that
without building anything (docs/src/design/before-after-sheet.md). The sheet is a page in a pull request's
preview. This script makes it in 3 steps, and the browser's checks (`sheet.mjs`) run once `stitch` is built:

- `tools WORK` builds `stitch`, which the browser's checks compare the editor with.
- `measure WORK` builds the testkit's `sheet` example and runs it. The example plans each standard picture,
  draws it and measures it with the functions the cases use. No number is computed here.
- `page WORK OUT` reads what main published of its own sheet, puts the 2 sets of numbers and pictures side
  by side in `OUT`, and writes the lines the preview's comment shows.

Main's numbers are the ones main's own run published, not a second build of main in this run, which would
double the time. The page names the commit they are from, and says when that commit and the branch differ
by more than the branch's own commits.
"""
import argparse
import html
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
# The release profile without its link-time optimization, which takes minutes and changes no stitch: the
# engine's arithmetic is the same in every build (docs/src/design/determinism.md).
QUICK_RELEASE = {'CARGO_PROFILE_RELEASE_LTO': 'off', 'CARGO_PROFILE_RELEASE_CODEGEN_UNITS': '16'}
# A picture's file as the example names it. A name read from the network is checked before it is a path.
PICTURE = re.compile(r'[a-z0-9][a-z0-9-]*\.png')
# How many changes for the worse the comment lists. The sheet has them all.
WORST = 5


def run(*args, env=None):
    subprocess.run(args, cwd=ROOT, check=True, env=dict(os.environ, **(env or {})))


def release_folder():
    """Where Cargo puts this repository's release builds, which is not always its own `target` (ADR-0011)."""
    metadata = subprocess.run(['cargo', 'metadata', '--no-deps', '--format-version', '1'], cwd=ROOT, check=True,
                              text=True, capture_output=True)
    return Path(json.loads(metadata.stdout)['target_directory']) / 'release'


def tools(work):
    """Make `work` new and build `stitch`, which the browser's checks compare the editor with."""
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    started = time.monotonic()
    run('cargo', 'build', '--release', '--locked', '-p', 'stitchcraft-cli', env=QUICK_RELEASE)
    (work / 'build.json').write_text(json.dumps({
        'stitch': str(release_folder() / 'stitch'), 'seconds': {'build': round(time.monotonic() - started)}}, indent=1) + '\n')


def measure(work):
    """Build the example and have it write `work/after`. Adds the time each took to what `tools` recorded."""
    started = time.monotonic()
    run('cargo', 'build', '--release', '--locked', '-p', 'stitchcraft-testkit', '--example', 'sheet', env=QUICK_RELEASE)
    built = time.monotonic()
    run(str(release_folder() / 'examples/sheet'), str(work / 'after'))
    build = json.loads((work / 'build.json').read_text())
    build['seconds']['build'] += round(built - started)
    build['seconds']['measure'] = round(time.monotonic() - built)
    (work / 'build.json').write_text(json.dumps(build, indent=1) + '\n')


def whole(number):
    return isinstance(number, (int, float)) and float(number).is_integer()


def shown(number):
    """A number as the sheet writes it: a count whole, a share to a thousandth."""
    if number is None:
        return 'none'
    return f'{int(number):,}' if whole(number) else f'{number:.3f}'


def changes(measures, before, after):
    """Every number of `after` that differs from `before`, as rows a table shows.

    A row is `picture`, `label`, `before`, `after`, `verdict` and `weight`. The verdict is `worse` or
    `better` for a measure of which more is worse, and `changed` for one that is neither. The weight is the
    change as a share of the number before, of at least 1 for a count and 0.01 for a share, so that a rise
    from nothing is ranked and not divided by 0. A picture that planned before and does not now is worse
    than any number.
    """
    was = {picture['name']: picture for picture in before}
    rows = []
    for picture in after:
        old = was.get(picture['name'])
        if old is None or 'error' in old:
            continue
        if 'error' in picture:
            rows.append({'picture': picture['name'], 'label': 'Plans', 'before': 'yes', 'after': 'no',
                         'verdict': 'worse', 'weight': float('inf')})
            continue
        for measure in measures:
            a, b = picture.get(measure['key']), old.get(measure['key'])
            if a is None or b is None or a == b:
                continue
            floor = 1 if whole(a) and whole(b) else 0.01
            verdict = 'changed' if not measure['more_is_worse'] else 'worse' if a > b else 'better'
            rows.append({'picture': picture['name'], 'label': measure['label'], 'before': shown(b), 'after': shown(a),
                         'verdict': verdict, 'weight': abs(a - b) / max(abs(b), floor)})
    return rows


def staleness(compared, own_commits):
    """What separates main's sheet from this branch besides the branch's own commits, as sentences.

    `compared` is GitHub's comparison from the commit of main's sheet to the branch's head, and `own_commits`
    how many commits the pull request has. Commits the sheet's commit has and the branch lacks are `behind_by`.
    Commits the branch has beyond its own are main's commits from after the sheet was made.
    """
    if not compared:
        return ['Whether main\'s sheet and this branch start from the same commit could not be checked.']
    said = []
    behind = compared.get('behind_by', 0)
    newer = compared.get('ahead_by', 0) - (own_commits or 0)
    if behind:
        said.append(f'Main\'s sheet has {behind} commit{"s" if behind != 1 else ""} this branch lacks. '
                    'Some differences may be theirs: merge main to see this branch\'s alone.')
    if own_commits is not None and newer > 0:
        said.append(f'This branch has {newer} commit{"s" if newer != 1 else ""} of main from after main\'s sheet was '
                    'made. Some differences may be theirs: main\'s sheet catches up when its run ends.')
    return said


def get(url, token=None):
    request = urllib.request.Request(url, headers={'User-Agent': 'stitchcraft-sheet'})
    if token:
        request.add_header('Authorization', f'Bearer {token}')
    with urllib.request.urlopen(request, timeout=30) as response:
        return response.read()


def fetch_main(url, folder):
    """Main's published record with its pictures copied into `folder`, or `None` when main has no sheet yet."""
    try:
        record = json.loads(get(f'{url}sheet.json'))
    except (urllib.error.URLError, TimeoutError, ValueError) as error:
        print(f'Main\'s sheet at {url} could not be read: {error}')
        return None
    shutil.rmtree(folder, ignore_errors=True)
    folder.mkdir(parents=True)
    names = [picture.get('picture') for picture in record.get('pictures', [])]
    names.append((record.get('browser') or {}).get('editor'))
    for name in names:
        if not name or not PICTURE.fullmatch(name):
            continue
        try:
            (folder / name).write_bytes(get(f'{url}after/{name}'))
        except (urllib.error.URLError, TimeoutError) as error:
            print(f'Main\'s picture {name} could not be read: {error}')
    return record


def compare(repository, base, head, token):
    """GitHub's comparison of 2 commits, or `None` when it can't be had."""
    if not (repository and base and head):
        return None
    try:
        return json.loads(get(f'https://api.github.com/repos/{repository}/compare/{base}...{head}', token))
    except (urllib.error.URLError, TimeoutError, ValueError) as error:
        print(f'The comparison of {base} and {head} could not be read: {error}')
        return None


def seconds(ms):
    return f'{ms / 1000:.1f} s'


def browser_lines(browser):
    """What the browser's checks showed, as sentences for the page and the comment."""
    if not browser:
        return ['The browser\'s checks did not run.']
    said = []
    parity = browser.get('parity', [])
    same = [design['design'] for design in parity if all((design.get(kind) or {}).get('same') for kind in ('pes', 'dst'))]
    if same:
        said.append(f'The editor in Chromium and `stitch` wrote the same PES and DST files, byte for byte, of {", ".join(same)}.')
    for design in parity:
        refused = sorted({design[kind]['refused'] for kind in ('pes', 'dst') if (design.get(kind) or {}).get('refused')})
        if refused:
            said.append(f'The editor did not export {design["design"]}, so nothing was compared: {"; ".join(refused)}.')
    speed = browser.get('speed')
    if speed and speed.get('runs'):
        turns = sorted(run['turn_ms'] for run in speed['runs'])
        exports = sorted(run['export_ms'] for run in speed['runs'] if run.get('export_ms') is not None)
        limit = speed.get('limit_ms') or {}
        within = lambda key: f' (limit {seconds(limit[key])})' if limit.get(key) else ''
        left = max(run.get('waiting', 0) for run in speed['runs'])
        export = (f'its PES export {seconds(exports[len(exports) // 2])}{within("export")}' if exports
                  else 'its PES export was refused')
        said.append(f'Giving {speed["design"]} the Auto effect took '
                    f'{seconds(turns[len(turns) // 2])}{within("turn")} until the editor had drawn them'
                    f'{f", but for {left} it left undrawn" if left else ""}, and {export}: '
                    f'the middle of {len(turns)} runs in Chromium drawing in software.')
    return said


def comment(rows, pictures, main, stale, browser):
    """The section of the preview's comment, as lines. `{sheet}` stands for the sheet's address."""
    lines = ['**Before and after:** {sheet}', '']
    if main is None:
        lines.append(f'{pictures} pictures as this pull request sews them. Main has published no sheet to compare with yet.')
    else:
        worse = sorted((row for row in rows if row['verdict'] == 'worse'), key=lambda row: -row['weight'])
        better = sum(row['verdict'] == 'better' for row in rows)
        lines.append(f'{pictures} pictures as this pull request sews them, against main at {main["revision"][:7]}: '
                     f'{len(worse)} numbers got worse, {better} got better, '
                     f'{len(rows) - len(worse) - better} changed otherwise.')
        if worse:
            lines += ['', '| Picture | Number | main | this pull request |', '|---|---|---:|---:|']
            lines += [f'| {row["picture"]} | {row["label"]} | {row["before"]} | {row["after"]} |' for row in worse[:WORST]]
            if len(worse) > WORST:
                lines.append(f'\nThe sheet has the other {len(worse) - WORST}.')
        lines += ['', *stale] if stale else []
    return lines + ['', *browser_lines(browser)]


STYLE = '''
:root { color-scheme: light dark; --line: #8886; --worse: #c62828; --better: #2e7d32; }
body { font: 15px/1.5 system-ui, sans-serif; margin: 0 auto; max-width: 1200px; padding: 16px; }
table { border-collapse: collapse; margin: 12px 0; }
th, td { border: 1px solid var(--line); padding: 4px 8px; text-align: right; }
th:first-child, td:first-child { text-align: left; }
.worse { color: var(--worse); font-weight: 600; }
.better { color: var(--better); }
.was { opacity: .65; }
.pair { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin: 8px 0 28px; }
.pair.one { grid-template-columns: 1fr; }
figure { margin: 0; }
figure img { max-width: 100%; height: auto; border: 1px solid var(--line); background: #f4efe6; }
.scroll { overflow-x: auto; }
'''


def cell(measure, old, new):
    """One number's cell: the number now, and the one before when it differs, marked when it got worse."""
    if new is None:
        return '<td>none</td>'
    if old is None or old == new:
        return f'<td>{shown(new)}</td>'
    verdict = 'changed' if not measure['more_is_worse'] else 'worse' if new > old else 'better'
    mark = {'worse': ' ▲ worse', 'better': ' ▼ better', 'changed': ''}[verdict]
    return f'<td class="{verdict}"><span class="was">{shown(old)} →</span> {shown(new)}{mark}</td>'


def figure(folder, name, caption):
    if not name:
        return f'<figure><figcaption>{html.escape(caption)}: no picture</figcaption></figure>'
    return (f'<figure><img src="{folder}/{html.escape(name)}" alt="{html.escape(caption)}" loading="lazy">'
            f'<figcaption>{html.escape(caption)}</figcaption></figure>')


def page(record, main, stale, have):
    """The sheet as an HTML page. `have` are the names of main's pictures that were copied beside it."""
    e = html.escape
    measures = record['measures']
    was = {picture['name']: picture for picture in (main or {}).get('pictures', [])}
    now = 'this pull request' if main is not None else 'this revision'
    out = ['<!doctype html>', '<html lang="en">', '<meta charset="utf-8">',
           '<meta name="viewport" content="width=device-width, initial-scale=1">',
           '<title>StitchCraft, before and after</title>', f'<style>{STYLE}</style>', '<h1>Before and after</h1>',
           f'<p>The standard pictures as {now} sews them, at <code>{e(record["revision"][:10])}</code>'
           + (f', beside main at <code>{e(main["revision"][:10])}</code>.' if main is not None else '.')
           + ' Every number is measured by the functions the conformance cases use.</p>']
    if main is None and record.get('ref') != 'main':
        out.append('<p>Main has published no sheet yet, so there is nothing to compare with.</p>')
    out += [f'<p>{e(line)}</p>' for line in stale]
    head = ''.join(f'<th>{e(measure["label"])}</th>' for measure in measures)
    out += ['<h2>Numbers</h2>', '<div class="scroll"><table>', f'<tr><th>Picture</th>{head}</tr>']
    for picture in record['pictures']:
        old = was.get(picture['name'], {})
        if 'error' in picture:
            marked = ' class="worse"' if old and 'error' not in old else ''
            out.append(f'<tr><td>{e(picture["name"])}</td><td colspan="{len(measures)}"{marked}>does not plan: '
                       f'{e(picture["error"][:600])}</td></tr>')
            continue
        cells = ''.join(cell(measure, old.get(measure['key']), picture.get(measure['key'])) for measure in measures)
        out.append(f'<tr><td><a href="#{e(picture["name"])}">{e(picture["name"])}</a></td>{cells}</tr>')
    out += ['</table></div>',
            '<p>A number in red got worse and one in green got better. More stitches are neither.</p>']
    browser, before = record.get('browser') or {}, (main or {}).get('browser') or {}
    out += ['<h2>In the browser</h2>', *(f'<p>{e(line).replace("`stitch`", "<code>stitch</code>")}</p>'
                                         for line in browser_lines(browser))]
    if before.get('speed'):
        out.append(f'<p>On main: {e(browser_lines({"speed": before["speed"]})[0])}</p>')
    if browser.get('editor'):
        old = before.get('editor') if before.get('editor') in have else None
        figures = ([figure('before', old, 'The editor with main\'s plug-ins')] if main is not None else []) + [
            figure('after', browser['editor'], f'The editor with the plug-ins of {now}')]
        out.append(f'<div class="pair{"" if main is not None else " one"}">{"".join(figures)}</div>')
    out.append('<h2>Pictures</h2>')
    for picture in record['pictures']:
        name = picture['name']
        out.append(f'<h3 id="{e(name)}">{e(name)}</h3>')
        old = was.get(name, {}).get('picture')
        figures = ([figure('before', old if old in have else None, f'{name} on main')] if main is not None else []) + [
            figure('after', picture.get('picture'), f'{name} in {now}')]
        out.append(f'<div class="pair{"" if main is not None else " one"}">{"".join(figures)}</div>')
    taken = record.get('seconds') or {}
    if taken:
        out.append(f'<p>This run built the tools in {taken.get("build", "?")} s and planned and measured the pictures in '
                   f'{taken.get("measure", "?")} s.</p>')
    return '\n'.join(out + ['</html>', ''])


def assemble(work, out, revision, ref, main_url, compared_with=None):
    """Write the sheet's folder `out` from what `measure` and the browser's checks left in `work`."""
    numbers = json.loads((work / 'after/numbers.json').read_text())
    build = json.loads((work / 'build.json').read_text())
    browser_file = work / 'browser.json'
    browser = json.loads(browser_file.read_text()) if browser_file.is_file() else None
    record = {'revision': revision, 'ref': ref, 'measures': numbers['measures'], 'pictures': numbers['pictures'],
              'browser': browser, 'seconds': build.get('seconds')}
    main = fetch_main(main_url, work / 'before') if main_url else None
    stale = []
    if main is not None:
        compared, own = compared_with(main['revision']) if compared_with else (None, None)
        stale = staleness(compared, own)
    shutil.rmtree(out, ignore_errors=True)
    shutil.copytree(work / 'after', out / 'after')
    (out / 'after/numbers.json').unlink()
    have = set()
    if main is not None:
        shutil.copytree(work / 'before', out / 'before')
        have = {path.name for path in (out / 'before').iterdir()}
    rows = changes(record['measures'], main['pictures'], record['pictures']) if main is not None else []
    (out / 'sheet.json').write_text(json.dumps(record, indent=1) + '\n')
    (out / 'index.html').write_text(page(record, main, stale, have))
    (out / 'comment.json').write_text(json.dumps(comment(rows, len(record['pictures']), main, stale, browser), indent=1) + '\n')
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest='command', required=True)
    for name, what in (('tools', 'make the work folder new and build stitch'),
                       ('measure', 'plan, draw and measure the standard pictures')):
        commands.add_parser(name, help=what).add_argument('work', type=Path)
    paging = commands.add_parser('page', help='write the sheet from the numbers, the browser\'s results and main\'s sheet')
    paging.add_argument('work', type=Path)
    paging.add_argument('out', type=Path)
    paging.add_argument('--revision', required=True, help='the commit the numbers are of')
    paging.add_argument('--ref', required=True, help='main, or pr/<number>')
    paging.add_argument('--main', help='the address of main\'s published sheet, ending in /: leave out on main')
    paging.add_argument('--own-commits', type=int, help='how many commits the pull request has')
    args = parser.parse_args()
    if args.command in ('tools', 'measure'):
        {'tools': tools, 'measure': measure}[args.command](args.work.resolve())
        return
    def compared_with(base):
        found = compare(os.environ.get('GITHUB_REPOSITORY'), base, args.revision, os.environ.get('GITHUB_TOKEN'))
        return found, args.own_commits
    rows = assemble(args.work.resolve(), args.out.resolve(), args.revision, args.ref, args.main, compared_with)
    worse = [row for row in rows if row['verdict'] == 'worse']
    print(f'The sheet is at {args.out}: {len(rows)} numbers differ from main\'s, {len(worse)} for the worse.')
    for row in worse:
        print(f'  worse: {row["picture"]}, {row["label"]}: {row["before"]} to {row["after"]}')


if __name__ == '__main__':
    main()
