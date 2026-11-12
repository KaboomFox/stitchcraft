// The host's documented page API is the only connection to VectorCraft.
export async function installPlugins(api, names, fetchFile = fetch, progress = () => {}) {
  const installed = [];
  for (const name of names) {
    progress(`Loading embroidery tools (${installed.length + 1}/${names.length})…`);
    const response = await fetchFile(new URL(`./plugins/${name}.wasm`, import.meta.url));
    if (!response.ok) throw new Error(`${name}: HTTP ${response.status}`);
    const bytes = new Uint8Array(await response.arrayBuffer());

    const plugin = await api.execute('plugin.install', {name: `${name}.wasm`, dataBase64: encodeBytes(bytes)});
    installed.push(plugin);
  }
  return installed;
}

// Chunking avoids overflowing the JavaScript argument stack on a large module.
export function encodeBytes(bytes) {
  let binary = '';
  for (let i = 0; i < bytes.length; i += 8192) {
    binary += String.fromCharCode(...bytes.subarray(i, i + 8192));
  }
  return btoa(binary);
}
