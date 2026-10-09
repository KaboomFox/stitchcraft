# Playbook: add a diagnostic

1. **Pick the code** from the right range ([diagnostics](../design/diagnostics.md#codes)); never reuse
   a retired one.
2. **Register it** in `stitchcraft-core`'s diagnostics table with severity, title and the long
   explanation: what it means, why it matters for the sew-out, how to fix it, an example.
3. **Emit it** where the problem is detected, with the element, a location when there is one, a
   specific message ("0.18 mm tall; rows are 0.25 mm apart") and a fix hint or applicable fix.
4. **Trigger it** in a test named `diag_sc_w0305_<what>`, in the crate that emits it: produce the code
   from real input through the public API and assert the whole text a user reads (`d.to_string()` and
   the fix). `cargo xtask conformance --check` fails until such a test exists.
5. **Regenerate docs:** `cargo xtask docs` adds the explanation page; consider a shot showing the
   problem and the fix.
