// Follow the reader's route from PR docs into that PR's demo and back.
import {chromium} from '@playwright/test';
import assert from 'node:assert/strict';

const root = new URL(process.argv[2]);
const permanent = process.argv[3] === 'permanent';
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
  assert.equal(new URL(page.url()).pathname, permanent ? new URL('../app/', root).pathname : demo.pathname, 'docs open the correct demo');
  const ui = await page.evaluate(() => vectorcraft.uiInspect());
  assert.equal(ui.documents.length, 1, 'starter opens from the documentation link');
  await page.getByText('StitchCraft ready · Starter & help', {exact: true}).click();
  await page.getByRole('link', {name: 'Documentation', exact: true}).click();
  await page.waitForURL(url => url.href === root.href || url.href === new URL('index.html', root).href);
  await page.getByRole('link', {name: 'Open the browser demo', exact: true}).waitFor();
  console.log(`PASS docs → matching demo → docs: ${root.href}`);
} finally { await browser.close(); }
