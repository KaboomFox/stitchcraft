"""Why this file exists: the sheet says which numbers got worse, and a sheet that marks the wrong ones, or
hides that main's numbers are of another commit, misleads the reviewer it is for (web/sheet.py)."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
import unittest.mock

WEB = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('sheet', WEB / 'sheet.py')
sheet = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sheet)

MEASURES = [{'key': 'stitches', 'label': 'Stitches', 'more_is_worse': False},
            {'key': 'thread_changes', 'label': 'Thread changes', 'more_is_worse': True},
            {'key': 'bare_percent', 'label': 'Bare, % of the design', 'more_is_worse': True}]


def picture(name, stitches, changes, bare):
    return {'name': name, 'picture': f'{name}.png', 'stitches': stitches, 'thread_changes': changes, 'bare_percent': bare}


class ChangesTest(unittest.TestCase):
    def test_a_number_is_worse_only_when_more_of_it_is_worse_and_it_rose(self):
        before = [picture('dog', 1000, 18, 0.018), picture('pear', 500, 3, 0.0)]
        after = [picture('dog', 1200, 28, 0.018), picture('pear', 400, 2, 0.0)]
        rows = sheet.changes(MEASURES, before, after)
        self.assertEqual([(row['picture'], row['label'], row['before'], row['after'], row['verdict']) for row in rows], [
            ('dog', 'Stitches', '1,000', '1,200', 'changed'), ('dog', 'Thread changes', '18', '28', 'worse'),
            ('pear', 'Stitches', '500', '400', 'changed'), ('pear', 'Thread changes', '3', '2', 'better')])
        self.assertAlmostEqual(rows[1]['weight'], 10 / 18)

    def test_a_rise_from_nothing_has_a_weight_and_a_share_is_weighed_by_its_own_floor(self):
        rows = sheet.changes(MEASURES, [picture('dog', 1, 0, 0.0)], [picture('dog', 1, 2, 0.081)])
        self.assertEqual([(row['label'], row['before'], row['after'], row['verdict']) for row in rows],
                         [('Thread changes', '0', '2', 'worse'), ('Bare, % of the design', '0', '0.081', 'worse')])
        self.assertEqual(rows[0]['weight'], 2.0)
        self.assertAlmostEqual(rows[1]['weight'], 8.1)

    def test_a_picture_that_stops_planning_is_the_worst_and_a_new_one_is_no_change(self):
        before = [picture('dog', 1, 1, 0.0), {'name': 'pear', 'error': 'it does not plan'}]
        after = [{'name': 'dog', 'error': 'it does not plan'}, picture('pear', 1, 1, 0.0), picture('patch', 1, 1, 0.0)]
        rows = sheet.changes(MEASURES, before, after)
        self.assertEqual([(row['picture'], row['label'], row['verdict']) for row in rows], [('dog', 'Plans', 'worse')])
        self.assertEqual(rows[0]['weight'], float('inf'))

    def test_a_number_missing_on_either_side_is_no_change(self):
        before, after = picture('dog', 1, 1, None), picture('dog', 1, 1, 0.5)
        self.assertEqual(sheet.changes(MEASURES, [before], [after]), [])
        self.assertEqual(sheet.changes(MEASURES, [after], [before]), [])


class StalenessTest(unittest.TestCase):
    def test_the_same_start_says_nothing(self):
        self.assertEqual(sheet.staleness({'behind_by': 0, 'ahead_by': 4}, 4), [])

    def test_commits_of_main_the_branch_lacks_are_named(self):
        said = sheet.staleness({'behind_by': 3, 'ahead_by': 4}, 4)
        self.assertEqual(len(said), 1)
        self.assertIn('3 commits this branch lacks', said[0])
        self.assertIn('1 commit this branch lacks', sheet.staleness({'behind_by': 1, 'ahead_by': 4}, 4)[0])

    def test_commits_of_main_from_after_its_sheet_are_named(self):
        said = sheet.staleness({'behind_by': 0, 'ahead_by': 6}, 4)
        self.assertEqual(len(said), 1)
        self.assertIn('2 commits of main from after', said[0])
        self.assertEqual(len(sheet.staleness({'behind_by': 2, 'ahead_by': 5}, 4)), 2)

    def test_a_comparison_that_could_not_be_had_is_said(self):
        self.assertIn('could not be checked', sheet.staleness(None, 4)[0])
        self.assertEqual(sheet.staleness({'behind_by': 0, 'ahead_by': 9}, None), [])


SAME = {'pes': {'same': True}, 'dst': {'same': True}}
BROWSER = {'parity': [{'design': 'alpine-night', **SAME}, {'design': 'dog-on-turf-6', **SAME}], 'editor': 'editor.png',
           'speed': {'design': 'dog-on-turf-6', 'shapes': 97, 'limit_ms': {'turn': 60000, 'export': None},
                     'runs': [{'turn_ms': 9000, 'export_ms': 700}, {'turn_ms': 4000, 'export_ms': 500}, {'turn_ms': 5000, 'export_ms': 900}]}}


class CommentTest(unittest.TestCase):
    def test_the_worst_changes_come_first_and_the_rest_are_counted(self):
        rows = [{'picture': f'p{k}', 'label': 'Jumps', 'before': '1', 'after': str(k), 'verdict': 'worse', 'weight': k} for k in range(2, 9)]
        rows.append({'picture': 'p', 'label': 'Slivers', 'before': '2', 'after': '1', 'verdict': 'better', 'weight': 0.5})
        rows.append({'picture': 'p', 'label': 'Stitches', 'before': '2', 'after': '1', 'verdict': 'changed', 'weight': 0.5})
        lines = sheet.comment(rows, 10, {'revision': 'abcdef0123'}, ['Main is ahead.'], BROWSER)
        self.assertEqual(lines[0], '**Before and after:** {sheet}')
        self.assertIn('against main at abcdef0: 7 numbers got worse, 1 got better, 1 changed otherwise.', lines[2])
        table = [line for line in lines if line.startswith('| p')]
        self.assertEqual(table, [f'| p{k} | Jumps | 1 | {k} |' for k in (8, 7, 6, 5, 4)])
        self.assertIn('\nThe sheet has the other 2.', lines)
        self.assertIn('Main is ahead.', lines)
        self.assertTrue(all(len(line) <= 600 for line in lines) and len(lines) <= 60)

    def test_the_browser_s_checks_are_said_with_the_middle_time_and_its_limit(self):
        said = sheet.browser_lines(BROWSER)
        self.assertIn('byte for byte, of alpine-night, dog-on-turf-6.', said[0])
        self.assertIn('the 97 shapes of dog-on-turf-6 the Auto effect took 5.0 s (limit 60.0 s)', said[1])
        self.assertIn('its PES export 0.7 s:', said[1])
        self.assertEqual(sheet.browser_lines(None), ['The browser\'s checks did not run.'])

    def test_an_export_the_editor_refused_is_said_and_not_counted_as_the_same(self):
        refused = {'refused': 'plug-in exceeded its instruction budget', 'ms': 9000}
        runs = [{'turn_ms': 4000, 'export_ms': None, 'waiting': 3}]
        said = sheet.browser_lines({'parity': [{'design': 'alpine-night', **SAME}, {'design': 'dog', 'pes': refused, 'dst': refused}],
                                    'speed': {'design': 'dog', 'shapes': 97, 'limit_ms': {}, 'runs': runs}})
        self.assertIn('byte for byte, of alpine-night.', said[0])
        self.assertEqual(said[1], 'The editor did not export dog, so nothing was compared: plug-in exceeded its instruction budget.')
        self.assertIn('took 4.0 s until the editor had drawn them, but for 3 it left undrawn, and its PES export was refused:', said[2])

    def test_without_main_s_sheet_the_comment_says_so_and_lists_nothing(self):
        lines = sheet.comment([], 10, None, [], BROWSER)
        self.assertIn('Main has published no sheet to compare with yet.', lines[2])
        self.assertFalse(any(line.startswith('|') for line in lines))


class AssembleTest(unittest.TestCase):
    def folder(self, root, pictures):
        work = root / 'work'
        (work / 'after').mkdir(parents=True)
        (work / 'after/numbers.json').write_text(json.dumps({'measures': MEASURES, 'pictures': pictures}))
        for entry in pictures:
            if 'picture' in entry:
                (work / 'after' / entry['picture']).write_bytes(b'png')
        (work / 'after/editor.png').write_bytes(b'png')
        (work / 'build.json').write_text(json.dumps({'stitch': '/x/stitch', 'seconds': {'build': 90, 'measure': 40}}))
        (work / 'browser.json').write_text(json.dumps(BROWSER))
        return work

    def test_main_s_own_sheet_has_its_pictures_and_no_comparison(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            work = self.folder(root, [picture('dog', 10, 2, 0.0), {'name': 'pear', 'error': 'it does not plan: <b>'}])
            (root / 'out').mkdir()
            (root / 'out/stale.txt').write_text('from an earlier run')
            rows = sheet.assemble(work, root / 'out', 'a' * 40, 'main', None)
            self.assertEqual(rows, [])
            self.assertEqual(sorted(path.name for path in (root / 'out').iterdir()), ['after', 'comment.json', 'index.html', 'sheet.json'])
            self.assertEqual(sorted(path.name for path in (root / 'out/after').iterdir()), ['dog.png', 'editor.png'])
            record = json.loads((root / 'out/sheet.json').read_text())
            self.assertEqual((record['revision'], record['ref'], record['browser']['editor']), ('a' * 40, 'main', 'editor.png'))
            page = (root / 'out/index.html').read_text()
            self.assertIn('does not plan: it does not plan: &lt;b&gt;', page)
            self.assertNotIn('before/', page)
            self.assertNotIn('Main has published no sheet', page)

    def test_a_pull_request_s_sheet_marks_what_got_worse_beside_main_s_pictures(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            work = self.folder(root, [picture('dog', 12, 5, 0.0), picture('patch', 1, 1, 0.0)])
            published = {'revision': 'b' * 40, 'ref': 'main', 'measures': MEASURES, 'browser': {'editor': 'editor.png', 'speed': BROWSER['speed']},
                         'pictures': [picture('dog', 10, 2, 0.0), {'name': 'x', 'picture': '../../escape.png'}]}
            asked = []

            def get(url, token=None):
                asked.append(url)
                return json.dumps(published).encode() if url.endswith('sheet.json') else b'main png'

            with unittest.mock.patch.object(sheet, 'get', get):
                rows = sheet.assemble(work, root / 'out', 'c' * 40, 'pr/7', 'https://site/sheet/',
                                      lambda base: ({'behind_by': 2, 'ahead_by': 3}, 3))
            self.assertEqual(asked, ['https://site/sheet/sheet.json', 'https://site/sheet/after/dog.png', 'https://site/sheet/after/editor.png'])
            self.assertEqual([(row['label'], row['verdict']) for row in rows], [('Stitches', 'changed'), ('Thread changes', 'worse')])
            self.assertEqual((root / 'out/before/dog.png').read_bytes(), b'main png')
            page = (root / 'out/index.html').read_text()
            self.assertIn('<td class="worse"><span class="was">2 →</span> 5 ▲ worse</td>', page)
            self.assertIn('<td class="changed"><span class="was">10 →</span> 12</td>', page)
            self.assertIn('src="before/dog.png"', page)
            self.assertIn('src="after/patch.png"', page)
            self.assertIn('patch on main: no picture', page)
            self.assertIn('2 commits this branch lacks', page)
            self.assertIn('beside main at <code>bbbbbbbbbb</code>', page)
            lines = json.loads((root / 'out/comment.json').read_text())
            self.assertIn('| dog | Thread changes | 2 | 5 |', lines)

    def test_main_without_a_sheet_leaves_the_pull_request_s_alone(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            work = self.folder(root, [picture('dog', 12, 5, 0.0)])

            def get(url, token=None):
                raise sheet.urllib.error.URLError('404')

            with unittest.mock.patch.object(sheet, 'get', get):
                rows = sheet.assemble(work, root / 'out', 'c' * 40, 'pr/7', 'https://site/sheet/')
            self.assertEqual(rows, [])
            self.assertIn('Main has published no sheet yet', (root / 'out/index.html').read_text())
            self.assertFalse((root / 'out/before').exists())


if __name__ == '__main__':
    unittest.main()
