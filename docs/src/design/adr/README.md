# Architecture decision records

Short records of decisions that shape the code, with their context and the alternatives we rejected.
New ADRs are numbered sequentially and never renumbered; a decision that changes gets a new ADR that
supersedes the old one.

| ADR | Title | Status |
|---|---|---|
| [0001](0001-license-and-clean-room.md) | MIT OR Apache-2.0, clean-room from Ink/Stitch | Accepted |
| [0002](0002-host-agnostic-engine-plugin-first.md) | Host-agnostic engine; VectorCraft ABI v1 plug-in first | Accepted |
| [0003](0003-parameter-registry.md) | One parameter registry generates everything | Accepted |
| [0004](0004-determinism.md) | Determinism by construction | Accepted |
| [0005](0005-geometry-stack.md) | Geometry stack | Proposed (spike M0.7) |
| [0006](0006-docs-mdbook-diataxis.md) | mdBook, Diátaxis, generated reference, CI-regenerated images | Accepted |
| [0007](0007-pes-first-brother-profile.md) | PES v1 first, for the Brother 200 × 200 mm machine | Accepted |
| [0008](0008-conformance-first.md) | Conformance-first development | Accepted |
| [0009](0009-adopt-vectorcraft-conventions.md) | Adopt VectorCraft's conventions; improve the ones that drift | Accepted |
| [0010](0010-diagnostics-with-codes.md) | Coded diagnostics with explanation pages | Accepted |
| [0011](0011-movable-into-vectorcraft.md) | StitchCraft can move into VectorCraft's repository | Accepted |

`cargo xtask docs --check` fails if an ADR file is missing from this table or its status here differs
from the file's status line.
