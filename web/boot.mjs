import {installPlugins} from './loader.mjs';

const panel = document.createElement('details');
panel.open = true;
panel.style.cssText = 'position:fixed;right:16px;bottom:16px;z-index:10000;max-width:360px;padding:12px;border:1px solid #777;border-radius:8px;background:#20242a;color:#eee;font:14px system-ui;box-shadow:0 4px 20px #0008';
const summary = document.createElement('summary');
summary.textContent = 'Starting StitchCraft…';
panel.append(summary);
const help = document.createElement('p');
help.textContent = 'Draw or open your vector artwork. Use Effect → Plug-ins for stitch settings, then File → Export for PES or DST. Save your design before closing this tab.';
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
  summary.textContent = 'StitchCraft ready · Help';
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
  summary.textContent = 'Embroidery tools failed to load';
  help.textContent = error.message;
  const reload = document.createElement('button');
  reload.textContent = 'Reload';
  reload.onclick = () => location.reload();
  panel.append(reload);
  panel.open = true;
  console.error(error);
});
