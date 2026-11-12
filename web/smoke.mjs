// Exercise the actual WASM editor and embroidery exporters from a nested static URL.
import {chromium} from '@playwright/test';
import assert from 'node:assert/strict';
import {mkdir} from 'node:fs/promises';
import {join} from 'node:path';
const screenshots = process.argv[3] || new URL('../target/web/screenshots', import.meta.url).pathname;
await mkdir(screenshots, {recursive: true});

const url = new URL(process.argv[2] || 'http://127.0.0.1:8766/');
url.searchParams.set('webgl', '');
const browser = await chromium.launch({channel: 'chromium', args: ['--no-sandbox', '--enable-unsafe-swiftshader']});
try {
  const page = await browser.newPage({viewport: {width: 1440, height: 1000}});
  const failures = [];
  page.on('pageerror', error => failures.push(error.message));
  page.on('response', response => {if (response.status() >= 400) failures.push(`${response.status()} ${response.url()}`);});
  await page.goto(url.href, {waitUntil: 'domcontentloaded', timeout: 60000});
  await page.waitForFunction(() => Boolean(window.stitchcraftReady), null, {timeout: 180000});
  const installed = await page.evaluate(() => window.stitchcraftReady);
  assert.equal(installed.length, 9);
  const starter = await page.evaluate(async () => ({ui: await vectorcraft.uiInspect(), doc: await vectorcraft.docInspect()}));
  assert.equal(starter.ui.documents.length, 1, 'empty session opens the starter');
  assert.ok(JSON.stringify(starter.doc).includes('Alpine'), 'starter art is present');
  await page.evaluate(() => vectorcraft.execute('select.none', {}));
  await page.screenshot({path: join(screenshots, 'starter-patch.png')});
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
  await page.screenshot({path: join(screenshots, 'design-settings.png')});
  await page.evaluate(() => vectorcraft.request('ui.dialog.cancel'));
  const starterExport = await page.evaluate(() => vectorcraft.execute('document.export', {format: 'dst'}));
  const starterDst = Buffer.from(starterExport.dataBase64, 'base64');
  assert.ok(Number(starterDst.subarray(0, 512).toString().match(/ST:\s*(\d+)/)?.[1]) > 0, 'starter exports actual stitches');
  await page.getByText('StitchCraft ready · Starter & help', {exact: true}).click();
  await page.getByRole('button', {name: 'Open Alpine Night starter patch', exact: true}).click();
  await page.waitForFunction(async () => (await vectorcraft.uiInspect()).documents.length === 2);
  await page.getByText('StitchCraft ready · Starter & help', {exact: true}).click();
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
