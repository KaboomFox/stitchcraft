// Why this file exists: the editor is published once, at /app/, and a pull request's preview brings its own
// plug-ins and loader. This script, which the editor's page loads, chooses whose loader runs: the preview's
// when the address names one (`?preview=260`), and the app's own otherwise
// (docs/src/design/vectorcraft-integration.md, Browser distribution).

// The folder of the loader to run, from the page's query string. Only a pull request's number names a
// preview, so the page never loads a script from an address someone wrote into a link.
export function demoFolder(search) {
  const preview = new URLSearchParams(search).get('preview');
  if (preview === null) return './';
  if (!/^[1-9][0-9]{0,8}$/.test(preview)) throw new Error(`No preview is named "${preview}".`);
  return `../pr/${preview}/demo/`;
}

// The loader's address. The folder's list of files names the build, so a browser that kept an older
// build's scripts fetches this one's.
export async function bootAddress(search, page, fetchFile = fetch) {
  const folder = new URL(demoFolder(search), page);
  const response = await fetchFile(new URL('plugins.json', folder), {cache: 'no-store'});
  if (!response.ok) throw new Error(`No demo at ${folder.pathname} (HTTP ${response.status}). A preview is removed when its pull request closes.`);
  const {version} = await response.json();
  if (!/^[0-9a-f]{16}$/.test(version ?? '')) throw new Error(`The demo at ${folder.pathname} does not name its build.`);
  return new URL(`boot.mjs?v=${version}`, folder).href;
}

if (typeof document !== 'undefined') {
  try {
    await import(await bootAddress(location.search, location.href));
  } catch (error) {
    const note = document.createElement('p');
    note.textContent = `StitchCraft could not start: ${error.message}`;
    note.style.cssText = 'position:fixed;right:16px;bottom:16px;z-index:10000;max-width:360px;padding:12px;border-radius:8px;background:#20242a;color:#eee;font:14px system-ui';
    document.body.append(note);
    console.error(error);
  }
}
