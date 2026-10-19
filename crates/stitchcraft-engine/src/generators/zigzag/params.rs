//! The zigzag stroke's parameters, declared with Ink/Stitch's keys and defaults, which are the
//! interoperability contract (`conformance/inkstitch-params.toml`). Strokes read `zigzag_spacing_mm` as a
//! list, as Ink/Stitch does, and satin columns as one length, so the key has a declaration for each of
//! those families (`REQ-PRM-005`, `docs/src/design/params.md`, rule 7). Meander fills read it as one length
//! that is 0 by default, and get a declaration of their own with the meander fill (M7.8).

use stitchcraft_params::{StitchType, params};

params! {
    /// Zigzag stroke: stitches across the path from side to side, as wide as the stroke.
    pub struct ZigzagStrokeParams for &[StitchType::ZigzagStitch];

    "Zigzag stroke" {
        /// How far it is along the path from one stitch to the next that crosses the same way, a zig and a
        /// zag together. Ink/Stitch takes several spacings separated by spaces, and sews the first.
        zigzag_spacing_mm: LengthList = "0.4", label "Zigzag spacing", range (0.01, 100.0);

        /// How much further out than half the stroke's width the needle goes on each side, to make up for
        /// the thread pulling the fabric in. 2 values set the 2 sides: the left of the way the path goes,
        /// then the right. Negative values make the zigzag narrower.
        stroke_pull_compensation_mm: LengthPair = "0", label "Pull compensation", range (-10.0, 10.0);

        /// Turns every stitch away from square to the path, in degrees: positive turns clockwise as the
        /// drawing shows it. On a curve the turned stitches point like the spokes of a wheel.
        zigzag_angle: BoundedAngle = "0", label "Zigzag angle", range (-89.0, 89.0);
    }
}
