// Why this file exists: the editor knows nothing of StitchCraft. This script, which web/start.mjs loads into
// the editor's page, installs the plug-ins, opens the plain drawing and shows a small panel that leads a
// visitor from the drawing to stitches. Every file it names is beside it, so a preview's copy uses that
// preview's plug-ins and drawing (docs/src/design/vectorcraft-integration.md, Browser distribution).
const version = new URL(import.meta.url).search;
const {installPlugins} = await import(`./loader.mjs${version}`);
const {openFinished, openStarter, turnIntoStitches} = await import(`./starter.mjs${version}`);

const panel = document.createElement('details');
panel.open = true;
panel.style.cssText = 'position:fixed;right:16px;bottom:16px;z-index:10000;max-width:360px;padding:12px;border:1px solid #777;border-radius:8px;background:#20242a;color:#eee;font:14px system-ui;box-shadow:0 4px 20px #0008';
const summary = document.createElement('summary');
summary.textContent = 'Starting StitchCraft…';
panel.append(summary);
const linkStyle = 'display:block;color:#a9dcff;margin:8px 0';
// One line of the panel's text.
const line = text => {
  const p = document.createElement('p');
  p.style.margin = '8px 0';
  p.textContent = text;
  return p;
};
// What the last button did, or why the demo could not go on.
const help = line('');
const about = document.createElement('p');
about.textContent = 'An unofficial build of VectorCraft with StitchCraft\'s plug-ins, made by the StitchCraft project and not by VectorCraft\'s makers.';
panel.append(about);
const docs = document.createElement('a');
docs.href = new URL('../index.html', import.meta.url).href;
docs.textContent = 'Documentation';
docs.style.cssText = linkStyle;
document.body.append(panel);

window.stitchcraftReady = (async () => {
  const start = performance.now();
  while (!window.vectorcraft) {
    if (performance.now() - start > 180000) throw new Error('VectorCraft did not start. Try reloading with ?webgl.');
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  const response = await fetch(new URL('./plugins.json', import.meta.url));
  if (!response.ok) throw new Error(`Plug-in list: HTTP ${response.status}`);
  const config = await response.json();
  // A preview runs in the editor main published. When its pull request moves the pin, the editor it was
  // checked with is another one, and the panel says so.
  if (new URL('./', import.meta.url).href !== new URL('./', location.href).href) {
    const own = await fetch(new URL('./plugins.json', location.href), {cache: 'no-store'});
    const editor = own.ok ? (await own.json()).vectorcraft : undefined;
    if (editor?.revision !== config.vectorcraft?.revision) {
      const differs = document.createElement('p');
      differs.textContent = `This preview was checked with VectorCraft ${config.vectorcraft?.tag}. The editor here is ${editor?.tag ?? 'another build'}.`;
      panel.append(differs);
    }
  }
  const installed = await installPlugins(window.vectorcraft, config.plugins, fetch, text => {summary.textContent = text;});
  // A button that runs `act` and writes what it did, or why it failed, under the buttons.
  const button = (label, act) => {
    const made = document.createElement('button');
    made.textContent = label;
    made.onclick = async () => {
      made.disabled = true;
      try { help.textContent = await act(); }
      catch (error) { help.textContent = error.message; panel.open = true; }
      finally { made.disabled = false; }
    };
    return made;
  };
  const stitch = button('Turn the drawing into stitches', async () => {
    const {shapes, changed} = await turnIntoStitches(window.vectorcraft);
    return changed
      ? `${changed} of ${shapes} shapes now have StitchCraft Auto. Select one to change its effect in Appearance.`
      : `All ${shapes} shapes have a StitchCraft effect already.`;
  });
  stitch.style.cssText = 'display:block;margin:8px 0;padding:8px 12px;border:0;border-radius:6px;background:#e6ad49;color:#20242a;font:600 14px system-ui;cursor:pointer';
  const finished = button('Open the finished example', async () => {
    await openFinished(window.vectorcraft);
    return 'The finished example is in a new tab of the editor. Each of its shapes has StitchCraft Auto.';
  });
  finished.style.cssText = 'display:block;margin:8px 0;padding:0;border:0;background:none;color:#a9dcff;font:14px system-ui;text-decoration:underline;cursor:pointer';
  const download = document.createElement('a');
  download.href = new URL(`./templates/alpine-night.vctemplate${version}`, import.meta.url).href;
  download.download = 'Alpine Night.vctemplate';
  download.textContent = 'Download the finished example as a template';
  download.style.cssText = linkStyle;
  panel.append(
    line('Alpine Night opens as a plain drawing. The button gives each shape stitches.'),
    stitch,
    help,
    line('By hand: Select › All, then Effect › Plug-ins › StitchCraft Auto.'),
    line('File › Export › Export As… writes PES or DST. Save your design before you close this tab.'),
    finished, download, docs);
  // What `document.open` answered for the drawing, or false when the session had a document. The browser
  // check reads it.
  const opened = await openStarter(window.vectorcraft, {onlyIfEmpty: true});
  window.stitchcraftStarter = opened;
  summary.textContent = 'StitchCraft ready · Start here';
  const optional = document.createElement('p');
  optional.textContent = 'Optional heavy filters (download, then open in VectorCraft to approve): ';
  for (const name of config.optionalPlugins) {
    const link = document.createElement('a');
    link.href = new URL(`./plugins/${name}.wasm`, import.meta.url).href;
    link.download = `${name}.wasm`;
    link.textContent = name.replaceAll('-', ' ');
    link.style.color = '#a9dcff';
    optional.append(link, ' · ');
  }
  panel.append(optional);
  // The panel stays open over the plain drawing, where its button is the next thing to do.
  panel.open = Boolean(opened);
  return installed;
})();
window.stitchcraftReady.catch(error => {
  summary.textContent = 'StitchCraft could not finish loading';
  help.textContent = error.message;
  const reload = document.createElement('button');
  reload.textContent = 'Reload';
  reload.onclick = () => location.reload();
  // The buttons' line may not be in the panel yet: a failure before the plug-ins are in comes here too.
  panel.append(help, reload, docs);
  panel.open = true;
  console.error(error);
});
