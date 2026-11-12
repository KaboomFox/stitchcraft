"""The PR docs must contain their own demo, replacing the permanent-docs launcher."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('preview_site', Path(__file__).with_name('site.py'))
site = importlib.util.module_from_spec(spec)
spec.loader.exec_module(site)


class SiteTest(unittest.TestCase):
    def test_docs_and_matching_demo_replace_previous_preview(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            book, demo, out = root / 'book', root / 'app', root / 'site'
            for path in (book / 'demo', demo / 'screenshots', out):
                path.mkdir(parents=True)
            (book / 'index.html').write_text('<a href="demo/index.html">Open the browser demo</a>')
            (book / 'demo/index.html').write_text('redirect to permanent app')
            (demo / 'index.html').write_text('this PR app')
            (demo / 'screenshots/settings.png').write_bytes(b'screenshot')
            (out / 'old.wasm').write_bytes(b'stale app at former preview root')
            site.assemble(book, demo, out)
            self.assertEqual((out / 'index.html').read_text(), (book / 'index.html').read_text())
            self.assertEqual((out / 'demo/index.html').read_text(), 'this PR app')
            self.assertTrue((out / 'demo/screenshots/settings.png').is_file())
            self.assertFalse((out / 'old.wasm').exists())
            self.assertTrue((out / '.nojekyll').exists())
