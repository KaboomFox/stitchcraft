// Exercise the actual WASM editor and embroidery exporters from a nested static URL.
import {chromium} from '@playwright/test';
import assert from 'node:assert/strict';

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
  await page.screenshot({path: new URL('../target/web-smoke.png', import.meta.url).pathname});
  assert.deepEqual(failures, [], 'no browser errors or missing assets');
  console.log(`PASS ${url.href}: ${installed.length} plug-ins; PES ${pes.length} bytes, DST ${dst.length} bytes`);
} finally {
  await browser.close();
}
