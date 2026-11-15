// Why this file exists: a parity check that reports the wrong byte, or a limit that never fails, would pass
// in CI and say nothing (web/parity.mjs).
import assert from 'node:assert/strict';
import test from 'node:test';
import {difference, firstDifference, label, middle, overLimit} from './parity.mjs';

test('the same files have no first difference, and others have it at the first byte that differs', () => {
  assert.equal(firstDifference(Buffer.from([1, 2, 3]), Buffer.from([1, 2, 3])), -1);
  assert.equal(firstDifference(Buffer.from([1, 2, 3]), Buffer.from([1, 9, 3])), 1);
  assert.equal(firstDifference(Buffer.from([9, 2, 3]), Buffer.from([1, 2, 3])), 0);
  assert.equal(firstDifference(Buffer.from([1, 2]), Buffer.from([1, 2, 3])), 2);
  assert.equal(firstDifference(Buffer.from([1, 2, 3]), Buffer.from([1, 2])), 2);
  assert.equal(firstDifference(Buffer.from([]), Buffer.from([])), -1);
});

test('a difference names the file, the byte and what each side has there', () => {
  assert.equal(difference('dog.pes', Buffer.from([1, 2]), Buffer.from([1, 2])), null);
  assert.equal(difference('dog.pes', Buffer.from([1, 0x0a, 3]), Buffer.from([1, 0xff])),
    'dog.pes: the editor\'s file (3 bytes) and stitch\'s (2 bytes) first differ at byte 1: the editor has 0x0a, stitch has 0xff');
  assert.match(difference('dog.dst', Buffer.from([1]), Buffer.from([1, 2])), /at byte 1: the editor has nothing, the file ends, stitch has 0x02$/);
});

test('a DST header gives the design\'s name', () => {
  assert.equal(label(Buffer.from('LA:dog-on-turf-6   \rST:   1234\r')), 'dog-on-turf-6');
  assert.equal(label(Buffer.from('LA:Alpine Night    \rST:')), 'Alpine Night');
  assert.equal(label(Buffer.from('#PES0001')), '');
});

test('the middle of an odd count is its median, whatever the order', () => {
  assert.equal(middle([30, 10, 20]), 20);
  assert.equal(middle([5]), 5);
  assert.equal(middle([1, 2, 3, 4]), 3);
});

test('times fail only above a limit that is set', () => {
  const runs = [{turn_ms: 900, export_ms: 50}, {turn_ms: 1000, export_ms: 70}, {turn_ms: 5000, export_ms: 60}];
  assert.deepEqual(overLimit(runs, {turn: null, export: null}), []);
  assert.deepEqual(overLimit(runs, {turn: 1000, export: 60}), []);
  assert.deepEqual(overLimit(runs, {turn: 999, export: 60}), ['giving the shapes the Auto effect took 1000 ms, the middle of 3 runs, above the limit of 999 ms']);
  assert.deepEqual(overLimit(runs, {turn: 1000, export: 59}), ['the PES export took 60 ms, the middle of 3 runs, above the limit of 59 ms']);
});
