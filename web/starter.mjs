// Why this file exists: an empty editor teaches nothing about embroidery, and a design that is already
// stitches leaves a visitor nothing to do. This module opens Alpine Night as a plain drawing when the session
// has no document of its own, gives a drawing's shapes StitchCraft's Auto effect when the visitor asks, and
// opens the finished example (docs/src/design/vectorcraft-integration.md, The starter drawing).
const version = new URL(import.meta.url).search;
const {encodeBytes} = await import(`./loader.mjs${version}`);

export const AUTO = 'plugin.dev.stitchcraft.auto';
// The id every StitchCraft effect starts with: a shape that has one already sews.
const OURS = '"plugin.dev.stitchcraft.';

// Opening a file makes a new document; never replace an existing tab.
async function open(api, file, name, {onlyIfEmpty = false, fetchFile = fetch} = {}) {
  const empty = async () => {
    const ui = await api.uiInspect();
    return ui.documents.length === 0 && !ui.ui?.dialog && !ui.opening?.length;
  };
  if (onlyIfEmpty && !await empty()) return false;
  const response = await fetchFile(new URL(`./templates/${file}${version}`, import.meta.url));
  if (!response.ok) throw new Error(`${name}: HTTP ${response.status}`);
  const dataBase64 = encodeBytes(new Uint8Array(await response.arrayBuffer()));
  // A person may have opened their own file while this one was downloading.
  if (onlyIfEmpty && !await empty()) return false;
  const opened = await api.execute('document.open', {name, dataBase64});
  await api.execute('view.fitArtboard', {});
  return opened ?? true;
}

// The plain drawing, from the SVG the finished example was made from. Answers what `document.open` did, or
// false when the session had a document.
export function openStarter(api, options) {
  return open(api, 'alpine-night.svg', 'Alpine Night.svg', options);
}

// The same drawing with the Auto effect on every shape, as a template VectorCraft wrote.
export function openFinished(api, options) {
  return open(api, 'alpine-night.vctemplate', 'Alpine Night.vctemplate', options);
}

// Give every shape of the document in front the Auto effect, as Select › All and then Effect › Plug-ins ›
// StitchCraft Auto do. A shape that has a StitchCraft effect keeps it, so a second press adds nothing.
// Answers how many shapes there are and how many were given the effect.
export async function turnIntoStitches(api) {
  const ui = await api.uiInspect();
  if (ui.documents.length === 0) throw new Error('Open a drawing first.');
  await api.execute('select.all', {});
  const {applied = []} = await api.execute('effect.list', {});
  if (applied.length === 0) throw new Error('The drawing has no shapes to turn into stitches.');
  const plain = applied.filter(shape => !sews(shape)).map(shape => shape.id);
  if (plain.length) await api.execute('effect.apply', {effect: AUTO, params: {}, item: null, ids: plain});
  await api.execute('select.none', {});
  return {shapes: applied.length, changed: plain.length};
}

// Whether a shape, as `effect.list` reports it, has a StitchCraft effect: its own, or one on a fill or stroke.
export function sews(shape) {
  return JSON.stringify([shape.effects ?? [], (shape.items ?? []).map(item => item.effects ?? [])]).includes(OURS);
}
