//! Scale-aware validity checks for physical graded cells.

use super::DiscretizeError;
use crate::geometry::{LocalBasis, Segment};

/// Conservatively retain the kernel's documented longitudinal aspect envelope
/// in every dimension. This also exceeds its 1e-12 relative snapping tolerance
/// by five orders of magnitude; mere nonzero f64 extents would be insufficient.
const MIN_RELATIVE_EXTENT: f64 = 1e-7;

pub(super) fn validate(
    segment: &Segment,
    basis: &LocalBasis,
    widths: &[(f64, f64)],
    heights: &[(f64, f64)],
    (nw, nh, ratio): (usize, usize, f64),
) -> Result<(), DiscretizeError> {
    let shape_scale = segment.length().max(segment.width).max(segment.height);
    // An absolute-coordinate envelope bounds rounding even for rotated cells
    // whose projections would otherwise cancel large coordinate components.
    let envelope = segment.a.position().abs().sup(&segment.b.position().abs())
        + basis.width.abs() * (0.5 * segment.width)
        + basis.height.abs() * (0.5 * segment.height);
    for (axis, direction, cells) in [
        ("width", basis.width, widths),
        ("height", basis.height, heights),
    ] {
        let coordinate_scale = direction.abs().dot(&envelope);
        let minimum =
            (MIN_RELATIVE_EXTENT * shape_scale).max(32.0 * f64::EPSILON * coordinate_scale);
        let extent = cells
            .iter()
            .map(|&(_, size)| size)
            .fold(f64::INFINITY, f64::min);
        let valid = cells.iter().all(|&(centre, size)| {
            size.is_finite()
                && size > 0.0
                && size >= minimum
                && centre.is_finite()
                && centre - 0.5 * size < centre
                && centre + 0.5 * size > centre
        }) && cells.windows(2).all(|pair| pair[0].0 < pair[1].0);
        if !valid {
            return Err(DiscretizeError::UnusableGrading {
                axis,
                nw,
                nh,
                ratio,
                extent,
                minimum,
            });
        }
    }
    Ok(())
}
