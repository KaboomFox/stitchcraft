// Shared by the workflow and local preview publication.
module.exports = function previewComment(url, sha) {
  const base = url.endsWith('/') ? url : `${url}/`;
  const image = (name, label) => `![${label}](${base}screenshots/${name}.png?v=${sha})`;
  return [
    '<!-- stitchcraft-web-preview -->',
    `**WASM preview:** ${base}`,
    '',
    `Built from ${sha}. Opens the Alpine Night starter patch in an empty session.`,
    `[Download the VectorCraft template](${base}templates/alpine-night.vctemplate)`,
    '',
    'Screenshots captured in Chromium from this build:',
    '',
    image('starter-patch', 'Alpine Night starter patch in VectorCraft'),
    '',
    image('stitch-settings', 'StitchCraft Auto settings'),
    '',
    image('design-settings', 'StitchCraft machine and design settings'),
    '',
    'Pages may take a minute to update. The screenshot URLs include the build revision.',
  ].join('\n');
};
