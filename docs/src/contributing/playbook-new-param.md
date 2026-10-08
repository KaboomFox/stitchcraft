# Playbook: add or change a parameter

1. **Name it.** If Ink/Stitch has the same parameter, use its attribute name, meaning and default
   ([naming rules](../design/params.md#naming-rules)); otherwise follow the same style.
2. **Declare it** in the generator's `params!` block: kind, default, range, group, visibility condition,
   doc comment (the help users will read — write it for an embroiderer, mention units and the visible
   effect).
3. **Use it** through the typed struct; never read raw `ParamSet` values in a generator.
4. **Test it:** a conformance case where the parameter changes the result in the documented direction,
   plus a property test if it has a monotonic effect.
5. **Check limits:** the plug-in test fails if the family's VectorCraft manifest would exceed 64
   parameters; if it does, discuss moving rarely used parameters to a preset or to ABI v2.
6. **Regenerate docs:** `cargo xtask docs`; add a before/after shot if the effect is visual.
7. **Changing** an existing parameter's meaning, unit or default is a breaking change: add a migration
   and a case with old input, and a changelog line.
