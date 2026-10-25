# ADR-0002: Host-agnostic engine; VectorCraft ABI v1 plug-in first

**Status:** Accepted · 2026-10-08

## Context

VectorCraft offers sandboxed WebAssembly plug-ins (ABI v1): object filters and live effects, with
parameters stored per object as effect records, but no exporters, panels or overlays, and tight
budgets. Changing VectorCraft requires its maintainers' agreement. A machine needs files from day one.

## Decision

1. The engine (`stitchcraft-core` … `stitchcraft-render`) knows no host. Hosts adapt documents into a
   `Design` and consume a `StitchPlan`.
2. **Phase 1:** ship VectorCraft plug-ins on ABI v1 (live effects per stitch family, one tools filter)
   and produce machine files with `stitch export file.vectorcraft`. No VectorCraft change needed.
3. **Phase 2:** propose generic ABI v2 features upstream ([RFC](../rfc-vectorcraft-abi-v2.md)).
4. **Phase 3 (optional):** an in-tree VectorCraft crate as one more adapter.

**Amended at M6.4:** phase 1 drops the tools filter. VectorCraft's own commands make a satin column of 2
paths, add rungs and bake previews, and making a line into a column needs more instructions than ABI v1
gives a filter ([tools](../vectorcraft-integration.md#tools-vectorcrafts-own-commands)).

**Amended 2026-10-25:** VectorCraft took most of phase 2 into ABI v1 from 0.14.0 on, where an ABI v2 was
proposed ([status](../rfc-vectorcraft-abi-v2.md#status)). Exporters, rich parameters, data kept with the
document, heavy filters and a live effect's paints came that way. M6.8 to M6.12 use them, and M6.12 brings
the tools filter back as a heavy filter. Overlays, heavy live effects and the objects beside a live effect
are still proposed, in M9. M6.9's exporters end the two-step workflow below for VectorCraft 0.14 and
later, whose File › Export writes the machine file.

## Consequences

- Useful on the machine before any upstream discussion.
- Previews in v1 are budget-limited (coarse levels for large fills); the CLI shows full fidelity.
- Two-step workflow (design in VectorCraft, export with the CLI) until exporter plug-ins exist.
- The compatibility gate is required, because we depend on a host we do not control.

## Alternatives considered

- **Fork VectorCraft and integrate natively now:** best UX, but a fork to maintain against a fast-moving
  upstream (~290k lines). Kept as phase 3.
- **Standalone app:** duplicates the editor. Rejected; the CLI covers headless needs.
