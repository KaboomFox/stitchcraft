# VectorCraft compatibility

- `vectorcraft.toml` pins the VectorCraft version of each track (stable, pre-release, `release` branch,
  `main`), with the SHA-256 of the stable release's Linux x86_64 archive.
- `supported.toml` records the releases that passed both levels. The pull request adopting a release
  writes it (`cargo xtask compat report`), and the user guide's supported-versions page is generated from
  it.
- The Level A contract tests are in `vc-contract/`. They load our `.wasm` plug-ins into VectorCraft's real
  plug-in host (`vectorcraft-plugins`) at the ref under test, *hello* and a module for each stitch family.
  It is a separate workspace, which `cargo xtask compat contract` builds after the modules.
- `scenarios/` has the Level B scenarios. VectorCraft's own command line, `vectorcraft-cli`, draws, applies
  the plug-ins' effects and saves, and StitchCraft checks how what it saved sews.
- `vc-window/` holds the window probe. It runs the stable release's app without a screen and compares its
  screenshots, and xa11y drives the app through its accessibility tree. A separate workspace, run by
  `cargo xtask compat window` in a desktop of its own (Xvfb, D-Bus and AT-SPI, on Linux). Its
  `tests/shots.rs` takes the docs' pictures of VectorCraft's menus and dialogs, in the same desktop, for
  `cargo xtask shots --vectorcraft`.

```sh
cargo xtask compat discover                          # latest stable, pre-release and branch heads
cargo xtask compat contract --ref v0.7.0             # builds the modules into target/compat/plugins
cargo xtask compat e2e --ref v0.12.0                 # Level B with that release's vectorcraft-cli
cargo xtask compat e2e --cli ../vectorcraft/target/release/vectorcraft-cli
cargo xtask compat track stable --ref v0.12.0        # both levels, and the result for the report
cargo xtask compat window                            # needs the packages .github/actions/desktop installs
cargo xtask shots --vectorcraft                      # the docs' VectorCraft pictures, in that desktop
```

Design: [docs/src/design/compatibility-gate.md](../docs/src/design/compatibility-gate.md), and for the
window probe [docs/src/design/docs-pipeline.md](../docs/src/design/docs-pipeline.md).
