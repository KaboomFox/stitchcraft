// Regenerate the native starter from our original SVG through VectorCraft's public API.
import {chromium} from '@playwright/test';
import {readFile, writeFile} from 'node:fs/promises';
const svg = await readFile(new URL('./templates/alpine-night.svg', import.meta.url));
const browser = await chromium.launch({channel: 'chromium', args: ['--no-sandbox', '--enable-unsafe-swiftshader']});
try {
  const page = await browser.newPage({viewport: {width: 1440, height: 1000}});
  await page.goto(process.argv[2] || 'http://127.0.0.1:8766/pr/1/?webgl');
  await page.waitForFunction(() => Boolean(window.stitchcraftReady));
  await page.evaluate(() => window.stitchcraftReady);
  const result = await page.evaluate(async dataBase64 => {
    await vectorcraft.execute('document.open', {name: 'Alpine Night.svg', dataBase64});
    await vectorcraft.execute('select.all', {});
    await vectorcraft.execute('effect.apply', {effect: 'plugin.dev.stitchcraft.auto', params: {}});
    return vectorcraft.execute('document.serialize', {format: 'template'});
  }, svg.toString('base64'));
  const bytes = result.dataBase64 ? Buffer.from(result.dataBase64, 'base64') : result.text;
  if (!bytes) throw new Error(`No serialized template: ${JSON.stringify(result)}`);
  await writeFile(new URL('./templates/alpine-night.vctemplate', import.meta.url), bytes);
  console.log('Generated Alpine Night.vctemplate');
} finally { await browser.close(); }
