"""Why this file exists: a preview must hold its book and its own plug-ins, and never the editor, whose size
is why it is published once (web/site.py). A pin that drifts from the gate's is caught here too."""
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest
import unittest.mock

WEB = Path(__file__).resolve().parent


def load(name):
    spec = importlib.util.spec_from_file_location(name, WEB / f'{name}.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


site, build = load('site'), load('build')


class SiteTest(unittest.TestCase):
    def folders(self, root):
        book, demo, out = root / 'book', root / 'demo', root / 'site'
        for path in (book / 'demo', demo / 'plugins', out):
            path.mkdir(parents=True)
        (book / 'index.html').write_text('<a href="demo/index.html">Open the browser demo</a>')
        (book / 'demo/index.html').write_text('the page that opens the editor')
        (demo / 'plugins.json').write_text('{}')
        (demo / 'plugins/fill.wasm').write_bytes(b'module')
        (out / 'old.wasm').write_bytes(b'left by an earlier build')
        return book, demo, out

    def test_a_preview_is_the_book_with_the_demo_folder_beside_its_page(self):
        with tempfile.TemporaryDirectory() as folder:
            book, demo, out = self.folders(Path(folder))
            site.assemble(book, demo, out)
            self.assertEqual((out / 'index.html').read_text(), (book / 'index.html').read_text())
            self.assertEqual((out / 'demo/index.html').read_text(), 'the page that opens the editor')
            self.assertEqual((out / 'demo/plugins/fill.wasm').read_bytes(), b'module')
            self.assertFalse((out / 'old.wasm').exists())
            self.assertTrue((out / '.nojekyll').exists())

    def test_a_preview_never_carries_the_editor(self):
        for name in ('index.html', 'vectorcraft-web-0123_bg.wasm'):
            with tempfile.TemporaryDirectory() as folder:
                book, demo, out = self.folders(Path(folder))
                (demo / name).write_bytes(b'the editor')
                with self.assertRaisesRegex(ValueError, 'carries no editor'):
                    site.assemble(book, demo, out)

    def test_a_book_without_the_page_that_opens_the_editor_is_refused(self):
        with tempfile.TemporaryDirectory() as folder:
            book, demo, out = self.folders(Path(folder))
            (book / 'demo/index.html').unlink()
            with self.assertRaisesRegex(ValueError, 'Missing'):
                site.assemble(book, demo, out)


class BuildTest(unittest.TestCase):
    def test_every_feature_of_the_plug_in_is_a_module_and_the_heavy_ones_are_downloads(self):
        light, heavy = build.modules()
        features = list(tomllib.loads((WEB.parent / 'apps/stitchcraft-vc-plugin/Cargo.toml').read_text())['features'])
        self.assertEqual(sorted(light + heavy), sorted(features))
        self.assertEqual(heavy, ['stroke-to-satin', 'route-satins', 'route-runs'])
        self.assertIn('auto', light)

    def test_the_demo_s_release_is_recorded_beside_the_gate_s_and_is_never_older(self):
        pins = tomllib.loads(build.PINS.read_text())
        repository, tag, revision = build.pin()
        self.assertEqual(repository, pins['repository'])
        self.assertRegex(revision, r'^[0-9a-f]{40}$')
        number = lambda text: tuple(int(part) for part in text.removeprefix('v').split('.'))
        # When the stable track reaches this release, both must name it: set [web] to the stable tag and its
        # commit in the pull request that moves the stable pin.
        self.assertGreaterEqual(number(tag), number(pins['tracks']['stable']['tag']))

    def test_a_checkout_the_cache_s_cleaning_emptied_is_cloned_again(self):
        _, tag, revision = build.pin()
        calls = []

        def ran(*args, cwd=None, **more):
            calls.append(args[:2])
            if args[1] == 'clone':
                subprocess.run(('git', 'init', '-q', args[-1]), check=True)
            return subprocess.CompletedProcess(args, 0, stdout=f'{revision}\n')

        with tempfile.TemporaryDirectory() as temp, unittest.mock.patch.object(build, 'run', ran):
            # The folder around is a repository, as the one around the editor's checkout is.
            subprocess.run(('git', 'init', '-q', temp), check=True)
            folder = Path(temp) / 'target' / 'vectorcraft-web'
            # The cleaning leaves folders, an empty `.git` among them.
            (folder / '.git').mkdir(parents=True)
            (folder / 'crates').mkdir()
            self.assertFalse(build.whole(folder))
            build.checkout(folder)
            self.assertEqual(calls, [('git', 'clone'), ('git', 'rev-parse')])
            self.assertFalse((folder / 'crates').exists())
            # A whole checkout is kept as it is.
            self.assertTrue(build.whole(folder))
            build.checkout(folder)
            self.assertEqual(calls[2:], [('git', 'rev-parse')])

    def test_the_editor_s_page_gets_our_title_and_start_script_or_the_build_fails(self):
        html = '<head><title>VectorCraft</title></head><body><canvas></canvas></body>'
        made = build.page(html, 'abc')
        self.assertIn('<title>StitchCraft in VectorCraft</title>', made)
        self.assertIn('<script type="module" src="./start.mjs?v=abc"></script>\n</body>', made)
        self.assertNotIn('ArtCraft', made)
        for broken in ('<body></body>', '<title>VectorCraft</title>', html + html):
            with self.assertRaises(ValueError):
                build.page(broken, 'abc')

    def test_the_demo_folder_holds_each_module_the_scripts_the_starter_and_our_licences(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            built, out = root / 'built', root / 'demo'
            built.mkdir()
            light, heavy = build.modules()
            for name in light + heavy:
                (built / f'{name}.wasm').write_bytes(name.encode())
            build.demo(out, built, 'v9.9.9', 'f' * 40)
            listed = build.json.loads((out / 'plugins.json').read_text())
            self.assertEqual(listed, {'plugins': light, 'optionalPlugins': heavy, 'version': build.version(WEB),
                                      'vectorcraft': {'tag': 'v9.9.9', 'revision': 'f' * 40}})
            self.assertRegex(listed['version'], r'^[0-9a-f]{16}$')
            names = sorted(str(path.relative_to(out)) for path in out.rglob('*') if path.is_file())
            self.assertEqual(names, sorted(
                [f'plugins/{name}.wasm' for name in light + heavy] + list(build.SCRIPTS)
                + ['plugins.json', 'templates/alpine-night.svg', 'templates/alpine-night.vctemplate']
                + [f'StitchCraft-{name}.txt' for name in ('LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE')]))
            self.assertEqual((out / 'plugins/auto.wasm').read_bytes(), b'auto')
            # The app is the editor with the demo folder's files, our page and VectorCraft's licence texts.
            editor, vectorcraft, app = root / 'editor', root / 'vectorcraft', root / 'app'
            editor.mkdir()
            vectorcraft.mkdir()
            (editor / 'index.html').write_text('<title>VectorCraft</title><body></body>')
            (editor / 'editor_bg.wasm').write_bytes(b'editor')
            (editor / '.trunk-lock').write_text('')
            for name in ('LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE'):
                (vectorcraft / name).write_text(f'VectorCraft {name}')
            build.app(app, editor, out, vectorcraft)
            self.assertFalse((app / '.trunk-lock').exists())
            self.assertEqual((app / 'editor_bg.wasm').read_bytes(), b'editor')
            self.assertEqual((app / 'plugins.json').read_text(), (out / 'plugins.json').read_text())
            self.assertEqual((app / 'VectorCraft-NOTICE.txt').read_text(), 'VectorCraft NOTICE')
            self.assertTrue((app / 'StitchCraft-NOTICE.txt').is_file())
            self.assertIn('./start.mjs?v=', (app / 'index.html').read_text())
            self.assertTrue((app / 'start.mjs').is_file())
            table = build.sizes(app)
            self.assertIn('| **all files** |', table)

    def test_a_change_to_the_drawing_or_its_finished_example_names_another_build(self):
        hashed = [*build.SCRIPTS, 'start.mjs', 'templates/alpine-night.svg', 'templates/alpine-night.vctemplate']
        with tempfile.TemporaryDirectory() as temp:
            folder = Path(temp)
            (folder / 'templates').mkdir()
            for name in hashed:
                (folder / name).write_bytes((WEB / name).read_bytes())
            self.assertEqual(build.version(folder), build.version(WEB))
            for name in hashed:
                kept = (folder / name).read_bytes()
                (folder / name).write_bytes(kept + b' ')
                self.assertNotEqual(build.version(folder), build.version(WEB), name)
                (folder / name).write_bytes(kept)

    def test_a_file_github_would_refuse_fails_the_build(self):
        with tempfile.TemporaryDirectory() as folder:
            (Path(folder) / 'small.bin').write_bytes(b'x' * 1000 * 1000)
            self.assertIn('| `small.bin` | 1,000,000 |', build.sizes(Path(folder)))
            with unittest.mock.patch.object(build, 'LARGEST_FILE', 1000 * 1000):
                with self.assertRaisesRegex(ValueError, 'GitHub refuses'):
                    build.sizes(Path(folder))


if __name__ == '__main__':
    unittest.main()
