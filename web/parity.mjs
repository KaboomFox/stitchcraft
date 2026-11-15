// Why this file exists: the browser's checks of the before and after sheet (sheet.mjs) compare machine files
// and times, and what they compare with is plain arithmetic that a test can pin without a browser
// (docs/src/design/before-after-sheet.md, The browser's checks).

// Where 2 files first differ: the offset of the first byte that is not the same in both, the length of the
// shorter when one is the start of the other, or -1 when they are the same file.
export function firstDifference(a, b) {
  const shared = Math.min(a.length, b.length);
  for (let at = 0; at < shared; at += 1) if (a[at] !== b[at]) return at;
  return a.length === b.length ? -1 : shared;
}

// What a failed comparison says: which file, the first byte that differs and what each side has there.
export function difference(name, editor, stitch) {
  const at = firstDifference(editor, stitch);
  if (at < 0) return null;
  const byte = file => (at < file.length ? `0x${file[at].toString(16).padStart(2, '0')}` : 'nothing, the file ends');
  return `${name}: the editor's file (${editor.length} bytes) and stitch's (${stitch.length} bytes) first differ at byte ${at}: `
    + `the editor has ${byte(editor)}, stitch has ${byte(stitch)}`;
}

// The design's name a DST file's header holds, after `LA:` and up to the carriage return: the name the
// editor gave its exporter, which `stitch` takes from its design file's name.
export function label(dst) {
  const header = Buffer.from(dst.subarray(0, 20)).toString('latin1');
  return header.startsWith('LA:') ? header.slice(3).split('\r')[0].trimEnd() : '';
}

// The middle one of the numbers, the upper of the 2 in the middle when there is an even count.
export function middle(numbers) {
  const sorted = [...numbers].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
}

// Why a run's times break their limits, as sentences: none when each middle time is within its limit, or
// when no limit is set. `limits` and a run's times are in milliseconds, by `turn` and `export`.
export function overLimit(runs, limits) {
  const said = [];
  for (const [key, what] of [['turn', 'giving the shapes the Auto effect'], ['export', 'the PES export']]) {
    const taken = middle(runs.map(run => run[`${key}_ms`]));
    if (limits[key] && taken > limits[key]) said.push(`${what} took ${Math.round(taken)} ms, the middle of ${runs.length} runs, above the limit of ${limits[key]} ms`);
  }
  return said;
}
