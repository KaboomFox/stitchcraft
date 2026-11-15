// Why this file exists: the panel's exports run the exporters' modules in the browser, and the call that
// runs one must give it what VectorCraft gives it. These tests run the built modules as the page does
// (web/engine.mjs). The modules are the ones `web/build.py` built, or those in the folder
// `STITCHCRAFT_MODULES` names. Without them the tests that need one are skipped, but never in CI.
import assert from 'node:assert/strict';
import {existsSync} from 'node:fs';
import {readFile} from 'node:fs/promises';
import test from 'node:test';
import {FILE, PICTURE, decodeBase64, exportParams, exportWith} from './engine.mjs';

const root = new URL('..', import.meta.url);
const modules = process.env.STITCHCRAFT_MODULES ?? new URL('target/web/modules', root).pathname;
const built = existsSync(`${modules}/pes.wasm`);
if (!built) assert.ok(!process.env.CI, `CI builds the modules before these tests, and ${modules}/pes.wasm is missing`);
const skip = built ? false : 'the modules are not built here';
const traced = () => readFile(new URL('docs/fixtures/traced-dog-on-turf-6.vectorcraft', root));

test('a run gets the dialog\'s defaults, what is set, and the file\'s context', () => {
  const manifest = {params: {auto: {type: 'bool', default: false}, profile: {type: 'choice', default: 'document'}}};
  assert.deepEqual(exportParams(manifest, {auto: true}, 'dst', 'Dog'),
    {auto: true, profile: 'document', _context: {mode: 'export', extension: 'dst', name: 'Dog'}});
  assert.deepEqual(exportParams({}, {}, 'pes', 'Dog', PICTURE), {_context: {mode: 'picture', extension: 'pes', name: 'Dog'}});
  assert.equal(FILE, 'export');
});

test('base64 decodes to its bytes', () => {
  assert.deepEqual([...decodeBase64('AAEC/w==')], [0, 1, 2, 255]);
  assert.equal(decodeBase64('').length, 0);
});

test('a traced photo, too large for VectorCraft\'s export, is written as PES and as DST with its name', {skip}, async () => {
  const document = await traced();
  const pes = await exportWith(await readFile(`${modules}/pes.wasm`), document, 'pes', 'Dog', {auto: true});
  assert.equal(Buffer.from(pes.bytes.subarray(0, 8)).toString(), '#PES0001');
  assert.ok(pes.bytes.length > 10000, `${pes.bytes.length} bytes of PES`);
  assert.ok(pes.warnings.some(line => line.startsWith('info SC-I0901: ')), pes.warnings.join('\n'));
  // A compiled module runs too, and the same document gives the same file.
  const again = await exportWith(await WebAssembly.compile(await readFile(`${modules}/pes.wasm`)), document, 'pes', 'Dog', {auto: true});
  assert.deepEqual(again.bytes, pes.bytes);
  const dst = await exportWith(await readFile(`${modules}/dst.wasm`), document, 'dst', 'Dog', {auto: true});
  assert.equal(Buffer.from(dst.bytes.subarray(0, 20)).toString('latin1'), 'LA:Dog             \r');
});

test('asked for the picture, the exporter answers a PNG', {skip}, async () => {
  const picture = await exportWith(await readFile(`${modules}/pes.wasm`), await traced(), 'pes', 'Dog', {auto: true}, PICTURE);
  assert.deepEqual([...picture.bytes.subarray(0, 8)], [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
});

test('what the exporter declines with is thrown', {skip}, async () => {
  // A document without a shape has nothing to sew, and bytes that are no document can't be read.
  const empty = new TextEncoder().encode(JSON.stringify({format: 'vectorcraft', version: 4, document: {version: 1, artboards: [], next_id: 2, layers: []}}));
  await assert.rejects(exportWith(await readFile(`${modules}/pes.wasm`), empty, 'pes', 'Dog'), /^Error: error SC-E0010: /);
  await assert.rejects(exportWith(await readFile(`${modules}/pes.wasm`), new TextEncoder().encode('not a document'), 'pes', 'Dog'), /^Error: error SC-E/);
});
