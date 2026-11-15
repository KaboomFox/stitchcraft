// Why this file exists: planning a traced photo takes seconds, and done on the page's own thread it would
// freeze the editor for that long. This worker runs the exporter's module off that thread. It is given the
// module's address, the document and what is asked for, and answers the bytes with what the export says, or
// the reason there are none (web/engine.mjs runs the module).
self.onmessage = async ({data}) => {
  try {
    const {exportWith} = await import(`./engine.mjs${new URL(import.meta.url).search}`);
    const response = await fetch(data.module);
    if (!response.ok) throw new Error(`${data.module}: HTTP ${response.status}`);
    const {bytes, warnings} = await exportWith(await response.arrayBuffer(), data.document, data.extension, data.name, {}, data.mode);
    self.postMessage({bytes, warnings}, [bytes.buffer]);
  } catch (error) {
    self.postMessage({error: String(error?.message ?? error)});
  }
};
