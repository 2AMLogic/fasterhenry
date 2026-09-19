//! The published closed forms, exposed with plain scalar arguments.
//!
//! [`mutual_inductance`](super::mutual_inductance) dispatches to these
//! automatically; they are public so that they can be checked, benchmarked
//! and reused on their own. All lengths are in metres and all results in
//! henries.

use nalgebra::Vector3;

use super::{bar, lines, MU0_OVER_4PI};

/// Result of a closed form that is subject to cancellation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClosedForm {
    /// The partial inductance, in henries.
    pub value: f64,
    /// Estimated relative rounding error of `value`: machine epsilon times
    /// the ratio of the summed term magnitudes to the result. The formula is
    /// exact in exact arithmetic, so this is its only error.
    pub relative_error: f64,
}

/// Partial self-inductance of a straight bar of rectangular cross-section
/// carrying a uniform current density along its length — the exact closed
/// form of Hoer & Love (*J. Res. NBS* 69C, 1965) and Ruehli (*IBM J. Res.
/// Dev.* 16, 1972, eq. 15), of which the Grover/Rosa geometric-mean-distance
/// formula `μ0·l/(2π)·[ln(2l/(w+h)) + ½ + …]` is the thin-conductor limit.
///
/// The formula is an alternating sum that cancels heavily once the length is
/// more than a few hundred times the cross-section; consult
/// [`ClosedForm::relative_error`], or use
/// [`self_inductance`](super::self_inductance), which switches methods
/// automatically.
pub fn rectangular_bar_self(length: f64, width: f64, height: f64) -> ClosedForm {
    rectangular_bars([length; 2], [width; 2], [height; 2], [0.0; 3])
}

/// Partial mutual inductance of two parallel rectangular bars with aligned
/// cross-section axes and the same current direction (Hoer & Love 1965).
///
/// `offset` is the position of the second bar's centre relative to the
/// first's, along the length, width and height axes. Any offset is allowed:
/// the bars may be separate, touching, overlapping or identical.
pub fn rectangular_bars(
    length: [f64; 2],
    width: [f64; 2],
    height: [f64; 2],
    offset: [f64; 3],
) -> ClosedForm {
    let result = bar::bar_integral(
        &bar::corner_differences(length[0], length[1], offset[0]),
        &bar::corner_differences(width[0], width[1], offset[1]),
        &bar::corner_differences(height[0], height[1], offset[2]),
    );
    let areas = width[0] * height[0] * width[1] * height[1];
    ClosedForm {
        value: MU0_OVER_4PI * result.value / areas,
        relative_error: result.relative_error,
    }
}

/// Mutual inductance of two equal parallel filaments of negligible
/// cross-section, side by side a `distance` apart (Grover, *Inductance
/// Calculations*, the basic formula for equal parallel filaments; Rosa
/// 1908):
///
/// ```text
/// M = μ0/(2π) · [ l·asinh(l/d) − √(l² + d²) + d ]
/// ```
pub fn parallel_filaments(length: f64, distance: f64) -> f64 {
    2.0 * MU0_OVER_4PI * (length * (length / distance).asinh() - length.hypot(distance) + distance)
}

/// Mutual inductance of two parallel filaments of lengths `l1`, `l2` a
/// transverse `distance` apart, the second starting `axial_offset` beyond
/// the start of the first along their common direction (Grover's unequal
/// parallel filaments).
///
/// Returns `None` when the filaments are coaxial (`distance == 0`) and
/// overlap, where the integral diverges. Coaxial filaments that touch end to
/// end or are separated by a gap are fine.
pub fn parallel_filaments_offset(
    l1: f64,
    l2: f64,
    axial_offset: f64,
    distance: f64,
) -> Option<f64> {
    lines::parallel(l1, l2, axial_offset, distance).map(|k| MU0_OVER_4PI * k)
}

/// Mutual inductance of two filaments of negligible cross-section inclined
/// at any angle, `a0 → a1` and `b0 → b1`, including the `cos ε` of their
/// relative direction (Grover's filaments inclined at an angle, in the
/// general skew position).
///
/// Returns `None` for filaments within `1e-6` rad of parallel, where the
/// formula degenerates; use [`parallel_filaments_offset`] there.
pub fn inclined_filaments(a0: [f64; 3], a1: [f64; 3], b0: [f64; 3], b1: [f64; 3]) -> Option<f64> {
    let [a0, a1, b0, b1] = [a0, a1, b0, b1].map(Vector3::from);
    let cos = (a1 - a0).normalize().dot(&(b1 - b0).normalize());
    lines::skew(a0, a1, b0, b1, super::neumann::PARALLEL_SIN).map(|k| MU0_OVER_4PI * cos * k)
}
