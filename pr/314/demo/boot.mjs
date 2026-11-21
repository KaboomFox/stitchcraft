// Why this file exists: the editor knows nothing of StitchCraft. This script, which web/start.mjs loads into
// the editor's page, installs the plug-ins, opens the plain drawing and shows a small panel that leads a
// visitor from the drawing to stitches. Every file it names is beside it, so a preview's copy uses that
// preview's plug-ins and drawing (docs/src/design/vectorcraft-integration.md, Browser distribution).
const version = new URL(import.meta.url).search;
const {installPlugins} = await import(`./loader.mjs${version}`);
const {openFinished, openStarter, turnIntoStitches} = await import(`./starter.mjs${version}`);
const {done, make, seconds} = await import(`./exports.mjs${version}`);

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
  // What the last export said, one line each, folded away under the buttons.
  const said = document.createElement('details');
  said.hidden = true;
  // The picture of the stitches, over the editor until it is closed.
  const shown = document.createElement('figure');
  shown.style.cssText = 'position:fixed;inset:24px;z-index:10001;margin:0;padding:12px;border-radius:8px;background:#20242a;box-shadow:0 4px 40px #000c;display:none;flex-direction:column;gap:8px';
  // Makes `what` with StitchCraft's engine, says how long it has been working, and lists what the export said.
  const made = async (what, doing) => {
    said.hidden = true;
    const out = await make(window.vectorcraft, what, {working: ms => {help.textContent = `${doing}… ${seconds(ms)}`;}});
    const heading = document.createElement('summary');
    heading.textContent = 'What the export said';
    said.replaceChildren(heading, ...out.warnings.map(line));
    said.hidden = out.warnings.length === 0;
    return out;
  };
  const exporter = (label, what) => {
    const exports = button(label, async () => {
      const out = await made(what, 'Planning the stitches');
      const link = document.createElement('a');
      link.href = URL.createObjectURL(new Blob([out.bytes], {type: 'application/octet-stream'}));
      link.download = out.name;
      link.click();
      return done(out);
    });
    exports.style.cssText = 'margin:4px 8px 4px 0;padding:6px 10px;border:1px solid #e6ad49;border-radius:6px;background:none;color:#e6ad49;font:600 14px system-ui;cursor:pointer';
    return exports;
  };
  const picture = button('Show the real stitches', async () => {
    const out = await made('picture', 'Drawing the stitches');
    const image = document.createElement('img');
    image.src = URL.createObjectURL(new Blob([out.bytes], {type: 'image/png'}));
    image.alt = 'The stitches the machine file sews';
    image.style.cssText = 'flex:1;min-height:0;object-fit:contain';
    const close = document.createElement('button');
    close.textContent = 'Close the picture';
    close.style.cssText = 'align-self:flex-end';
    close.onclick = () => {shown.style.display = 'none';};
    shown.replaceChildren(close, image);
    shown.style.display = 'flex';
    return `The picture is what the PES file sews, drawn in ${seconds(out.ms)}.`;
  });
  picture.style.cssText = 'margin:4px 8px 4px 0;padding:6px 10px;border:1px solid #a9dcff;border-radius:6px;background:none;color:#a9dcff;font:14px system-ui;cursor:pointer';
  document.body.append(shown);
  const makes = document.createElement('div');
  makes.append(exporter('Export PES', 'pes'), exporter('Export DST', 'dst'), picture);
  // The checks ask for a file as the buttons make it, without a download.
  window.stitchcraftMake = what => make(window.vectorcraft, what);
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
    line('These buttons write the machine file with StitchCraft\'s own engine, in this page. They also export a traced photo, which the editor\'s File › Export › Export As… stops as too large.'),
    makes,
    said,
    line('Save your design before you close this tab.'),
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
