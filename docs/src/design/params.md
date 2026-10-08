# Parameter registry

Every embroidery parameter is declared **once**, next to the generator that uses it. Everything
else that needs to know about parameters is generated from that declaration:

```text
                        ┌──▶ typed params struct used by the generator
                        ├──▶ validation (types, ranges, applicability) with coded diagnostics
params! { … }  ─────────┼──▶ VectorCraft plug-in manifests (schema per plug-in, ≤ 64 params)
(in the generator)      ├──▶ CLI: `stitch params`, `--param key=value`
                        ├──▶ SVG mapping (`inkstitch:<key>` attributes, read and write)
                        ├──▶ reference docs pages + JSON Schema (generated, checked in CI)
                        └──▶ proptest strategies for conformance and fuzzing
```

This is the answer to Ink/Stitch's drift between code labels and a hand-maintained docs dataset
([finding F6](inkstitch-analysis.md#f6--parameters-described-twice-by-hand)).

## Declaring parameters

Parameters are declared with a `macro_rules!` macro (no proc-macro crate, so contributors read plain
Rust and builds stay fast). Doc comments become the help text.

```rust,ignore
params! {
    /// Tatami fill.
    pub struct TatamiParams for StitchType::Tatami {
        group "Fill";

        /// Distance between rows of stitches. Smaller is denser.
        row_spacing: Length = mm(0.25), range mm(0.1)..=mm(10.0), key "row_spacing_mm";

        /// Angle of the rows, counter-clockwise from horizontal.
        angle: Angle = deg(0.0), key "angle";

        /// Longest stitch along a row before it is split.
        max_stitch_length: Length = mm(4.0), range mm(0.5)..=mm(12.0), key "max_stitch_length_mm";

        /// How many rows before the stitch pattern repeats; hides lines in large fills.
        staggers: Count = 4, range 1..=20, key "staggers";

        group "Fill underlay";

        /// Sew a sparse layer under the fill to stabilize the fabric.
        fill_underlay: Toggle = true, key "fill_underlay";
        …
    }
}
```

The macro expands to:

- `pub struct TatamiParams { pub row_spacing: Mm, pub angle: Deg, … }` with a
  `TatamiParams::from_set(&ParamSet) -> Result<Self, Vec<Diagnostic>>` constructor;
- a `static` slice of `ParamSpec` registered in the global registry (an ordered list, no
  life-before-main tricks: the registry is a plain `const` array assembled in one module, listing each
  generator's specs — adding a generator is one line there).

## `ParamSpec`

```rust,ignore
pub struct ParamSpec {
    pub key: &'static str,          // registry key == Ink/Stitch attribute name when one exists
    pub label: &'static str,        // short UI label (English source string, translation key)
    pub help: &'static str,         // from the doc comment; Markdown allowed
    pub kind: Kind,                 // Length, Angle, Percent, Count, Toggle, Choice, Seed, LengthList, Text
    pub group: &'static str,        // UI and docs grouping
    pub applies_to: &'static [StitchType],
    pub visible_when: Option<Condition>,   // e.g. lock_custom_start only when lock_start == custom
    pub stability: Stability,       // Stable | Experimental | Deprecated { since, use_instead }
    pub since: &'static str,        // first StitchCraft version that accepted it
    pub origin: Origin,             // InkStitch (same name and meaning) | InkStitchDeviates | StitchCraft
}
```

### Kinds and validation

| Kind | Storage | Validation | Ink/Stitch equivalent |
|---|---|---|---|
| `Length` | mm, `f64` | finite, within `range`; units accepted on input: mm, in, pt | `float`, unit `mm` |
| `Angle` | degrees | finite; normalized to (−180, 180] | `float`, unit `deg` |
| `Percent` | `f64` | finite, within range | `float`, unit `%` |
| `Count` | `u32` | within range | `int` |
| `Toggle` | `bool` | — | `boolean`, `toggle` |
| `Choice` | string id | one of the declared ids; unknown ids are a diagnostic, not a default | `combo`, `dropdown` |
| `Seed` | `u64` | any; empty means "derive from element id" | `random_seed` |
| `LengthList` | `Vec<Mm>` | 1–16 values, each in range (patterns like `"2.5 1.0"`, asymmetric per-side values) | `string`/`float` lists |
| `Text` | string | ≤ 4,096 bytes, kind-specific grammar (e.g. custom lock path) | `string` |

Invalid values never fall back silently to defaults: the element gets `SC-E0101` (wrong type) or
`SC-W0102` (clamped to range) with the key, the value and the accepted range.

## Naming rules

1. If Ink/Stitch has the parameter with the same meaning, **use its attribute name verbatim**
   (`row_spacing_mm`). This keeps SVG files compatible with no mapping table.
2. If our meaning differs, keep the name, mark `origin: InkStitchDeviates`, and document the difference
   in the [compatibility contract](inkstitch-compat-contract.md) deviations section.
3. New parameters follow the same style: `snake_case`, a unit suffix for lengths (`_mm`), percentages
   (`_percent`) and angles where Ink/Stitch does (`angle` alone for row angles).
4. Never reuse a removed key with a new meaning.
5. **Defaults are part of the contract.** For a parameter of Ink/Stitch origin the registry default
   equals Ink/Stitch's default, so an SVG that omits the attribute stitches the same way in both
   tools. Better starting values for new objects come from [presets](#presets) and machine profiles,
   never from changing a default.

## Generated outputs

| Output | Generated by | Checked by |
|---|---|---|
| Reference pages `docs/src/user/reference/params/*.md` | `cargo xtask docs` | `cargo xtask docs --check` (CI fails if stale) |
| `docs/src/user/reference/params.schema.json` | `cargo xtask docs` | same |
| VectorCraft manifests (embedded in each `.wasm`) | `build.rs` of the plug-in crate, from the registry | plug-in tests validate them against VectorCraft's manifest rules |
| CLI parameter help | at runtime from the registry | `trycmd` examples |
| SVG attribute read/write | `stitchcraft-svg` iterates the registry | round-trip property test `REQ-PRM-003` |
| `proptest` strategies | `stitchcraft-testkit` iterates the registry | used by every generator property test |

## Splitting for VectorCraft ABI v1

VectorCraft v1 allows at most 64 parameters per plug-in and has no groups or conditional visibility.
The plug-in crate therefore builds **one plug-in per stitch family** (`running`, `satin`, `fill`), each
with only the parameters that apply to it; the shared lock/trim parameters are included in each. A test
fails if any family's manifest exceeds 64 parameters or 64 KiB, so adding a parameter that would break
the limit is caught in the PR, not by a user. When VectorCraft supports richer schemas (ABI v2 RFC),
groups and conditions are emitted as well.

## Presets

A preset is a named partial `ParamSet` (`"denim"`, `"knit, light"`) stored as TOML in
`crates/stitchcraft-params/presets/`, validated by the same code as user input, and documented by a
generated page. In VectorCraft, users can also save appearance (including embroidery live effects) as
Graphic Styles.

## Versioning and migration

The registry has a schema version. A breaking change (renamed key, changed unit) ships a migration
function `vN → vN+1` and a conformance case with an old-format input. Unknown keys from newer files are
preserved on round trips where the host allows it, and reported once as `SC-W0105`.
