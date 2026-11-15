// Why this file exists: the panel's export buttons take the open document from the editor and hand it to a
// worker, and what they ask of each must hold without a browser (web/exports.mjs).
import assert from 'node:assert/strict';
import test from 'node:test';
import {MAKES, done, fileName, make, savedDocument, seconds, titleOf} from './exports.mjs';

const saved = title => new TextEncoder().encode(JSON.stringify({format: 'vectorcraft', document: {title}}));
// An editor with `documents` open, whose document in front is saved as `bytes`. It records the calls.
const editor = (bytes, documents = 1) => {
  const calls = [];
  return {
    calls,
    uiInspect: async () => ({documents: Array.from({length: documents}, () => ({title: 'x'}))}),
    execute: async (command, params) => {
      calls.push([command, params]);
      return {dataBase64: Buffer.from(bytes).toString('base64')};
    },
  };
};
// A worker that answers `answer` to the message it is sent, and records the message.
const worker = (answer, sent) => () => ({
  postMessage(message) {
    sent.push(message);
    queueMicrotask(() => this.onmessage({data: answer}));
  },
  terminate() { sent.push('terminated'); },
});

test('a file is named by the document\'s title, without a drawing\'s extension or a character a name can\'t have', () => {
  assert.equal(fileName('Alpine Night.svg'), 'Alpine Night');
  assert.equal(fileName('dog-on-turf-6.vectorcraft'), 'dog-on-turf-6');
  assert.equal(fileName('Untitled-1'), 'Untitled-1');
  assert.equal(fileName('a/b:c*d?e"f<g>h|i\\j\u0001k'), 'a-b-c-d-e-f-g-h-i-j-k');
  assert.equal(fileName(`  ${'x'.repeat(70)}`), 'x'.repeat(60));
  for (const none of [undefined, null, '', '   ', '.svg']) assert.equal(fileName(none), 'design');
  assert.equal(titleOf(saved('Dog')), 'Dog');
  assert.equal(titleOf(new TextEncoder().encode('not JSON')), undefined);
  assert.equal(titleOf(new TextEncoder().encode('{}')), undefined);
});

test('the document is the one the editor saves, asked for through the page\'s documented call', async () => {
  const api = editor(saved('Dog'));
  assert.deepEqual(await savedDocument(api), saved('Dog'));
  assert.deepEqual(api.calls, [['document.export', {format: 'vectorcraft'}]]);
  await assert.rejects(savedDocument(editor(saved('Dog'), 0)), /^Error: Open a drawing first\.$/);
});

test('each button asks the worker for its file of the saved document, and the picture is the PES file\'s', async () => {
  for (const [what, extension, mode, name] of [['pes', 'pes', 'export', 'Dog.pes'], ['dst', 'dst', 'export', 'Dog.dst'], ['picture', 'pes', 'picture', 'Dog.png']]) {
    const sent = [];
    const times = [1000, 4500];
    const out = await make(editor(saved('Dog.svg')), what, {makeWorker: worker({bytes: new Uint8Array([1, 2]), warnings: ['a']}, sent), now: () => times.shift()});
    assert.deepEqual({...out, document: undefined}, {name, bytes: new Uint8Array([1, 2]), warnings: ['a'], ms: 3500, document: undefined}, what);
    assert.deepEqual(out.document, saved('Dog.svg'));
    assert.equal(sent.length, 2);
    assert.deepEqual({...sent[0], module: new URL(sent[0].module).pathname.split('/').slice(-2).join('/')},
      {module: `plugins/${extension}.wasm`, document: saved('Dog.svg'), extension, name: 'Dog', mode}, what);
    assert.equal(sent[1], 'terminated');
    assert.deepEqual(MAKES[what].mode, mode);
  }
});

test('what the exporter declines with is thrown, and the worker is stopped all the same', async () => {
  const sent = [];
  await assert.rejects(make(editor(saved('Dog')), 'pes', {makeWorker: worker({error: 'error SC-E0010: nothing to sew'}, sent)}), /^Error: error SC-E0010: nothing to sew$/);
  assert.equal(sent.at(-1), 'terminated');
  // A worker that can't start says so.
  const broken = () => ({postMessage() { queueMicrotask(() => this.onerror({message: ''})); }, terminate() {}});
  await assert.rejects(make(editor(saved('Dog')), 'pes', {makeWorker: broken}), /^Error: The engine could not start\.$/);
});

test('the panel is told how long the engine has worked while it works', async () => {
  const told = [];
  const slow = () => ({
    postMessage() { setTimeout(() => this.onmessage({data: {bytes: new Uint8Array(1), warnings: []}}), 350); },
    terminate() {},
  });
  await make(editor(saved('Dog')), 'pes', {makeWorker: slow, working: ms => told.push(ms)});
  assert.ok(told.length >= 2 && told.every((ms, at) => at === 0 || ms > told[at - 1]), `${told}`);
});

test('what is said of a finished file: its name, its size, the time and how much the export said', () => {
  assert.equal(seconds(3449), '3.4 s');
  assert.equal(done({name: 'Dog.pes', bytes: new Uint8Array(24015), warnings: [], ms: 3449}), 'Dog.pes: 24,015 bytes in 3.4 s. The export said nothing.');
  assert.equal(done({name: 'Dog.pes', bytes: new Uint8Array(1), warnings: ['a'], ms: 60}), 'Dog.pes: 1 bytes in 0.1 s. The export said 1 thing, listed below.');
  assert.equal(done({name: 'Dog.pes', bytes: new Uint8Array(1), warnings: ['a', 'b'], ms: 60}), 'Dog.pes: 1 bytes in 0.1 s. The export said 2 things, listed below.');
});
