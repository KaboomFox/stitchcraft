# Conformance suite

Requirements (`requirements.toml`), cases (`cases/<area>/*.toml`), small fixtures (`fixtures/`), golden
outputs (`golden/`), Ink/Stitch facts (`inkstitch-params.toml`) and intended differences from Ink/Stitch
(`deviations.toml`). How it all works: [docs/src/design/conformance.md](../docs/src/design/conformance.md).

```sh
cargo xtask conformance            # run every case; report in target/conformance/report.md
cargo xtask conformance --check    # consistency of requirements and cases, without running them
cargo xtask conformance --bless ID # rewrite one data case's golden files (say why in the PR)
```

`inkstitch/check_params.py` checks `inkstitch-params.toml` against the parameter declarations in an
Ink/Stitch checkout, field by field (see its header); run it whenever the contract moves to a newer
Ink/Stitch commit.

Rust tests named `req_<area>_<nnn>_<what>` are cases too: `req_plan_002_stitch_lengths` proves
`REQ-PLAN-002`. The golden files under `golden/testsheets/` are the exact bytes sewn at machine
checkpoints; `golden/formats/` holds the canonical plans of the format tests.
