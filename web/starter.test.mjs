import {test} from 'node:test';
import assert from 'node:assert/strict';
import {openStarter} from './starter.mjs';

test('opens a native template in an empty session and fits the view', async () => {
  const calls = [];
  const api = {uiInspect: async () => ({documents: []}), execute: async (...args) => calls.push(args)};
  assert.equal(await openStarter(api, {onlyIfEmpty: true, fetchFile: async () => new Response('template')}), true);
  assert.deepEqual(calls.map(c => c[0]), ['document.open', 'view.fitArtboard', 'select.all']);
  assert.equal(calls[0][1].name, 'Alpine Night.vctemplate');
  assert.equal(atob(calls[0][1].dataBase64), 'template');
});

test('leaves existing documents, recovery dialogs and pending opens alone', async () => {
  for (const state of [{documents: [{}]}, {documents: [], ui: {dialog: {kind: 'recovery'}}}, {documents: [], opening: ['art.svg']}]) {
    const api = {uiInspect: async () => state};
    assert.equal(await openStarter(api, {onlyIfEmpty: true, fetchFile: () => assert.fail('must not fetch')}), false);
  }
});

test('does not switch documents if a file was opened during download', async () => {
  let queried = 0;
  const api = {uiInspect: async () => ({documents: queried++ ? [{}] : []}), execute: () => assert.fail('must not open')};
  assert.equal(await openStarter(api, {onlyIfEmpty: true, fetchFile: async () => new Response('template')}), false);
});

test('missing template reports a visible error', async () => {
  await assert.rejects(openStarter({}, {fetchFile: async () => new Response('', {status: 404})}), /Starter patch: HTTP 404/);
});
