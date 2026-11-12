const version = new URL(import.meta.url).search;
const {installPlugins} = await import(`./loader.mjs${version}`);
const {openStarter} = await import(`./starter.mjs${version}`);

const panel = document.createElement('details');
panel.open = true;
panel.style.cssText = 'position:fixed;right:16px;bottom:16px;z-index:10000;max-width:360px;padding:12px;border:1px solid #777;border-radius:8px;background:#20242a;color:#eee;font:14px system-ui;box-shadow:0 4px 20px #0008';
const summary = document.createElement('summary');
summary.textContent = 'Starting StitchCraft…';
panel.append(summary);
const help = document.createElement('p');
help.textContent = 'The Alpine Night starter uses StitchCraft Auto. Select a shape and edit its effect in Appearance. File → Export → Export As… downloads PES or DST. Save your editable design before closing this tab.';
panel.append(help);
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
  const installed = await installPlugins(window.vectorcraft, config.plugins, fetch, text => {summary.textContent = text;});
  const starter = document.createElement('button');
  starter.textContent = 'Open Alpine Night starter patch';
  starter.onclick = async () => {
    starter.disabled = true;
    try { await openStarter(window.vectorcraft); }
    catch (error) { help.textContent = error.message; panel.open = true; }
    finally { starter.disabled = false; }
  };
  const download = document.createElement('a');
  download.href = `./templates/alpine-night.vctemplate${version}`;
  download.download = 'Alpine Night.vctemplate';
  download.textContent = 'Download reusable template';
  download.style.cssText = 'display:block;color:#a9dcff;margin:8px 0';
  panel.append(starter, download);
  await openStarter(window.vectorcraft, {onlyIfEmpty: true});
  summary.textContent = 'StitchCraft ready · Starter & help';
  const optional = document.createElement('p');
  optional.textContent = 'Optional heavy filters (download, then open in VectorCraft to approve): ';
  for (const name of config.optionalPlugins) {
    const link = document.createElement('a');
    link.href = `./plugins/${name}.wasm`;
    link.download = `${name}.wasm`;
    link.textContent = name.replaceAll('-', ' ');
    link.style.color = '#a9dcff';
    optional.append(link, ' · ');
  }
  panel.append(optional);
  panel.open = false;
  return installed;
})();
window.stitchcraftReady.catch(error => {
  summary.textContent = 'StitchCraft could not finish loading';
  help.textContent = error.message;
  const reload = document.createElement('button');
  reload.textContent = 'Reload';
  reload.onclick = () => location.reload();
  panel.append(reload);
  panel.open = true;
  console.error(error);
});
