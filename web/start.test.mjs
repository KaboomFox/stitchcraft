// Why this file exists: the script that chooses a preview's loader runs on the published editor's page, where
// a wrong choice would run another folder's script (web/start.mjs).
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {bootAddress, demoFolder} from './start.mjs';

const page = 'https://example.invalid/stitchcraft/app/?preview=260';
const list = body => async () => new Response(JSON.stringify(body));

test('the app runs its own loader, and a preview its pull request\'s', async () => {
  assert.equal(demoFolder(''), './');
  assert.equal(demoFolder('?webgl'), './');
  assert.equal(demoFolder('?webgl&preview=260'), '../pr/260/demo/');
  assert.equal(await bootAddress('', page, list({version: '0123456789abcdef'})),
    'https://example.invalid/stitchcraft/app/boot.mjs?v=0123456789abcdef');
  const asked = [];
  const address = await bootAddress('?preview=260', page, async (url, options) => {
    asked.push([url.href, options.cache]);
    return new Response(JSON.stringify({version: 'fedcba9876543210'}));
  });
  assert.equal(address, 'https://example.invalid/stitchcraft/pr/260/demo/boot.mjs?v=fedcba9876543210');
  assert.deepEqual(asked, [['https://example.invalid/stitchcraft/pr/260/demo/plugins.json', 'no-store']]);
});

test('only a pull request\'s number names a preview', () => {
  for (const preview of ['0', '01', '-1', '1/../../x', '//evil.example', 'https://evil.example/', '1234567890', '']) {
    assert.throws(() => demoFolder(`?preview=${encodeURIComponent(preview)}`), /No preview is named/, preview);
  }
  assert.equal(demoFolder('?preview=123456789'), '../pr/123456789/demo/');
});

test('a preview that is gone, or names no build, says so', async () => {
  await assert.rejects(bootAddress('?preview=7', page, async () => new Response('', {status: 404})), /No demo at \/stitchcraft\/pr\/7\/demo\/ \(HTTP 404\)/);
  for (const version of [undefined, '', 'x', '0123456789abcdeg', '0123456789abcdef0', '../x']) {
    await assert.rejects(bootAddress('', page, list({version})), /does not name its build/, String(version));
  }
});
