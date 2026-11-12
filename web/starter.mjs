const version = new URL(import.meta.url).search;
const {encodeBytes} = await import(`./loader.mjs${version}`);

// Opening a native template makes a new document; never replace an existing tab.
export async function openStarter(api, {onlyIfEmpty = false, fetchFile = fetch} = {}) {
  const empty = async () => {
    const ui = await api.uiInspect();
    return ui.documents.length === 0 && !ui.ui?.dialog && !ui.opening?.length;
  };
  if (onlyIfEmpty && !await empty()) return false;
  const response = await fetchFile(new URL(`./templates/alpine-night.vctemplate${version}`, import.meta.url));
  if (!response.ok) throw new Error(`Starter patch: HTTP ${response.status}`);
  const dataBase64 = encodeBytes(new Uint8Array(await response.arrayBuffer()));
  // A person may have opened their own file while the template was downloading.
  if (onlyIfEmpty && !await empty()) return false;
  await api.execute('document.open', {name: 'Alpine Night.vctemplate', dataBase64});
  await api.execute('view.fitArtboard', {});
  await api.execute('select.all', {});
  return true;
}
