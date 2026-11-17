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

    def test_a_font_built_into_the_exporters_is_published_with_its_licence(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            built, out, fonts, listed = root / 'built', root / 'demo', root / 'fonts', root / 'list.txt'
            built.mkdir()
            light, heavy = build.modules()
            for name in light + heavy:
                (built / f'{name}.wasm').write_bytes(name.encode())
            for name, licence in (('plain', 'LICENSE'), ('other', 'licence.txt'), ('bare', None)):
                (fonts / name).mkdir(parents=True)
                (fonts / name / 'font.json').write_text('{}')
                if licence:
                    (fonts / name / licence).write_text(f'the licence of {name}')
            listed.write_text('# The fonts built in.\nplain\n\nother\n')
            # Without a folder of fonts none is built in, and the folder is as it was before fonts.
            self.assertEqual(build.fonts(None, listed), [])
            self.assertEqual(build.fonts('', listed), [])
            found = build.fonts(fonts, listed)
            self.assertEqual(found, [('plain', fonts / 'plain/LICENSE'), ('other', fonts / 'other/licence.txt')])
            build.demo(out, built, 'v9.9.9', 'f' * 40, found)
            self.assertEqual((out / 'fonts/plain/LICENSE').read_text(), 'the licence of plain')
            self.assertEqual((out / 'fonts/other/licence.txt').read_text(), 'the licence of other')
            self.assertEqual(build.json.loads((out / 'plugins.json').read_text())['fonts'], [
                {'folder': 'plain', 'licence': 'fonts/plain/LICENSE'}, {'folder': 'other', 'licence': 'fonts/other/licence.txt'}])
            # A font without a licence file is never published.
            listed.write_text('bare\n')
            with self.assertRaisesRegex(ValueError, 'the font bare has no licence file'):
                build.fonts(fonts, listed)
            # The plug-in's own list names fonts, the default first.
            named = [line for line in build.FONTS.read_text().splitlines() if line and not line.startswith('#')]
            self.assertEqual(named, ['small_font', 'barstitch_bold'])

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

    def test_the_kept_editor_has_another_name_when_anything_it_is_built_from_changes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            pins = root / 'pins.toml'
            pins.write_text('repository = "r"\n[web]\ntag = "v1.0.0"\nrevision = "0a1"\n')
            for name in build.EDITOR_INPUTS:
                (root / name).parent.mkdir(parents=True, exist_ok=True)
                (root / name).write_text(name)
            tools = root / build.TOOLS
            tools.parent.mkdir(parents=True)
            listed = '  [mdbook]=0.5.4\n  [trunk]=0.22.1\n  [wasm-bindgen]=0.2.129\n  [wasm-opt]=version_131\n'
            tools.write_text(listed)
            key = build.editor_key(pins, root)
            self.assertRegex(key, r'^[0-9a-f]{64}$')
            self.assertEqual(build.editor_key(pins, root), key)
            for name in build.EDITOR_INPUTS:
                (root / name).write_text(name + ' ')
                self.assertNotEqual(build.editor_key(pins, root), key, name)
                (root / name).write_text(name)
            for old, new in (('"r"', '"s"'), ('v1.0.0', 'v1.0.1'), ('0a1', '0a2')):
                pins.write_text(pins.read_text().replace(old, new))
                self.assertNotEqual(build.editor_key(pins, root), key, new)
                pins.write_text(pins.read_text().replace(new, old))
            self.assertEqual(build.editor_key(pins, root), key)
            # A version of a tool that builds the editor names another editor, and another tool's doesn't.
            for old, new in (('0.22.1', '0.22.2'), ('0.2.129', '0.2.130'), ('version_131', 'version_132')):
                tools.write_text(listed.replace(old, new))
                self.assertNotEqual(build.editor_key(pins, root), key, new)
            tools.write_text(listed.replace('0.5.4', '0.5.5') + '  [cargo-nextest]=0.9.151\n')
            self.assertEqual(build.editor_key(pins, root), key)
            tools.write_text(listed.replace('  [trunk]=0.22.1\n', ''))
            with self.assertRaises(KeyError):
                build.editor_key(pins, root)
        # The compiler and this script are among the files, and the workflow's action pins each tool.
        self.assertEqual(set(build.EDITOR_INPUTS), {'rust-toolchain.toml', 'web/build.py'})
        self.assertTrue(all((build.ROOT / name).is_file() for name in build.EDITOR_INPUTS))
        self.assertRegex(build.editor_key(), r'^[0-9a-f]{64}$')

    def test_an_editor_is_kept_whole_under_its_key_and_used_under_no_other(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            dist, vectorcraft, kept = root / 'dist', root / 'vectorcraft', root / 'editor'
            dist.mkdir()
            vectorcraft.mkdir()
            (dist / 'index.html').write_text('the page')
            (dist / '.trunk-lock').write_text('')
            for name in build.LICENCES:
                (vectorcraft / name).write_text(f'VectorCraft {name}')
            self.assertFalse(build.kept_editor(kept, 'k1'))
            (kept / 'dist').mkdir(parents=True)
            (kept / 'dist/old.wasm').write_text('an older build')
            build.keep_editor(kept, 'k1', dist, vectorcraft, 'v1.0.0', '0a1')
            self.assertTrue(build.kept_editor(kept, 'k1'))
            self.assertFalse(build.kept_editor(kept, 'k2'))
            self.assertEqual(build.json.loads((kept / 'built.json').read_text()), {'key': 'k1', 'tag': 'v1.0.0', 'revision': '0a1'})
            names = sorted(str(path.relative_to(kept)) for path in kept.rglob('*') if path.is_file())
            self.assertEqual(names, sorted(['built.json', 'dist/index.html', *build.LICENCES]))
            # A folder that lost a file, or whose key's file is no JSON object, is built again.
            (kept / 'NOTICE').unlink()
            self.assertFalse(build.kept_editor(kept, 'k1'))
            (kept / 'NOTICE').write_text('x')
            self.assertTrue(build.kept_editor(kept, 'k1'))
            (kept / 'dist/index.html').unlink()
            self.assertFalse(build.kept_editor(kept, 'k1'))
            (kept / 'dist/index.html').write_text('x')
            for text in ('', '[]', '"k1"'):
                (kept / 'built.json').write_text(text)
                self.assertFalse(build.kept_editor(kept, 'k1'), text)

    def test_the_workflow_keeps_the_editor_under_the_scripts_key(self):
        workflow = (build.ROOT / '.github/workflows/web.yml').read_text()
        self.assertIn('python3 -I web/build.py --editor-key', workflow)
        # Restored and saved at the folder the script keeps the editor in, under one key.
        self.assertEqual(workflow.count('path: target/web/editor\n'), 2)
        self.assertEqual(workflow.count('key: ${{ steps.editor-key.outputs.key }}\n'), 2)
        self.assertFalse('restore-keys:' in workflow)


if __name__ == '__main__':
    unittest.main()
