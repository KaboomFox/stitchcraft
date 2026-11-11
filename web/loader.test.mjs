import {test} from 'node:test';
import assert from 'node:assert/strict';
import {installPlugins} from './loader.mjs';

test('installs bytes sequentially using relative URLs, including large modules', async () => {
  const bytes = new Uint8Array(100000).fill(42);
  const calls = [];
  const api = {execute: async (command, params) => {
    assert.equal(command, 'plugin.install');
    assert.deepEqual(Buffer.from(params.dataBase64, 'base64'), Buffer.from(bytes));
    calls.push(params.name);
    return {id: params.name};
  }};
  const result = await installPlugins(api, ['stroke', 'pes'], async url => {
    assert.match(url.pathname, /\/plugins\/(stroke|pes)\.wasm$/);
    return new Response(bytes);
  });
  assert.deepEqual(calls, ['stroke.wasm', 'pes.wasm']);
  assert.equal(result.length, 2);
});

test('download failure stops installation and exposes the failed module', async () => {
  let installed = false;
  await assert.rejects(installPlugins({execute: async () => { installed = true; }}, ['fill'],
    async () => new Response('', {status: 404})), /fill: HTTP 404/);
  assert.equal(installed, false);
});

test('host rejection is not reported as success or skipped', async () => {
  await assert.rejects(installPlugins({execute: async () => {throw new Error('invalid module');}},
    ['auto'], async () => new Response(new Uint8Array([0]))), /invalid module/);
});
