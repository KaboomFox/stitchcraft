// Why this file exists: VectorCraft stops an exporter plug-in that runs more instructions than it allows an
// export, and a traced photo needs about 5 times as many. The exporters' modules are plain WebAssembly with
// no imports (VectorCraft's docs/plugins.md, ABI v1), so the browser can run one itself, where nothing
// counts its instructions. This file is that call and nothing else: it gives the module the document and the
// parameters VectorCraft would give it, and hands back what the module answers. Every stitch is the
// module's (docs/src/design/vectorcraft-integration.md, An export that finishes).

// What a run is asked for: the machine file, which is VectorCraft's own mode, or the picture `stitch export
// --preview` draws of that file, a PNG (`PICTURE` in apps/stitchcraft-vc-plugin/src/export.rs).
export const FILE = 'export';
export const PICTURE = 'picture';

// The parameters VectorCraft passes an exporter's `vc_run`: each of its dialog's at the default its manifest
// names, then `set`, and the file's extension and name, with what is asked for as the mode.
export function exportParams(manifest, set, extension, name, mode = FILE) {
  const params = Object.fromEntries(Object.entries(manifest.params ?? {}).map(([id, param]) => [id, param.default]));
  return {...params, ...set, _context: {mode, extension, name}};
}

// Runs the exporter module `wasm` (its bytes, or a compiled module) on `document`, the bytes of a
// `.vectorcraft` file, as the file `name` with the extension `extension`. Answers the bytes of the file, or
// of its picture when `mode` asks for that, with what the export says, or throws what the exporter declined
// with.
export async function exportWith(wasm, document, extension, name, set = {}, mode = FILE) {
  const module = wasm instanceof WebAssembly.Module ? wasm : await WebAssembly.compile(wasm);
  const {exports} = await WebAssembly.instantiate(module, {});
  if (exports.vc_abi_version() !== 1) throw new Error('This module does not speak plug-in ABI v1.');
  // Bytes at `(length << 32) | offset`, as `vc_manifest` and `vc_run` answer; a negative answer is a failure.
  const answered = packed => {
    if (packed <= 0n) throw new Error(`The exporter failed (code ${packed}).`);
    const text = new Uint8Array(exports.memory.buffer, Number(packed & 0xffffffffn), Number(packed >> 32n));
    return JSON.parse(new TextDecoder().decode(text));
  };
  // `bytes` copied into a block the module allocates: where it is, and how long.
  const given = bytes => {
    const at = exports.vc_alloc(bytes.length);
    if (at === 0) throw new Error('The exporter has no memory left for this document.');
    new Uint8Array(exports.memory.buffer, at, bytes.length).set(bytes);
    return [at, bytes.length];
  };
  const manifest = answered(BigInt.asUintN(64, exports.vc_manifest()));
  const params = new TextEncoder().encode(JSON.stringify(exportParams(manifest, set, extension, name, mode)));
  const out = answered(exports.vc_run(...given(document), ...given(params)));
  if (out.error !== undefined) throw new Error(out.error);
  return {bytes: decodeBase64(out.dataBase64), warnings: out.warnings ?? []};
}

// The bytes `text` names in base64.
export function decodeBase64(text) {
  return Uint8Array.from(atob(text), character => character.charCodeAt(0));
}
