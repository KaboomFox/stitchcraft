//! What the measured guard says of details last (`REQ-AUTO-098`), with `SC-I0907`.
//!
//! A detail sewn after the larger areas round it (`REQ-AUTO-096`) keeps their rows off it, but it moves
//! fills in the sewing order, and that can make another spot thicker. So when the details' wishes change
//! the order, the design is planned both ways, and [`crate::guard::kept`] keeps the plan that sews them last
//! only when it is no thicker and has no more jumps or thread changes ([`crate::guard::Worse::ALL`]). This
//! module says why when it doesn't.

use stitchcraft_core::{Code, Diagnostic};

use crate::guard::Measured;

/// `SC-I0907` saying the details sew in the order found without their rule, the plan that sews them last
/// measuring `with`, or made with none, and the plan kept `without`.
pub(crate) fn worse(with: Option<Measured>, without: Measured) -> Diagnostic {
    let message = match with {
        Some(with) => format!(
            "Sewn after the larger areas round them, the details made the plan worse: its thickest spot {:.2} fills against {:.2}, {} jump \
             stitches against {} and {} thread changes against {}. They sew in the order found without that rule.",
            with.thickest, without.thickest, with.jumps, without.jumps, with.changes, without.changes
        ),
        None => "No plan came of sewing the details after the larger areas round them, so they sew in the order found without that rule.".to_string(),
    };
    Diagnostic::new(Code::AutoOrder, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_auto_098_the_guard_says_why_the_details_keep_the_order_found_without_them() {
        let without = Measured { thickest: 4.69, jumps: 57, changes: 8 };
        assert_eq!(
            worse(Some(Measured { thickest: 5.49, jumps: 58, changes: 9 }), without).message,
            "Sewn after the larger areas round them, the details made the plan worse: its thickest spot 5.49 fills against 4.69, 58 jump \
             stitches against 57 and 9 thread changes against 8. They sew in the order found without that rule."
        );
        assert_eq!(
            worse(None, without).message,
            "No plan came of sewing the details after the larger areas round them, so they sew in the order found without that rule."
        );
    }
}
