// Why this file exists: VectorCraft allows an export 10 billion instructions, and a traced photo with the Auto
// effect needs 5 to 6 times as many, so File › Export stops with "plug-in exceeded its instruction budget".
// The panel's buttons export with the same exporter modules, run by the browser in a worker, where nothing
// counts instructions. This module takes the open document from the editor, starts the worker and says how
// long it has been working (docs/src/design/vectorcraft-integration.md, An export that finishes).
//
// It reads the document through the page's documented calls alone: `document.export` with the format
// `vectorcraft` answers the file the editor saves, in base64 (VectorCraft's docs/control-protocol.md, the
// `app.export` row and "In the browser").
const version = new URL(import.meta.url).search;
const {FILE, PICTURE, decodeBase64} = await import(`./engine.mjs${version}`);

// What the panel makes: each machine file, by its extension, and the picture of the stitches. The picture is
// drawn of the PES file, as `stitch export --preview` draws it.
export const MAKES = {
  pes: {extension: 'pes', mode: FILE, type: 'application/octet-stream'},
  dst: {extension: 'dst', mode: FILE, type: 'application/octet-stream'},
  picture: {extension: 'pes', mode: PICTURE, type: 'image/png'},
};

// The name a file gets from a document's title: without the extension of a drawing, without a character a
// file name can't have, at most 60 characters, and `design` when nothing is left.
export function fileName(title) {
  const stem = String(title ?? '').replace(/\.(vectorcraft|vctemplate|svg)$/i, '').replace(/[\\/:*?"<>|\u0000-\u001f]/g, '-').trim().slice(0, 60).trim();
  return stem || 'design';
}

// The title a saved document names, or nothing when the bytes are not a document's.
export function titleOf(saved) {
  try {
    return JSON.parse(new TextDecoder().decode(saved)).document?.title;
  } catch {
    return undefined;
  }
}

// The document in front, as the editor saves it.
export async function savedDocument(api) {
  const ui = await api.uiInspect();
  if (ui.documents.length === 0) throw new Error('Open a drawing first.');
  const {dataBase64} = await api.execute('document.export', {format: 'vectorcraft'});
  return decodeBase64(dataBase64);
}

// A worker that runs the exporter's module.
const engineWorker = () => new Worker(new URL(`./engine-worker.mjs${version}`, import.meta.url), {type: 'module'});

// Makes `what` (a key of MAKES) of the document in front: answers the file's name, its bytes, what the
// export says and how long the engine took, in milliseconds. `working` is told every tenth of a second how
// long it has taken so far. The exporter's own refusal, such as a design with nothing to sew, is thrown.
export async function make(api, what, {working = () => {}, makeWorker = engineWorker, now = () => performance.now()} = {}) {
  const {extension, mode} = MAKES[what];
  const saved = await savedDocument(api);
  const name = fileName(titleOf(saved));
  const started = now();
  const ticking = setInterval(() => working(now() - started), 100);
  const worker = makeWorker();
  try {
    const answer = await new Promise((resolve, reject) => {
      worker.onmessage = ({data}) => resolve(data);
      worker.onerror = event => reject(new Error(event.message || 'The engine could not start.'));
      worker.postMessage({module: new URL(`./plugins/${extension}.wasm${version}`, import.meta.url).href, document: saved, extension, name, mode});
    });
    if (answer.error !== undefined) throw new Error(answer.error);
    return {name: `${name}.${mode === PICTURE ? 'png' : extension}`, bytes: answer.bytes, warnings: answer.warnings, ms: now() - started, document: saved};
  } finally {
    clearInterval(ticking);
    worker.terminate();
  }
}

// Seconds with one decimal, for a time in milliseconds.
export function seconds(ms) {
  return `${(ms / 1000).toFixed(1)} s`;
}

// What the panel says once a file is made: its name, its size, the time, and how much the export said.
export function done({name, bytes, warnings, ms}) {
  const notes = warnings.length === 0 ? 'The export said nothing.' : `The export said ${warnings.length} ${warnings.length === 1 ? 'thing' : 'things'}, listed below.`;
  return `${name}: ${bytes.length.toLocaleString('en')} bytes in ${seconds(ms)}. ${notes}`;
}
