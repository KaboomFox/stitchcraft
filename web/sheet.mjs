// Why this file exists: 3 things about a change only the real editor in a browser shows, and the before and
// after sheet reports them (docs/src/design/before-after-sheet.md, The browser's checks):
//
// - the editor's exporters write the machine files `stitch` writes of the same document, byte for byte;
// - how long StitchCraft's Auto effect takes on a traced photo, on the page's main thread;
// - what a person sees: the traced photo in the editor with its stitches drawn.
//
// `node web/sheet.mjs URL WORK` runs them at the editor's address `URL`, with the `stitch` that
// `web/sheet.py tools WORK` built, and writes `WORK/browser.json` and `WORK/after/editor.png`. A file that
// differs, or a time above its limit, fails the run after the results are written.
import {chromium} from '@playwright/test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdir, readFile, writeFile} from 'node:fs/promises';
import {join} from 'node:path';
import {difference, label, middle, overLimit} from './parity.mjs';

const AUTO = 'plugin.dev.stitchcraft.auto';
const root = new URL('..', import.meta.url).pathname;
// The designs both programs export: the demo's starter drawing, and a traced photo, about a hundred shapes
// with ragged edges. The photo is also the one the times and the picture are taken of.
const DESIGNS = [
  {design: 'alpine-night', file: 'web/templates/alpine-night.svg', name: 'alpine-night.svg'},
  {design: 'dog-on-turf-6', file: 'docs/fixtures/traced-dog-on-turf-6.vectorcraft', name: 'dog-on-turf-6.vectorcraft'},
];
const TIMED = DESIGNS[1];
const RUNS = 3;
// The most the middle of the 3 runs may take, in milliseconds, or null while none is set
// (docs/src/design/before-after-sheet.md, The time's limit, says where the numbers come from).
const LIMIT_MS = {turn: null, export: null};
// How long the editor may take to draw the effects it left waiting before a run is recorded with them left.
const DRAWN_WITHIN_MS = 120000;

const [address, work] = process.argv.slice(2);
assert.ok(address && work, 'usage: node web/sheet.mjs URL WORK');
const {stitch} = JSON.parse(await readFile(join(work, 'build.json'), 'utf8'));
const url = new URL(address);
url.searchParams.set('webgl', '');
const browser = await chromium.launch({channel: 'chromium', args: ['--no-sandbox', '--enable-unsafe-swiftshader']});
const results = {chromium: browser.version(), parity: [], speed: null, editor: null};
const faults = [];
try {
  const page = await browser.newPage({viewport: {width: 1440, height: 1000}});
  const failures = [];
  page.on('pageerror', error => failures.push(error.message));
  await page.goto(url.href, {waitUntil: 'domcontentloaded', timeout: 60000});
  await page.waitForFunction(() => Boolean(window.stitchcraftReady), null, {timeout: 180000});
  await page.evaluate(() => window.stitchcraftReady);

  // Opens a design as a new document and gives every shape the Auto effect, as the demo's button does.
  // Answers how many shapes it has, how long the call that gives them the effect took, how long it took until
  // the editor had drawn every effect, and how many it still had not drawn when the wait ran out.
  const turned = async ({file, name}) => {
    const dataBase64 = (await readFile(join(root, file))).toString('base64');
    return page.evaluate(async ({name, dataBase64, auto, within}) => {
      await vectorcraft.execute('document.open', {name, dataBase64});
      await vectorcraft.execute('view.fitArtboard', {});
      await vectorcraft.execute('select.all', {});
      const {applied = []} = await vectorcraft.execute('effect.list', {});
      const ids = applied.map(shape => shape.id);
      const started = performance.now();
      await vectorcraft.execute('effect.apply', {effect: auto, params: {}, item: null, ids});
      const applied_ms = performance.now() - started;
      await vectorcraft.execute('select.none', {});
      const waiting = async () => (await vectorcraft.execute('effect.deferredStatus', {})).effects.length;
      let left = await waiting();
      while (left && performance.now() - started < within) {
        await vectorcraft.execute('effect.refreshDeferred', {}).catch(() => {});
        left = await waiting();
      }
      return {shapes: ids.length, apply_ms: applied_ms, turn_ms: performance.now() - started, waiting: left};
    }, {name, dataBase64, auto: AUTO, within: DRAWN_WITHIN_MS});
  };
  // The document in front as the editor's exporter of `format` writes it, and how long that took.
  const exported = async format => {
    const {dataBase64, ms} = await page.evaluate(async format => {
      const started = performance.now();
      const file = await vectorcraft.execute('document.export', {format});
      return {dataBase64: file.dataBase64, ms: performance.now() - started};
    }, format);
    return {bytes: Buffer.from(dataBase64, 'base64'), ms};
  };
  // The 2 machine files of the document in front, from the editor and from `stitch`, which reads the
  // document as the editor saves it, under the name the editor gave its exporter.
  const compared = async design => {
    const files = {pes: await exported('pes'), dst: await exported('dst')};
    const saved = await exported('vectorcraft');
    // Both programs' files stay in the work folder, which the workflow keeps: a difference can be looked at.
    const folder = join(work, 'parity', design);
    await mkdir(folder, {recursive: true});
    const named = label(files.dst.bytes) || 'design';
    const document = join(folder, `${named}.vectorcraft`);
    await writeFile(document, saved.bytes);
    const row = {design, name: named};
    for (const format of ['pes', 'dst']) {
      const out = join(folder, `${named}.${format}`);
      await writeFile(join(folder, `editor.${format}`), files[format].bytes);
      const ran = spawnSync(stitch, ['plan', document, '--output', out], {encoding: 'utf8'});
      if (ran.status !== 0) {
        faults.push(`${design}.${format}: stitch plan ended with ${ran.status ?? ran.error?.message}: ${ran.stderr.trim().slice(0, 1200)}`);
        row[format] = {bytes: files[format].bytes.length, same: false};
        continue;
      }
      const differs = difference(`${design}.${format}`, files[format].bytes, await readFile(out));
      if (differs) faults.push(differs);
      row[format] = {bytes: files[format].bytes.length, same: !differs};
    }
    results.parity.push(row);
    return files.pes.ms;
  };

  const starter = await turned(DESIGNS[0]);
  assert.ok(starter.shapes > 0, 'the starter drawing has shapes');
  await compared(DESIGNS[0].design);
  const runs = [];
  for (let run = 0; run < RUNS; run += 1) {
    const turn = await turned(TIMED);
    assert.ok(turn.shapes > 0, 'the traced photo has shapes');
    // The first run's document is the one compared, so its export is timed there.
    const export_ms = run === 0 ? await compared(TIMED.design) : (await exported('pes')).ms;
    runs.push({...turn, export_ms});
    console.log(`Run ${run + 1} of ${TIMED.design}: ${JSON.stringify(runs[run])}`);
  }
  results.speed = {design: TIMED.design, shapes: runs[0].shapes, runs, limit_ms: LIMIT_MS,
    middle_ms: {turn: middle(runs.map(run => run.turn_ms)), export: middle(runs.map(run => run.export_ms))}};
  faults.push(...overLimit(runs, LIMIT_MS));

  // What a person sees: the photo's shapes with their stitches drawn, the demo's panel folded away.
  await page.evaluate(async () => {
    for (const panel of document.querySelectorAll('details')) panel.open = false;
    await vectorcraft.execute('view.fitArtboard', {});
  });
  await mkdir(join(work, 'after'), {recursive: true});
  await page.screenshot({path: join(work, 'after', 'editor.png')});
  results.editor = 'editor.png';
  if (failures.length) faults.push(`the page threw: ${failures.join('; ').slice(0, 1200)}`);
} finally {
  await browser.close();
  await writeFile(join(work, 'browser.json'), `${JSON.stringify(results, null, 1)}\n`);
}
for (const fault of faults) console.error(`FAIL ${fault}`);
assert.deepEqual(faults, [], 'the editor and stitch write the same files, within the time\'s limit');
console.log(`PASS ${url.href}: ${results.parity.length} designs the same in PES and DST; `
  + `${TIMED.design} turned in ${Math.round(results.speed.middle_ms.turn)} ms and exported in ${Math.round(results.speed.middle_ms.export)} ms`);
