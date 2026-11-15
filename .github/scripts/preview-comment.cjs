// Why this file exists: a pull request's preview is announced in one comment that each push rewrites, and the
// workflow's publish and cleanup jobs must find the same comment. Its text is made of sections, so a later
// step adds its pictures as one more section and changes nothing here (web/README.md, The preview's comment).
const MARK = '<!-- stitchcraft-web-preview -->';

// The demo's section: where it opens, and the 3 pictures the browser check took of this build.
function demoSection(base, sha) {
  const demo = `${base}demo/`;
  const image = (name, label) => `![${label}](${demo}screenshots/${name}.png?v=${sha})`;
  return [
    `**Browser demo:** ${demo}`,
    '',
    'It opens the Alpine Night starter patch in the editor that main published, with this pull request\'s plug-ins.',
    `[Download the starter as a VectorCraft template](${demo}templates/alpine-night.vctemplate)`,
    '',
    'Chromium took these pictures of this build:',
    '',
    image('starter-patch', 'The Alpine Night starter patch in VectorCraft'),
    '',
    image('stitch-settings', 'The settings of StitchCraft\'s Auto effect'),
    '',
    image('design-settings', 'StitchCraft\'s machine and design settings'),
  ];
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

module.exports = Object.assign(previewComment, {MARK, post, removed});
