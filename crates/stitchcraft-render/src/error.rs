//! Why a preview could not be drawn. Each error maps to a registered diagnostic
//! ([`RenderError::diagnostic`]), so hosts report it like any other problem.

use stitchcraft_core::{Code, Diagnostic, Exhausted, Fix};

/// Why a preview could not be drawn.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum RenderError {
    /// The plan never puts the needle down.
    #[error("the design has no stitches")]
    Empty,
    /// A position is more than 10 m from the hoop centre (beyond the machine grid).
    #[error("the design runs more than 10 metres from the hoop centre")]
    OutOfRange,
    /// The image would be larger than [`crate::MAX_SIDE`] pixels on a side.
    #[error("the preview would be {width} × {height} pixels; previews are at most {max} pixels on a side")]
    TooLarge {
        /// The image width at the requested scale.
        width: u64,
        /// The image height at the requested scale.
        height: u64,
        /// [`crate::MAX_SIDE`].
        max: u32,
        /// The largest scale, in pixels per millimetre, at which the preview fits.
        largest_scale: f32,
    },
    /// The work budget ran out.
    #[error(transparent)]
    Budget(#[from] Exhausted),
    /// The rasterizer or the PNG encoder refused something StitchCraft gave it: a StitchCraft bug.
    #[error("{0}")]
    Internal(String),
}

impl RenderError {
    /// The diagnostic hosts show.
    pub fn diagnostic(&self) -> Diagnostic {
        match self {
            RenderError::Empty => Diagnostic::new(Code::NothingToStitch, "The design has no stitches, so there is nothing to show."),
            RenderError::OutOfRange => Diagnostic::new(Code::PreviewTooLarge, "The design runs more than 10 metres from the hoop centre."),
            RenderError::TooLarge { width, height, max, largest_scale } => Diagnostic::new(
                Code::PreviewTooLarge,
                format!("The preview would be {width} × {height} pixels; previews are at most {max} pixels on a side."),
            )
            .with_fix(Fix::Hint(format!("Use a scale of at most {largest_scale} pixels per millimetre."))),
            RenderError::Budget(_) => Diagnostic::new(Code::BudgetExhausted, "Drawing the preview needed more work than the budget allows.")
                .with_fix(Fix::Hint("Use a smaller scale.".to_string())),
            RenderError::Internal(what) => Diagnostic::new(Code::InternalCheckFailed, format!("The preview could not be drawn: {what}.")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_become_registered_diagnostics() {
        let d = RenderError::TooLarge { width: 9000, height: 100, max: 4096, largest_scale: 3.6 }.diagnostic();
        assert_eq!(d.to_string(), "error SC-E0801: The preview would be 9000 × 100 pixels; previews are at most 4096 pixels on a side.");
        assert_eq!(d.fix.map(|f| f.describe()).as_deref(), Some("Use a scale of at most 3.6 pixels per millimetre."));
        assert_eq!(RenderError::Empty.diagnostic().code, Code::NothingToStitch);
        assert_eq!(RenderError::Budget(Exhausted::Work).diagnostic().code, Code::BudgetExhausted);
        assert_eq!(RenderError::Internal("x".into()).diagnostic().code, Code::InternalCheckFailed);
    }
}
