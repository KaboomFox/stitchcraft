// Why this file exists: a pull request's preview is announced in one comment that each push rewrites, and the
// workflow's publish and cleanup jobs must find the same comment. Its text is made of sections: the demo's,
// and the before and after sheet's, whose lines the build wrote (web/README.md, The preview's comment).
const MARK = '<!-- stitchcraft-web-preview -->';

// The demo's section: where it opens, and the 3 pictures the browser check took of this build.
function demoSection(base, sha) {
  const demo = `${base}demo/`;
  const image = (name, label) => `![${label}](${demo}screenshots/${name}.png?v=${sha})`;
  return [
    `**Browser demo:** ${demo}`,
    '',
    'It opens Alpine Night as a plain drawing in the editor that main published, with this pull request\'s plug-ins.',
    'The panel\'s button turns the drawing into stitches.',
    `[Download the finished example as a VectorCraft template](${demo}templates/alpine-night.vctemplate)`,
    '',
    'Chromium took these pictures of this build:',
    '',
    image('plain-drawing', 'Alpine Night as it opens, a plain drawing'),
    '',
    image('stitched-drawing', 'The drawing after the panel\'s button, in stitches'),
    '',
    image('stitch-settings', 'The settings of StitchCraft\'s Auto effect'),
  ];
}

// The sheet's section: the lines `web/sheet.py` wrote, with the sheet's address where they say `{sheet}`.
// The lines come from a file of the build, so anything that is not a short list of short lines is left out.
function sheetSection(lines, url) {
  const base = url.endsWith('/') ? url : `${url}/`;
  if (!Array.isArray(lines) || lines.length === 0 || lines.length > 60) return null;
  if (!lines.every(line => typeof line === 'string' && line.length <= 600)) return null;
  return lines.map(line => line.replaceAll('{sheet}', `${base}sheet/`));
}

// The comment's text: the book's address, then each section under a rule. `sections` are arrays of lines.
function previewComment(url, sha, sections = []) {
  const base = url.endsWith('/') ? url : `${url}/`;
  const parts = [[`**Docs preview:** ${base}`, '', `Built from ${sha}. Pages can take a minute to show it.`], demoSection(base, sha), ...sections];
  return [MARK, ...parts.flatMap((lines, index) => (index ? ['', '---', ''] : []).concat(lines))].join('\n');
}

// Write `body` into the pull request's one preview comment, or make it, unless `create` is false.
async function post({github, context, body, create = true}) {
  const {owner, repo} = context.repo;
  const issue_number = context.payload.pull_request.number;
  const comments = await github.paginate(github.rest.issues.listComments, {owner, repo, issue_number});
  const old = comments.find(c => (c.user.type === 'Bot' || c.user.login === owner) && c.body.includes(MARK));
  if (old) await github.rest.issues.updateComment({owner, repo, comment_id: old.id, body});
  else if (create) await github.rest.issues.createComment({owner, repo, issue_number, body});
}

// A closed pull request's comment says its preview is gone. One that never had a preview gets no comment.
const removed = `${MARK}\nThe preview was removed when the pull request closed.`;

module.exports = Object.assign(previewComment, {MARK, post, removed, sheetSection});
