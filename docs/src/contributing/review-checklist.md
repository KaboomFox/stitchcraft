# Review checklist

CI checks the mechanical rules. Reviewers check what machines cannot:

**Behaviour**
- [ ] The PR implements exactly its roadmap step; requirements it touches are listed and `active`.
- [ ] New behaviour has conformance cases, including degenerate inputs and each new diagnostic.
- [ ] Golden changes are explained and expected (label `golden-change` + changelog line).
- [ ] No silent fallback: every degraded path emits a coded diagnostic.

**Design**
- [ ] Code sits in the right crate ([where does my code go?](../design/architecture.md#where-does-my-code-go)).
- [ ] Types make invalid states unrepresentable; functions are total.
- [ ] Loops over data charge the budget; recursion is bounded.
- [ ] Modules have one job; names say what things are in embroidery terms.

**Provenance**
- [ ] Nothing was copied, pasted or transliterated from Ink/Stitch or other GPL/AGPL code; behaviour is in the design docs in our own words.
- [ ] New data tables (palettes, format constants) cite their source.

**Docs and UX**
- [ ] User-facing docs updated in this PR; help text in the registry reads well for an embroiderer.
- [ ] New prose follows the [writing style](writing-style.md), and `cargo xtask prose` passes.
- [ ] Shots refreshed if pictures should change; alt text is meaningful.
- [ ] Messages are specific, actionable and free of internal jargon.

**Machine**
- [ ] If the change affects what sews, it is listed for the next machine checkpoint.
