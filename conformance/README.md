# Conformance suite

Requirements (`requirements.toml`), cases (`cases/<area>/*.toml`), small fixtures (`fixtures/`), golden
outputs (`golden/`), Ink/Stitch facts (`inkstitch-params.toml`) and intended differences from Ink/Stitch
(`deviations.toml`). How it all works: [docs/src/design/conformance.md](../docs/src/design/conformance.md).

```sh
cargo xtask conformance --check   # consistency of requirements, cases and docs references
cargo xtask conformance           # run every case (from M1)
```
