# Docs fixtures

Small SVG designs for the docs' pictures (`docs/shots.toml`) and command-line examples
(`docs/examples.toml`). `cargo xtask docs` and `cargo xtask shots` plan them with the engine, so a page
always shows what StitchCraft does with them today. Each one is hand-written for StitchCraft and shows one
thing. Test inputs live in `conformance/fixtures/`.

| Fixture | Shows | Used by |
|---|---|---|
| `badge.svg` | A red star inside a blue ring | the SVG tutorial: the `badge` shots and the `plan-badge` example |
| `wave.svg` | A smooth wave | running stitch length |
| `half-disc.svg` | A half disc: a straight side with 2 corners, and a curve | running stitch tolerance |
| `parallel.svg` | 14 rows 0.6 mm apart, sewn as one path | random stitch length |
| `nodes.svg` | Straight lines and one curve | manual stitch |
| `lock.svg` | An 8 mm line | lock shapes |
| `wide.svg` | A 210 mm border, wider than the reference hoop | the `plan-too-wide` example |
