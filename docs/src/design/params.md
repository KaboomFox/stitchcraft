# Parameter registry

<!-- implements: crates/stitchcraft-params/src/**, crates/stitchcraft-engine/src/registry.rs, crates/stitchcraft-engine/src/common.rs -->

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

Nothing about a parameter is written twice, so its label, help, range and default cannot drift apart
between the code, the docs and the user interfaces.

## Declaring parameters

Parameters are declared with a `macro_rules!` macro (no proc-macro crate, so contributors read plain
Rust and builds stay fast). The field name is the key; the doc comment is the help text, written for an
embroiderer. The real declaration of the settings every stitch type shares, in
`crates/stitchcraft-engine/src/common.rs`, starts like this:

```rust,ignore
params! {
    /// Settings every stitch type shares: lock stitches where the element's stitching starts and
    /// ends, a trim or a stop after it, and the shortest stitch and jump.
    pub struct CommonParams for StitchType::ALL;

    "Lock stitches" {
        /// Where the element gets lock stitches: …
        ties: Choice = "0", label "Lock stitches",
            options ["0" => "At the start and the end", "1" => "At the start", "2" => "At the end", "3" => "None"];

        /// The shape of the lock stitches at the start. …
        lock_start: Choice = "half_stitch", label "Start lock", choices LOCKS,
            origin Origin::InkStitchDeviates { deviation: "DEV-LCK-001" };

        /// The shape of a custom start lock: …
        lock_custom_start: Text = "", label "Custom start lock", when lock_start == "custom";

        /// How long each step of a start lock made of steps (back and forth, or custom) is.
        lock_start_scale_mm: Length = "0.7", label "Start lock size", range (0.1, 10.0),
            when lock_start in SIZED_IN_MM;
        …
    }
}
```

Each parameter is `key: Kind = "default", label "Label"`, followed by what its kind needs and what is
not the usual: `range (min, max)` for numbers and lists, `options [...]` or `choices CONST` for choices,
`applies` when it covers fewer stitch types than its struct, `defaults [Family::Fill => "4"]` when a
family of stitch types has a default of its own, `when other == "value"` (or `when other in VALUES`, a
list of them) when a user interface shows it only then, `origin` (default `Origin::InkStitch`) and
`stability` (default `Stability::Stable`).
A combination a kind does not take does not compile. Defaults are written as a design stores them, so
they are checked by the same code as a design's values. The grammar is in the `macros` module's
documentation.

The macro expands to:

- the typed struct, `pub struct CommonParams { pub ties: &'static str, pub lock_start_scale_mm: Mm, … }`,
  each field's type given by its kind (`kinds` module), so a field and its validation cannot disagree;
- `CommonParams::SPECS` (a `const` slice of `ParamSpec`) and `CommonParams::GROUP`;
- `CommonParams::from_set(&ParamSet) -> Result<Validated<CommonParams>, Vec<Diagnostic>>`: every
  parameter read from its text or its default, with the clamping warnings beside the values;
- `CommonParams::from_set_for(&ParamSet, Family)`: the same with the family's own defaults, which the
  engine reads an element's settings with.

The registry itself is `stitchcraft_engine::registry::PARAMETERS`, a plain `const` list of every
declaration's `GROUP` in the engine, next to the generators (no life-before-main tricks; adding a
generator's parameters is one line there). `stitchcraft-params` knows no stitch algorithm.

## `ParamSpec`

```rust,ignore
pub struct ParamSpec {
    pub key: &'static str,          // registry key == Ink/Stitch attribute name when one exists
    pub label: &'static str,        // short UI label (English source string, translation key)
    pub help: &'static str,         // the doc comment; `help()` gives the Markdown
    pub kind: Kind,                 // with its limits: range, options, maximum length
    pub default: &'static str,      // as a design stores it; "" for an empty optional value
    pub family_defaults: &'static [(Family, &'static str)], // a family's own default: `default_for`
    pub group: &'static str,        // UI and docs section
    pub applies_to: &'static [StitchType],
    pub visible_when: Option<Condition>,   // another key and the values it shows this for: lock_custom_start only for lock_start custom
    pub stability: Stability,       // Stable | Experimental | Deprecated { use_instead }
    pub origin: Origin,             // InkStitch | InkStitchDeviates { deviation } | StitchCraft
}
```

`InkStitchDeviates` names its entry in the deviations ledger (`conformance/deviations.toml`), which says
what differs and why; the reference pages show that entry, so a deviation is described once. A `since`
version and a registry schema version arrive with the first release, together with
[migrations](#versioning-and-migration).

### Kinds and validation

| Kind | Generators get | Validation | Ink/Stitch equivalent |
|---|---|---|---|
| `Length` | `Mm` | finite, within `range`; units accepted on input: mm, in, pt | `float`, unit `mm` |
| `OptionalLength` | `Option<Mm>` | as `Length`, or empty: the setting then comes from the document or the machine, or there is none. 0 or less counts as empty, as Ink/Stitch reads it | `float`, unit `mm`, no default |
| `Angle` | degrees, `f64` | finite; normalized to (−180, 180] | `float`, unit `deg` |
| `Percent` | `f64` | finite, within range | `float`, unit `%` |
| `Number` | `f64` | finite, within range, without a unit | `float`, no unit |
| `Count` | `u32` | whole, within range | `int` |
| `Toggle` | `bool` | `true`/`false` (also `yes`/`no`, `on`/`off`, `1`/`0`) | `boolean`, `toggle` |
| `Choice` | `&'static str` (the id) | one of the declared ids; unknown ids are a diagnostic, not a default | `combo`, `dropdown` |
| `Seed` | `Option<u64>` | any; empty means "derive from the element"; text that is not a number is hashed (FNV-1a) | `random_seed` |
| `LengthList` | `Vec<Mm>` | 1 to 16 values, each in range (patterns like `"2.5 1.0"`) | `string`/`float` lists |
| `LengthPair` | `[Mm; 2]` | 1 value for both sides, or 2 for the first side and the second (a satin's rails, or its start and end). Each is in range, with units as for `Length` | `float`, unit `mm (each side)` |
| `PercentPair` | `[f64; 2]` | as `LengthPair`, in percent | `float`, unit `% (each side)` |
| `OptionalLengthPair`, `OptionalPercentPair` | `Option<[Mm; 2]>`, `Option<[f64; 2]>` | as `LengthPair` and `PercentPair`, or empty: the setting then comes from another one, as a zigzag underlay's inset comes from the contour's. 0 is a value, as Ink/Stitch reads it | `float`, unit `mm (each side)` or `% (each side)`, no default |
| `PercentList` | `Vec<f64>` | 1 to 16 percentages, each in range (levels like `"15 30"`) | `float`, unit `%`, read as a list |
| `CountList` | `Vec<u32>` | 1–16 whole numbers, each in range | `string`/`int` lists |
| `Text` | `String` | ≤ 4,096 bytes; kind-specific grammar (e.g. a custom lock path) is checked by its user | `string` |

Invalid values never fall back silently to defaults: the element gets `SC-E0101` (not a value of the
kind; the element cannot be stitched) or `SC-W0102` (clamped into range), with the key, the value and what
is accepted. Keys the registry does not know are kept, so they round-trip, and reported once as
`SC-W0105`. Parsing lives in one place (`value` module), so every host reads text the same way: an SVG
attribute, a command-line `key=value`, a VectorCraft effect record.

### Completeness (REQ-PRM-001)

A declaration that compiles can still be wrong: an empty range, a default outside it, a repeated option,
a condition on a parameter that does not exist. `stitchcraft_params::audit` lists every such problem, and
the engine's `req_prm_001_every_registered_parameter_is_complete` test requires the list to be empty.
`cargo xtask docs --check` adds the cross-checks with outside data: a parameter of Ink/Stitch origin must
be in the [compatibility contract](inkstitch-compat-contract.md) with a compatible type and the same
default; one of StitchCraft's own must not take an Ink/Stitch key; deviations must name ledger entries;
the lock and method identifiers must equal Ink/Stitch's.

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
6. **A key belongs to its stitch types.** Ink/Stitch gives some keys to several elements with their own
   defaults: `running_stitch_tolerance_mm` is 0.2 mm on a stroke and 0.1 mm on a satin or a fill. The
   contract is per key and stitch type, so the contract page matches a declaration with the rows for the
   stitch types it applies to, and a declaration for a stitch type Ink/Stitch does not give the key is
   reported. A key is declared once, and a family whose default differs gets its own with `defaults`:
   `max_stitch_length_mm` is empty on manual stitch and satin columns and 4 mm on fills. The engine reads
   an element's settings with its family's defaults (`from_set_for`). The contract page compares each row
   with the default of the row's family. The reference page lists each family's default, and the JSON
   Schema gives them under `x-stitchcraft.family_defaults`.

## Generated outputs

| Output | Generated by | Checked by |
|---|---|---|
| Reference pages: the [index](../user/reference/params.md) and `docs/src/user/reference/params/*.md`, one per declaration | `cargo xtask docs` | `cargo xtask docs --check` (CI fails if stale; pages no declaration produces are deleted) |
| `docs/src/user/reference/params.schema.json`: the parameters as JSON values (numbers, booleans, strings, arrays, `null` for empty), for editors, presets and JSON hosts | `cargo xtask docs` | same |
| The *StitchCraft* column of the [Ink/Stitch compatibility contract](inkstitch-compat-contract.md) | `cargo xtask docs` | same, plus the cross-checks above |
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

A preset is a named partial `ParamSet` (`"denim"`, `"knit, light"`) stored as a TOML file in a
`presets` directory of the parameters crate. The same code as user input validates it, and a generated
page documents it. In VectorCraft, users can also save appearance (including embroidery live effects) as
Graphic Styles.

## Versioning and migration

The registry has a schema version. A breaking change (renamed key, changed unit) ships a migration
function `vN → vN+1` and a conformance case with an old-format input. Unknown keys from newer files are
preserved on round trips where the host allows it, and reported once as `SC-W0105`.
