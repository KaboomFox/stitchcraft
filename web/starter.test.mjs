// Why this file exists: the starter must never replace a visitor's document, and the button must give each
// shape the Auto effect once (web/starter.mjs).
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {AUTO, openFinished, openStarter, sews, turnIntoStitches} from './starter.mjs';

const file = text => async () => new Response(text);

test('an empty session opens the plain drawing from its SVG, fits the view and selects nothing', async () => {
  const calls = [];
  const asked = [];
  const api = {uiInspect: async () => ({documents: []}), execute: async (...args) => {
    calls.push(args);
    return args[0] === 'document.open' ? {index: 0, format: 'svg', warnings: []} : undefined;
  }};
  const opened = await openStarter(api, {onlyIfEmpty: true, fetchFile: async url => {asked.push(url.pathname); return new Response('<svg/>');}});
  assert.deepEqual(opened, {index: 0, format: 'svg', warnings: []});
  assert.deepEqual(calls.map(c => c[0]), ['document.open', 'view.fitArtboard']);
  assert.equal(calls[0][1].name, 'Alpine Night.svg');
  assert.equal(atob(calls[0][1].dataBase64), '<svg/>');
  assert.equal(asked.length, 1);
  assert.ok(asked[0].endsWith('/templates/alpine-night.svg'));
});

test('the finished example opens the template, also beside a document', async () => {
  const calls = [];
  const asked = [];
  const api = {uiInspect: () => assert.fail('does not ask whether the session is empty'), execute: async (...args) => {calls.push(args);}};
  assert.equal(await openFinished(api, {fetchFile: async url => {asked.push(url.pathname); return new Response('template');}}), true);
  assert.deepEqual(calls.map(c => c[0]), ['document.open', 'view.fitArtboard']);
  assert.equal(calls[0][1].name, 'Alpine Night.vctemplate');
  assert.equal(atob(calls[0][1].dataBase64), 'template');
  assert.ok(asked[0].endsWith('/templates/alpine-night.vctemplate'));
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
  assert.equal(await openStarter(api, {onlyIfEmpty: true, fetchFile: file('<svg/>')}), false);
});

test('a missing file reports a visible error that names it', async () => {
  const missing = async () => new Response('', {status: 404});
  await assert.rejects(openStarter({}, {fetchFile: missing}), /^Error: Alpine Night\.svg: HTTP 404$/);
  await assert.rejects(openFinished({}, {fetchFile: missing}), /^Error: Alpine Night\.vctemplate: HTTP 404$/);
});

// An editor of 3 shapes, as `effect.list` reports them: 7 has the Auto effect, 8 a satin effect on its fill.
function editor(applied, documents = [{}]) {
  const calls = [];
  return {calls, uiInspect: async () => ({documents}), execute: async (...args) => {
    calls.push(args);
    return args[0] === 'effect.list' ? {catalog: [], applied} : {};
  }};
}
const auto = [{id: AUTO, params: {}}];

test('the button selects everything and gives each plain shape the Auto effect, once', async () => {
  const api = editor([
    {id: 5, effects: [], items: [{index: 0, kind: 'fill', effects: []}]},
    {id: 6, effects: [{id: 'stylize.dropShadow'}], items: []},
    {id: 7, effects: auto, items: []},
    {id: 8, effects: [], items: [{index: 0, kind: 'fill', effects: [{id: 'plugin.dev.stitchcraft.satin'}]}]},
  ]);
  assert.deepEqual(await turnIntoStitches(api), {shapes: 4, changed: 2});
  assert.deepEqual(api.calls, [
    ['select.all', {}], ['effect.list', {}],
    ['effect.apply', {effect: 'plugin.dev.stitchcraft.auto', params: {}, item: null, ids: [5, 6]}],
    ['select.none', {}],
  ]);
});

test('a second press adds no effect', async () => {
  const api = editor([{id: 5, effects: auto, items: []}, {id: 6, effects: auto}]);
  assert.deepEqual(await turnIntoStitches(api), {shapes: 2, changed: 0});
  assert.deepEqual(api.calls.map(c => c[0]), ['select.all', 'effect.list', 'select.none']);
});

test('without a document or a shape the button says so and applies nothing', async () => {
  const none = editor([], []);
  await assert.rejects(turnIntoStitches(none), /^Error: Open a drawing first\.$/);
  assert.deepEqual(none.calls, []);
  const bare = editor([]);
  await assert.rejects(turnIntoStitches(bare), /no shapes to turn into stitches/);
  assert.deepEqual(bare.calls.map(c => c[0]), ['select.all', 'effect.list']);
  const unlisted = {uiInspect: async () => ({documents: [{}]}), execute: async () => ({})};
  await assert.rejects(turnIntoStitches(unlisted), /no shapes to turn into stitches/);
});

test('a shape sews when it, its fill or its stroke has a StitchCraft effect', () => {
  assert.equal(sews({id: 1}), false);
  assert.equal(sews({id: 1, effects: [], items: [{effects: []}, {}]}), false);
  assert.equal(sews({id: 1, effects: [{id: 'plugin.dev.other.auto'}, {id: 'warp.arc', params: {note: 'dev.stitchcraft'}}]}), false);
  assert.equal(sews({id: 1, effects: auto}), true);
  assert.equal(sews({id: 1, effects: [], items: [{effects: []}, {effects: [{id: 'plugin.dev.stitchcraft.fill'}]}]}), true);
});
