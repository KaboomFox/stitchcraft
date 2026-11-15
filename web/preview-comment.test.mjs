// Why this file exists: the preview's comment is found again by its mark and rewritten, and a later step adds
// sections to it. These cases hold both (.github/scripts/preview-comment.cjs).
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';

const comment = createRequire(import.meta.url)('../.github/scripts/preview-comment.cjs');
const url = 'https://owner.github.io/stitchcraft/pr/7';

test('the comment names the book, the demo and the 3 pictures of the build', () => {
  const body = comment(url, 'abc123');
  assert.ok(body.startsWith(comment.MARK));
  assert.ok(body.includes(`**Docs preview:** ${url}/`));
  assert.ok(body.includes(`**Browser demo:** ${url}/demo/`));
  const pictures = [...body.matchAll(/!\[[^\]]+\]\(([^)]+)\)/g)].map(match => match[1]);
  assert.deepEqual(pictures, ['plain-drawing', 'stitched-drawing', 'stitch-settings'].map(name => `${url}/demo/screenshots/${name}.png?v=abc123`));
  assert.equal(comment(`${url}/`, 'abc123'), body);
  assert.ok(!/ArtCraft/.test(body));
});

test('a later step adds a section after the demo\'s, under a rule', () => {
  const body = comment(url, 'abc123', [['**Before and after:** a sheet']]);
  assert.ok(body.endsWith('\n\n---\n\n**Before and after:** a sheet'));
  assert.ok(body.startsWith(comment(url, 'abc123')));
});

test('the one comment is rewritten, made when missing, and not made to say nothing was removed', async () => {
  const calls = [];
  const client = comments => ({
    paginate: async () => comments,
    rest: {issues: {listComments: {}, updateComment: async args => calls.push(['update', args.comment_id, args.body]),
      createComment: async args => calls.push(['create', args.issue_number, args.body])}},
  });
  const context = {repo: {owner: 'owner', repo: 'stitchcraft'}, payload: {pull_request: {number: 7}}};
  const others = [{id: 1, user: {type: 'User', login: 'visitor'}, body: comment.MARK}, {id: 2, user: {type: 'Bot', login: 'bot'}, body: 'other'}];
  await comment.post({github: client(others), context, body: 'new'});
  await comment.post({github: client([...others, {id: 3, user: {type: 'Bot', login: 'bot'}, body: `${comment.MARK}\nold`}]), context, body: 'new'});
  await comment.post({github: client(others), context, body: comment.removed, create: false});
  assert.deepEqual(calls, [['create', 7, 'new'], ['update', 3, 'new']]);
});
