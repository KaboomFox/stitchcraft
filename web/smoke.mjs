// Why this file exists: only a browser shows that the editor starts, takes the plug-ins and exports stitches.
// This check runs the built demo in Chromium as a visitor meets it: the plain drawing opens, the panel's
// button turns it into stitches, and the finished example opens beside it. It takes the 3 pictures a
// preview's comment shows.
import {chromium} from '@playwright/test';
import assert from 'node:assert/strict';
import {mkdir, readFile, rm} from 'node:fs/promises';
import {join} from 'node:path';
const screenshots = process.argv[3] || new URL('../target/web/demo/screenshots', import.meta.url).pathname;
// The folder is made new: a build cache can bring `target` back with an earlier build's pictures in it.
await rm(screenshots, {recursive: true, force: true});
await mkdir(screenshots, {recursive: true});

const url = new URL(process.argv[2] || 'http://127.0.0.1:8766/app/');
url.searchParams.set('webgl', '');
const browser = await chromium.launch({channel: 'chromium', args: ['--no-sandbox', '--enable-unsafe-swiftshader']});
try {
  const page = await browser.newPage({viewport: {width: 1440, height: 1000}});
  const failures = [];
  page.on('pageerror', error => failures.push(error.message));
  page.on('response', response => {if (response.status() >= 400) failures.push(`${response.status()} ${response.url()}`);});
  // A page that never starts says nothing by itself, so keep what it wrote and what failed to load, and
  // print them when the wait runs out: the timeout alone doesn't tell a slow start from a broken one.
  const said = [];
  page.on('console', message => said.push(`${message.type()}: ${message.text()}`.slice(0, 400)));
  page.on('requestfailed', request => failures.push(`${request.failure()?.errorText} ${request.url()}`));
  await page.goto(url.href, {waitUntil: 'domcontentloaded', timeout: 60000});
  // Anything else served at the address, a folder's listing say, fails at once and by name.
  assert.ok(await page.locator('#vectorcraft_canvas').count(), `${url.href} is not the editor's page`);
  try {
    await page.waitForFunction(() => Boolean(window.stitchcraftReady), null, {timeout: 180000});
  } catch (error) {
    const state = await page.evaluate(() => ({
      editor: typeof window.vectorcraft, loading: document.getElementById('vectorcraft_loading')?.textContent ?? null,
      notes: [...document.querySelectorAll('body > p, body > details')].map(node => node.textContent.slice(0, 300)),
      scripts: [...document.scripts].map(script => script.src),
    })).catch(failed => ({unreadable: failed.message}));
    console.error(`The demo did not start at ${url.href}.`);
    console.error(`The page: ${JSON.stringify(state)}`);
    console.error(`Failed to load or threw (${failures.length}):\n  ${failures.join('\n  ')}`);
    console.error(`The page's console, last ${Math.min(said.length, 40)} of ${said.length} lines:\n  ${said.slice(-40).join('\n  ')}`);
    throw error;
  }
  const installed = await page.evaluate(() => window.stitchcraftReady);
  // Every module the build lists as light installs, so a heavy one listed as light fails here.
  const listed = JSON.parse(await readFile(new URL('../target/web/demo/plugins.json', import.meta.url)));
  assert.equal(installed.length, listed.plugins.length);
  assert.ok(listed.plugins.length >= 9, 'the stitch effects, exporters and light filters');
  assert.equal(await page.title(), 'StitchCraft in VectorCraft');
  const starter = await page.evaluate(async () => ({ui: await vectorcraft.uiInspect(), doc: await vectorcraft.docInspect(), opened: window.stitchcraftStarter}));
  assert.equal(starter.ui.documents.length, 1, 'empty session opens the starter');
  assert.ok(JSON.stringify(starter.doc).includes('Alpine'), 'starter art is present');
  // What VectorCraft said of the SVG it opened: a warning names something that did not come in as drawn.
  console.log(`The plain drawing opened as ${JSON.stringify(starter.opened).slice(0, 600)}`);
  assert.deepEqual(starter.opened?.warnings ?? [], [], 'the SVG opens without a warning');
  // The drawing's shapes as `effect.list` reports them, and how many of them have the Auto effect once.
  const shapes = () => page.evaluate(async () => {
    await vectorcraft.execute('select.all', {});
    const {applied} = await vectorcraft.execute('effect.list', {});
    await vectorcraft.execute('select.none', {});
    const all = shape => [...shape.effects, ...shape.items.flatMap(item => item.effects)];
    return {
      count: applied.length,
      stitchcraft: applied.filter(shape => all(shape).some(effect => effect.id.startsWith('plugin.dev.stitchcraft.'))).length,
      autoOnce: applied.filter(shape => all(shape).length === 1 && shape.effects[0]?.id === 'plugin.dev.stitchcraft.auto').length,
    };
  });
  // The stitches a DST file of the document in front records, or the reason the editor gave for writing none.
  const stitches = () => page.evaluate(() => vectorcraft.execute('document.export', {format: 'dst'}))
    .then(file => Number(Buffer.from(file.dataBase64, 'base64').subarray(0, 512).toString().match(/ST:\s*(\d+)/)?.[1]), error => error.message);
  const drawn = (await readFile(new URL('./templates/alpine-night.svg', import.meta.url), 'utf8')).match(/ fill="/g).length;
  assert.deepEqual(await shapes(), {count: drawn, stitchcraft: 0, autoOnce: 0}, 'the starter is the plain drawing, each shape without a StitchCraft effect');
  console.log(`The plain drawing, ${drawn} shapes, exports to DST as: ${await stitches()}`);
  assert.equal(await page.locator('details').evaluate(panel => panel.open), true, 'the panel is open over the plain drawing');
  await page.screenshot({path: join(screenshots, 'plain-drawing.png')});
  const turn = page.getByRole('button', {name: 'Turn the drawing into stitches', exact: true});
  await turn.click();
  await page.getByText(`${drawn} of ${drawn} shapes now have StitchCraft Auto.`, {exact: false}).waitFor();
  assert.deepEqual(await shapes(), {count: drawn, stitchcraft: drawn, autoOnce: drawn}, 'after the button every shape has the Auto effect');
  const sewn = await stitches();
  assert.ok(sewn > 0, `the drawing exports actual stitches after the button: ${sewn}`);
  // VectorCraft draws a shape plain while its effect waits for time on screen, so the picture is taken once
  // the editor has been asked to draw the ones it left, and the log says how many it still leaves.
  const waiting = () => page.evaluate(async () => (await vectorcraft.execute('effect.deferredStatus', {})).effects.length);
  const left = await waiting();
  if (left) await page.evaluate(() => vectorcraft.execute('effect.refreshDeferred', {})).catch(error => console.log(`Refresh: ${error.message}`));
  console.log(`Effects the editor had not drawn after the button: ${left}, and after a refresh: ${left && await waiting()}`);
  await page.screenshot({path: join(screenshots, 'stitched-drawing.png')});
  await turn.click();
  await page.getByText(`All ${drawn} shapes have a StitchCraft effect already.`, {exact: true}).waitFor();
  assert.deepEqual(await shapes(), {count: drawn, stitchcraft: drawn, autoOnce: drawn}, 'a second press adds no effect');
  await page.evaluate(async () => {
    await vectorcraft.execute('select.all', {});
    await vectorcraft.execute('effect.dialog', {effect: 'plugin.dev.stitchcraft.auto', index: 0, item: null});
  });
  const effect = await page.evaluate(() => vectorcraft.uiInspect());
  assert.equal(effect.ui?.dialog?.kind, 'effect', 'actual StitchCraft Auto dialog is open');
  await page.screenshot({path: join(screenshots, 'stitch-settings.png')});
  await page.evaluate(async () => {
    await vectorcraft.request('ui.dialog.cancel');
    await vectorcraft.execute('plugin.dialog', {id: 'dev.stitchcraft.settings'});
  });
  const settings = await page.evaluate(() => vectorcraft.uiInspect());
  assert.equal(settings.ui?.dialog?.kind, 'plugin', 'actual Design Settings dialog is open');
  await page.evaluate(() => vectorcraft.request('ui.dialog.cancel'));
  await page.getByRole('button', {name: 'Open the finished example', exact: true}).click();
  await page.waitForFunction(async () => (await vectorcraft.uiInspect()).documents.length === 2);
  await page.getByText('The finished example is in a new tab of the editor.', {exact: false}).waitFor();
  assert.deepEqual(await shapes(), {count: drawn, stitchcraft: drawn, autoOnce: drawn}, 'the finished example has the Auto effect on every shape');
  const finished = await stitches();
  assert.ok(finished > 0, `the finished example exports actual stitches: ${finished}`);
  console.log(`After the button the drawing exports ${sewn} stitches, and the finished example ${finished}`);
  // The panel's own exports, by StitchCraft's engine in a worker: the button offers the file, and says how
  // long it took, and the picture of its stitches opens over the editor and closes again.
  for (const [label, extension, start] of [['Export PES', 'pes', '#PES0001'], ['Export DST', 'dst', 'LA:']]) {
    const [download] = await Promise.all([page.waitForEvent('download'), page.getByRole('button', {name: label, exact: true}).click()]);
    assert.match(download.suggestedFilename(), new RegExp(`\\.${extension}$`), `${label} offers a ${extension} file`);
    const file = await readFile(await download.path());
    assert.equal(file.subarray(0, start.length).toString(), start, `${label} writes a ${extension} file`);
    await page.getByText(new RegExp(`\\.${extension}: [0-9,]+ bytes in [0-9.]+ s\\.`)).waitFor();
    console.log(`${label}: ${download.suggestedFilename()}, ${file.length} bytes`);
  }
  await page.getByRole('button', {name: 'Show the real stitches', exact: true}).click();
  const picture = page.getByAltText('The stitches the machine file sews');
  await picture.waitFor();
  assert.ok(await picture.evaluate(image => image.decode().then(() => image.naturalWidth > 100 && image.naturalHeight > 100)), 'the picture of the stitches is drawn');
  await page.getByRole('button', {name: 'Close the picture', exact: true}).click();
  await picture.waitFor({state: 'hidden'});
  const exports = await page.evaluate(async () => {
    const api = window.vectorcraft;
    await api.execute('file.new', {width: 100, height: 100});
    await api.execute('shape.rectangle', {x: 10, y: 10, width: 30, height: 20});
    await api.execute('effect.apply', {effect: 'plugin.dev.stitchcraft.fill', params: {}});
    const pes = await api.execute('document.export', {format: 'pes'});
    const dst = await api.execute('document.export', {format: 'dst'});
    return {pes, dst};
  });
  const pes = Buffer.from(exports.pes.dataBase64, 'base64');
  const dst = Buffer.from(exports.dst.dataBase64, 'base64');
  assert.ok(pes.length > 100, 'PES contains stitches');
  assert.match(pes.subarray(0, 8).toString(), /^#PES/);
  assert.ok(dst.length > 512, 'DST contains stitches beyond its header');
  assert.equal(dst.subarray(0, 3).toString(), 'LA:');
  assert.ok(Number(dst.subarray(0, 512).toString().match(/ST:\s*(\d+)/)?.[1]) > 0, 'DST records actual stitches');
  await page.screenshot({path: new URL('../target/web-smoke.png', import.meta.url).pathname});
  assert.deepEqual(failures, [], 'no browser errors or missing assets');
  console.log(`PASS ${url.href}: ${installed.length} plug-ins; PES ${pes.length} bytes, DST ${dst.length} bytes`);
} finally {
  await browser.close();
}
