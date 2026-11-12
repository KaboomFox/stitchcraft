"""Exercise publication against a real bare Git remote without GitHub credentials."""
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('publisher', Path(__file__).resolve().parents[1] / '.github/scripts/publish-web.py')
publisher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publisher)


class PublishTest(unittest.TestCase):
    def test_previews_preserve_docs_and_each_other(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            remote, repo, site = root / 'remote.git', root / 'repo', root / 'site'
            subprocess.run(['git', 'init', '--bare', str(remote)], check=True, capture_output=True)
            repo.mkdir()
            publisher.git('init', cwd=repo)
            publisher.git('remote', 'add', 'origin', str(remote), cwd=repo)
            credentials = root / 'test-credentials.config'
            credentials.write_text('[http "https://github.com/"]\n    extraheader = TEST-ONLY\n')
            publisher.git('config', '--local', f'includeIf.gitdir:{repo}/.git.path', str(credentials), cwd=repo)
            site.mkdir()
            (site / 'index.html').write_text('first')
            previous = Path.cwd()
            try:
                os.chdir(repo)
                publisher.publish('pr/1', None)
                self.assertFalse(publisher.git('ls-remote', '--heads', 'origin', cwd=repo).stdout.strip())
                original_git = publisher.git
                raced = False

                def concurrent_git(*args, **kwargs):
                    nonlocal raced
                    if args[0] == 'push':
                        header = original_git('config', '--local', '--get', 'http.https://github.com/.extraheader', cwd=kwargs['cwd'], check=False)
                        self.assertEqual(header.stdout.strip(), 'TEST-ONLY', 'checkout v7 included credential must reach the publishing checkout')
                    if args[0] == 'push' and not raced:
                        raced = True
                        publisher.publish('pr/99', str(site))
                    return original_git(*args, **kwargs)

                with patch.object(publisher, 'git', concurrent_git):
                    publisher.publish('pr/1', str(site))
                self.assertTrue(raced)
                publisher.git('fetch', 'origin', 'gh-pages', cwd=repo)
                publisher.git('checkout', '-B', 'docs', 'FETCH_HEAD', cwd=repo)
                (repo / 'dev').mkdir()
                (repo / 'dev/index.html').write_text('documentation')
                publisher.git('config', 'user.name', 'Test', cwd=repo)
                publisher.git('config', 'user.email', 'test@example.invalid', cwd=repo)
                publisher.git('add', 'dev', cwd=repo)
                publisher.git('commit', '-m', 'docs', cwd=repo)
                publisher.git('push', 'origin', 'HEAD:gh-pages', cwd=repo)
                (site / 'index.html').write_text('second')
                publisher.publish('pr/2', str(site))
                publisher.publish('app', str(site))
                publisher.publish('dev', str(site))
                publisher.publish('site-root', str(site))
                publisher.publish('pr/1', None)
                publisher.git('fetch', 'origin', 'gh-pages', cwd=repo)
                tree = publisher.git('ls-tree', '-r', '--name-only', 'FETCH_HEAD', cwd=repo).stdout
                self.assertNotIn('pr/1/', tree)
                self.assertIn('pr/2/index.html', tree)
                self.assertIn('pr/99/index.html', tree)
                self.assertIn('app/index.html', tree)
                self.assertIn('dev/index.html', tree)
                self.assertIn('index.html', tree)
                self.assertEqual(publisher.git('show', 'FETCH_HEAD:dev/index.html', cwd=repo).stdout, 'second')
                with self.assertRaises(ValueError):
                    publisher.publish('dev', None)
                self.assertIn('.nojekyll', tree)
                # A repeated cleanup is a no-op, and traversal is refused.
                publisher.publish('pr/1', None)
                with self.assertRaises(ValueError):
                    publisher.publish('pr/../dev', None)
            finally:
                os.chdir(previous)


if __name__ == '__main__':
    unittest.main()
