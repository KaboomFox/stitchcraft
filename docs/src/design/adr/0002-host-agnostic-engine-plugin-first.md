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

## Consequences

- Useful on the machine before any upstream discussion.
- Previews in v1 are budget-limited (coarse levels for large fills); the CLI shows full fidelity.
- Two-step workflow (design in VectorCraft, export with the CLI) until exporter plug-ins exist.
- The compatibility gate is required, because we depend on a host we do not control.

## Alternatives considered

- **Fork VectorCraft and integrate natively now:** best UX, but a fork to maintain against a fast-moving
  upstream (~290k lines). Kept as phase 3.
- **Standalone app:** duplicates the editor. Rejected; the CLI covers headless needs.
