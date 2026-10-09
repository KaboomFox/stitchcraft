//! The parameter registry: every [`params!`](stitchcraft_params::params) declaration, listed once.
//!
//! Declarations live beside the code that uses them; this list is what makes them visible to everything
//! else. Hosts look parameters up here (to check a design, to report keys StitchCraft does not know with
//! `SC-W0105`, to build a settings panel), and `cargo xtask docs` generates the parameter reference
//! pages, their JSON Schema and the Ink/Stitch compatibility contract's StitchCraft column from it.
//! It is a plain `const` list, so its order is fixed and nothing runs before `main` (ADR-0003).
//! Registering a declaration is one line here (`docs/src/contributing/playbook-new-param.md`).

use stitchcraft_params::ParamGroup;

use crate::common::CommonParams;
use crate::generate::StrokeParams;
use crate::generators::manual::ManualParams;
use crate::generators::passes::RepeatParams;
use crate::generators::running::RunningParams;

/// Every parameter group, in the order the reference pages list them.
pub const PARAMETERS: &[&ParamGroup] =
    &[&CommonParams::GROUP, &StrokeParams::GROUP, &RunningParams::GROUP, &RepeatParams::GROUP, &ManualParams::GROUP];

#[cfg(test)]
mod tests {
    use stitchcraft_params::audit;

    use super::*;

    #[test]
    fn req_prm_001_every_registered_parameter_is_complete() {
        assert_eq!(audit(PARAMETERS), Vec::<String>::new());
    }
}
