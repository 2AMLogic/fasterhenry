//! Semi-analytic Neumann volume integral for two parallel rectangular bars
//! with aligned cross-section axes — the well-conditioned companion of the
//! 64-term closed form in [`super::bar`].
//!
//! The two length integrations are done exactly with the parallel-filament
//! closed form `K(ρ)` of [`super::lines::parallel`], which has no
//! cancellation problem for long thin bars. That leaves a fourfold integral
//! over the two cross-sections of a function of the coordinate differences
//! `u = y' − y`, `v = z' − z` only, which collapses to a double integral
//!
//! ```text
//! ∫∫ T_y(u) · T_z(v) · K(√(u² + v²)) du dv
//! ```
//!
//! where `T(u)` is the length of the set of `y` with `y` in the first
//! interval and `y + u` in the second — a trapezoid, linear between four
//! break points. The integral is evaluated panel by panel between those break
//! points with Gauss–Legendre rules:
//!
//! * `K` is analytic away from `ρ = 0` with a logarithmic (and conical)
//!   singularity there, so a panel is integrated directly only once it is no
//!   larger than its distance from the origin; larger panels are bisected,
//!   which grades the mesh geometrically towards the origin.
//! * When the bars touch or overlap the origin lies in the domain. Break
//!   points are inserted at `u = 0`, `v = 0` so that it is always a panel
//!   *corner*, and such panels are integrated with a Duffy transformation
//!   (which turns `ln ρ` into `ln s` plus a smooth function) followed by the
//!   substitution `s = τ³` (which smooths `s·ln s` to `τ⁵·ln τ`).
//!
//! This is what makes self terms and touching neighbours of extreme aspect
//! ratio computable to near machine precision.

use super::gauss;
use super::lines;

/// Two parallel bars in the frame of the first one.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AlignedBars {
    /// Lengths.
    pub(crate) length: [f64; 2],
    /// Extents along the shared width axis.
    pub(crate) width: [f64; 2],
    /// Extents along the shared height axis.
    pub(crate) height: [f64; 2],
    /// Offset of the second bar's *centre* from the first's, along the
    /// length, width and height axes.
    pub(crate) offset: [f64; 3],
}

impl AlignedBars {
    /// A bar paired with itself.
    pub(crate) fn same(length: f64, width: f64, height: f64) -> Self {
        Self {
            length: [length; 2],
            width: [width; 2],
            height: [height; 2],
            offset: [0.0; 3],
        }
    }

    /// Axial position of the second bar's start relative to the first's.
    fn axial_start(&self) -> f64 {
        self.offset[0] + 0.5 * (self.length[0] - self.length[1])
    }
}

/// Order of the rule on panels at least their own size away from the origin
/// (Bernstein parameter ≥ 2 + √5, so the error is ≲ 4.2⁻²⁰ ≈ 3·10⁻¹³).
const FAR_ORDER: usize = 10;
/// Order of the rules in both Duffy coordinates on panels at the origin.
const CORNER_ORDER: usize = 16;
/// Differences closer to zero than this fraction of the combined extent are
/// snapped to zero, so that touching bars meet exactly.
const SNAP: f64 = 1e-12;

/// The trapezoid `T(u)` for intervals of extent `e1`, `e2` with centres
/// `offset` apart.
fn overlap_weight(e1: f64, e2: f64, offset: f64, u: f64) -> f64 {
    let hi = (0.5 * e1).min(offset + 0.5 * e2 - u);
    let lo = (-0.5 * e1).max(offset - 0.5 * e2 - u);
    (hi - lo).max(0.0)
}

/// Sorted break points of `T` on its support, with zero inserted when it
/// lies inside.
fn break_points(e1: f64, e2: f64, offset: f64) -> Vec<f64> {
    let (sum, diff) = (0.5 * (e1 + e2), 0.5 * (e1 - e2).abs());
    let snap = |x: f64| if x.abs() <= SNAP * sum { 0.0 } else { x };
    let mut points = vec![
        snap(offset - sum),
        snap(offset - diff),
        snap(offset + diff),
        snap(offset + sum),
    ];
    if points[0] < 0.0 && points[3] > 0.0 {
        points.push(0.0);
    }
    points.sort_by(f64::total_cmp);
    points.dedup();
    points
}

struct Integrator<'a> {
    bars: &'a AlignedBars,
    axial_start: f64,
    /// Smallest non-zero axial end-to-end separation: the scale below which
    /// the smooth part of `K` is resolved on a panel at the origin.
    axial_scale: f64,
}

impl Integrator<'_> {
    fn integrand(&self, u: f64, v: f64) -> f64 {
        let b = self.bars;
        let weight = overlap_weight(b.width[0], b.width[1], b.offset[1], u)
            * overlap_weight(b.height[0], b.height[1], b.offset[2], v);
        if weight == 0.0 {
            return 0.0;
        }
        // Quadrature nodes are interior, so ρ > 0 and the value exists.
        let kernel = lines::parallel(b.length[0], b.length[1], self.axial_start, u.hypot(v));
        weight * kernel.unwrap_or(0.0)
    }

    /// Tensor Gauss–Legendre on a panel that is resolved.
    fn direct(&self, u: [f64; 2], v: [f64; 2]) -> f64 {
        let rule = gauss::rule(FAR_ORDER);
        let mut sum = 0.0;
        for (uu, wu) in rule.on(u[0], u[1]) {
            for (vv, wv) in rule.on(v[0], v[1]) {
                sum += wu * wv * self.integrand(uu, vv);
            }
        }
        sum
    }

    /// Duffy quadrature on the panel with one corner at the origin and the
    /// opposite corner at `(a, b)` (either sign).
    fn corner(&self, a: f64, b: f64) -> f64 {
        let rule = gauss::rule(CORNER_ORDER);
        let mut sum = 0.0;
        for (tau, w_tau) in rule.on(0.0, 1.0) {
            let s = tau * tau * tau;
            // ds = 3τ² dτ, and the Duffy Jacobian is |a·b|·s.
            let radial = w_tau * 3.0 * tau * tau * s;
            for (t, w_t) in rule.on(0.0, 1.0) {
                let value = self.integrand(a * s, b * s * t) + self.integrand(a * s * t, b * s);
                sum += radial * w_t * value;
            }
        }
        sum * (a * b).abs()
    }

    fn panel(&self, u: [f64; 2], v: [f64; 2], depth: u32) -> f64 {
        let (su, sv) = (u[1] - u[0], v[1] - v[0]);
        if su <= 0.0 || sv <= 0.0 {
            return 0.0;
        }
        let gap = |r: [f64; 2]| {
            if r[0] > 0.0 {
                r[0]
            } else if r[1] < 0.0 {
                -r[1]
            } else {
                0.0
            }
        };
        let distance = gap(u).hypot(gap(v));
        let size = su.max(sv);
        // Recursion is geometric, so 200 levels are never reached; the guard
        // only makes termination unconditional.
        if depth >= 200 {
            return self.direct(u, v);
        }
        let split_u =
            |at: f64| self.panel([u[0], at], v, depth + 1) + self.panel([at, u[1]], v, depth + 1);
        let split_v =
            |at: f64| self.panel(u, [v[0], at], depth + 1) + self.panel(u, [at, v[1]], depth + 1);
        if distance > 0.0 {
            return if size <= distance {
                self.direct(u, v)
            } else if su >= sv {
                split_u(0.5 * (u[0] + u[1]))
            } else {
                split_v(0.5 * (v[0] + v[1]))
            };
        }
        // The origin is a corner of this panel: make it roughly square and no
        // larger than the axial scale, then apply the Duffy rule.
        let target = su.min(sv).min(self.axial_scale);
        let toward = |r: [f64; 2], len: f64| if r[0] == 0.0 { len } else { -len };
        if su > 2.0 * target {
            return split_u(toward(u, target));
        }
        if sv > 2.0 * target {
            return split_v(toward(v, target));
        }
        let far = |r: [f64; 2]| if r[0] == 0.0 { r[1] } else { r[0] };
        self.corner(far(u), far(v))
    }
}

/// `∫∫ dV dV' / |r − r'|` over the two bars, in m⁵.
pub(crate) fn integral(bars: &AlignedBars) -> f64 {
    let axial_start = bars.axial_start();
    let (l1, l2) = (bars.length[0], bars.length[1]);
    let separations = [
        axial_start + l2,
        axial_start,
        axial_start + l2 - l1,
        axial_start - l1,
    ];
    let largest = separations.iter().fold(0.0_f64, |m, q| m.max(q.abs()));
    let axial_scale = separations
        .iter()
        .map(|q| q.abs())
        .filter(|&q| q > 1e-9 * largest)
        .fold(f64::INFINITY, f64::min);
    let integrator = Integrator {
        bars,
        axial_start,
        axial_scale,
    };
    let us = break_points(bars.width[0], bars.width[1], bars.offset[1]);
    let vs = break_points(bars.height[0], bars.height[1], bars.offset[2]);
    let mut sum = 0.0;
    for u in us.windows(2) {
        for v in vs.windows(2) {
            sum += integrator.panel([u[0], u[1]], [v[0], v[1]], 0);
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inductance::bar::{bar_integral, corner_differences};

    fn closed_form(b: &AlignedBars) -> (f64, f64) {
        let result = bar_integral(
            &corner_differences(b.length[0], b.length[1], b.offset[0]),
            &corner_differences(b.width[0], b.width[1], b.offset[1]),
            &corner_differences(b.height[0], b.height[1], b.offset[2]),
        );
        (result.value, result.relative_error)
    }

    /// The two methods share nothing but the problem statement, so their
    /// agreement validates both. The closed form's own rounding-error
    /// estimate is allowed for on top of `tolerance`.
    fn assert_agree(b: &AlignedBars, tolerance: f64) {
        let (want, estimate) = closed_form(b);
        assert!(estimate < 1e-8, "reference too noisy ({estimate:e}): {b:?}");
        let got = integral(b);
        let error = ((got - want) / want).abs();
        assert!(
            error < tolerance + estimate,
            "{b:?}: {got} vs {want}, error {error:e}, estimate {estimate:e}"
        );
    }

    #[test]
    fn trapezoid_weight_integrates_to_the_product_of_extents() {
        for (e1, e2, offset) in [(1.0, 1.0, 0.0), (1.0, 0.3, 0.2), (0.4, 2.0, -3.0)] {
            let points = break_points(e1, e2, offset);
            let total: f64 = points
                .windows(2)
                .flat_map(|p| gauss::rule(2).on(p[0], p[1]))
                .map(|(u, w)| w * overlap_weight(e1, e2, offset, u))
                .sum();
            assert!((total - e1 * e2).abs() < 1e-14, "{e1} {e2} {offset}");
        }
    }

    #[test]
    fn self_term_matches_the_closed_form() {
        for (l, w, h) in [
            (1.0, 1.0, 1.0),
            (1.0, 0.5, 0.25),
            (1.0, 0.1, 0.1),
            (1.0, 0.2, 0.02),
            (0.05, 1.0, 0.7),
            (0.3, 2.0, 0.05),
        ] {
            assert_agree(&AlignedBars::same(l, w, h), 1e-9);
        }
    }

    #[test]
    fn touching_overlapping_and_separated_bars_match_the_closed_form() {
        let base = AlignedBars::same(1.0, 0.2, 0.1);
        let cases = [
            // Side-by-side neighbours sharing a face, as in a bundle.
            [0.0, 0.2, 0.0],
            [0.0, 0.0, 0.1],
            // Diagonal neighbours sharing an edge.
            [0.0, 0.2, 0.1],
            // Partially overlapping volumes.
            [0.3, 0.05, 0.02],
            // Separated in every direction.
            [0.4, 0.5, 0.3],
            // Consecutive collinear bars sharing an end face.
            [1.0, 0.0, 0.0],
            // Collinear with a gap.
            [1.5, 0.0, 0.0],
        ];
        for offset in cases {
            assert_agree(&AlignedBars { offset, ..base }, 1e-9);
        }
    }

    #[test]
    fn unequal_bars_match_the_closed_form() {
        let bars = AlignedBars {
            length: [1.0, 0.6],
            width: [0.3, 0.1],
            height: [0.05, 0.2],
            offset: [0.1, 0.15, -0.05],
        };
        assert_agree(&bars, 1e-9);
        let nested = AlignedBars {
            offset: [0.0, 0.0, 0.0],
            ..bars
        };
        assert_agree(&nested, 1e-9);
    }
}
