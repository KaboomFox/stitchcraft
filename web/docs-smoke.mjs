// Why this file exists: the book's link to the demo and the demo's link back are addresses that only hold on
// the published site's layout. This check follows them on a copy of that layout.
// Follow the reader's route from PR docs into that PR's demo and back.
import {chromium} from '@playwright/test';
import assert from 'node:assert/strict';

const root = new URL(process.argv[2]);
// The editor is at /app/ on the site: 2 folders above a preview's book (/pr/123/), 1 above a permanent one.
const preview = root.pathname.match(/\/pr\/([1-9][0-9]*)\/$/);
const app = new URL(preview ? '../../app/' : '../app/', root);
const browser = await chromium.launch({channel: 'chromium', args: ['--no-sandbox', '--enable-unsafe-swiftshader']});
try {
  const page = await browser.newPage({viewport: {width: 1440, height: 1000}});
  await page.goto(root.href, {waitUntil: 'domcontentloaded'});
  const launch = page.getByRole('link', {name: 'Open the browser demo', exact: true});
  const demo = new URL(await launch.getAttribute('href'), root);
  assert.equal(demo.href, new URL('demo/index.html', root).href);
  // Force the same WebGL backend used by the embroidery export checks.
  await launch.evaluate(link => { link.href += '?webgl'; });
  await launch.click();
  await page.waitForFunction(() => Boolean(window.stitchcraftReady), null, {timeout: 180000});
  await page.evaluate(() => window.stitchcraftReady);
  const opened = new URL(page.url());
  assert.equal(opened.pathname, app.pathname, 'the book opens the one published editor');
  assert.equal(opened.searchParams.get('preview'), preview ? preview[1] : null, 'a preview names itself to the editor');
  const loader = await page.evaluate(() => performance.getEntriesByType('resource').map(entry => entry.name).find(name => name.includes('/boot.mjs')));
  assert.equal(new URL(loader).pathname, new URL('boot.mjs', preview ? demo : app).pathname, 'the loader is the preview\'s own');
  const ui = await page.evaluate(() => vectorcraft.uiInspect());
  assert.equal(ui.documents.length, 1, 'starter opens from the documentation link');
  // The panel is open over the plain drawing, with its link in reach.
  await page.getByText('StitchCraft ready · Start here', {exact: true}).waitFor();
  await page.getByRole('link', {name: 'Documentation', exact: true}).click();
  await page.waitForURL(url => url.href === root.href || url.href === new URL('index.html', root).href);
  await page.getByRole('link', {name: 'Open the browser demo', exact: true}).waitFor();
  console.log(`PASS docs → matching demo → docs: ${root.href}`);
} finally { await browser.close(); }
