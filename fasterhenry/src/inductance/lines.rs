//! Closed forms of the Neumann double line integral `∫∫ ds ds' / |r − r'|`
//! between two straight filaments of negligible cross-section.
//!
//! Both follow from integrating `1/R` twice, which is elementary:
//!
//! * **Parallel filaments** (Grover, *Inductance Calculations*, the chapters
//!   on equal and on unequal parallel filaments; Rosa, "The self and mutual
//!   inductances of linear conductors", *Bull. Bur. Stand.* 4 (1908)). With
//!   `Φ(q) = q·asinh(q/ρ) − √(q² + ρ²)`, for which
//!   `Φ'' = 1/√(q² + ρ²)`, the integral is the second difference of
//!   `Φ` over the four end-to-end axial separations. For equal filaments side
//!   by side it reduces to the textbook
//!   `2·[l·asinh(l/ρ) − √(l² + ρ²) + ρ]`.
//! * **Skew filaments** (Grover, the chapter on filaments inclined at an
//!   angle to each other, after Martens and Campbell). In
//!   coordinates `s`, `t` measured along each line from the feet of their
//!   common perpendicular of length `d`, with `ε` the angle between them,
//!   `R² = s² + t² − 2st·cos ε + d²` and
//!   `F = s·ln(R + t − s·cos ε) + t·ln(R + s − t·cos ε)
//!        − (d/sin ε)·atan((d²·cos ε + s·t·sin²ε) / (d·R·sin ε))`
//!   satisfies `∂²F/∂s∂t = 1/R`.
//!
//! Both primitives were verified symbolically against their defining
//! differential identities, and against direct numerical integration, before
//! being committed.

use nalgebra::Vector3;

/// `|q|·ln(|q| + √(q² + ρ²)) − √(q² + ρ²)`: the parallel-filament primitive
/// with its `−|q|·ln ρ` part split off, so that it stays finite as `ρ → 0`.
fn parallel_primitive(q: f64, rho_sq: f64) -> f64 {
    let q = q.abs();
    let root = (q * q + rho_sq).sqrt();
    if q == 0.0 {
        -root
    } else {
        q * (q + root).ln() - root
    }
}

/// Length over which the axial ranges `[0, l1]` and `[offset, offset + l2]`
/// overlap.
pub(crate) fn axial_overlap(l1: f64, l2: f64, offset: f64) -> f64 {
    (l1.min(offset + l2) - offset.max(0.0)).max(0.0)
}

/// Neumann integral of two parallel filaments a transverse distance `rho`
/// apart: one spanning `[0, l1]` along the common axis, the other
/// `[offset, offset + l2]`.
///
/// Returns `None` for the one divergent configuration — coaxial filaments
/// (`rho = 0`) that overlap over a finite length. Collinear filaments that
/// merely touch end to end are finite and handled.
pub(crate) fn parallel(l1: f64, l2: f64, offset: f64, rho: f64) -> Option<f64> {
    let rho_sq = rho * rho;
    let smooth = parallel_primitive(offset + l2, rho_sq)
        - parallel_primitive(offset, rho_sq)
        - parallel_primitive(offset + l2 - l1, rho_sq)
        + parallel_primitive(offset - l1, rho_sq);
    // Σ ±|q| = 2·overlap exactly; computing it geometrically keeps the
    // logarithm out of the (finite) collinear case.
    let overlap = axial_overlap(l1, l2, offset);
    if overlap == 0.0 {
        Some(smooth)
    } else if rho > 0.0 {
        Some(smooth - 2.0 * overlap * rho.ln())
    } else {
        None
    }
}

/// `x·ln(R + a)` for `R ≥ |a|`, where `R² − a² = residual_sq`, evaluated
/// without cancellation when `a < 0` and with `0·ln 0 = 0`.
fn skew_log_term(x: f64, big_r: f64, a: f64, residual_sq: f64) -> f64 {
    if x == 0.0 {
        return 0.0;
    }
    let argument = if a >= 0.0 {
        big_r + a
    } else {
        residual_sq / (big_r - a)
    };
    if argument > 0.0 {
        x * argument.ln()
    } else {
        // Only reachable at the integrable corner singularity of two lines
        // that meet, where the coefficient's limit is zero.
        0.0
    }
}

/// The skew-filament primitive `F(s, t)`.
///
/// For nearly parallel lines `s` and `t` are both huge (the common
/// perpendicular is far away) and nearly equal, so `R²` and `t − s·cos ε` are
/// formed from `s − t` and `1 − cos ε = sin²ε/(1 + cos ε)` rather than by
/// subtracting large products.
fn skew_primitive(s: f64, t: f64, d: f64, cos: f64, sin: f64) -> f64 {
    let sin_sq = sin * sin;
    // (1 ∓ cos), whichever is small, without cancellation.
    let versine = sin_sq / (1.0 + cos.abs());
    let (planar_sq, a_s, a_t) = if cos >= 0.0 {
        let diff = s - t;
        (
            diff * diff + 2.0 * s * t * versine,
            s * versine - diff,
            t * versine + diff,
        )
    } else {
        let sum = s + t;
        (
            sum * sum - 2.0 * s * t * versine,
            sum - s * versine,
            sum - t * versine,
        )
    };
    let big_r = (planar_sq + d * d).max(0.0).sqrt();
    let logs = skew_log_term(s, big_r, a_s, s * s * sin_sq + d * d)
        + skew_log_term(t, big_r, a_t, t * t * sin_sq + d * d);
    if d == 0.0 || big_r == 0.0 {
        return logs;
    }
    logs - (d / sin) * ((d * d * cos + s * t * sin_sq) / (d * big_r * sin)).atan()
}

/// Neumann integral of two non-parallel filaments, `a0 → a1` and `b0 → b1`.
///
/// `sin_min` is the smallest `|sin ε|` the caller accepts, below which `None`
/// is returned: the feet of the common perpendicular run off to infinity as
/// the lines become parallel, and locating them costs a relative rounding
/// error of up to about `1e-17 / sin²ε` — `1e-9` at `1e-4` rad, `1e-7` at
/// `1e-5` rad, `1e-5` at the `1e-6` rad limit used by the dispatcher. The
/// integral of two lines that cross or share an end point is
/// finite and handled.
pub(crate) fn skew(
    a0: Vector3<f64>,
    a1: Vector3<f64>,
    b0: Vector3<f64>,
    b1: Vector3<f64>,
    sin_min: f64,
) -> Option<f64> {
    let (la, lb) = ((a1 - a0).norm(), (b1 - b0).norm());
    let (u, v) = ((a1 - a0) / la, (b1 - b0) / lb);
    let cos = u.dot(&v);
    let sin = u.cross(&v).norm();
    if sin.is_nan() || sin < sin_min {
        return None;
    }
    // Feet of the common perpendicular: minimise |a0 + s·u − b0 − t·v|.
    let w0 = a0 - b0;
    let (du, dv) = (u.dot(&w0), v.dot(&w0));
    let sin_sq = sin * sin;
    let s0 = (cos * dv - du) / sin_sq;
    let t0 = (dv - cos * du) / sin_sq;
    let d = (w0 + u * s0 - v * t0).norm();
    // Below rounding level the lines are coplanar; the atan term's limit is 0.
    let d = if d <= 1e-14 * (la + lb + w0.norm()) {
        0.0
    } else {
        d
    };
    let (s_lo, s_hi) = (-s0, la - s0);
    let (t_lo, t_hi) = (-t0, lb - t0);
    Some(
        skew_primitive(s_hi, t_hi, d, cos, sin)
            - skew_primitive(s_hi, t_lo, d, cos, sin)
            - skew_primitive(s_lo, t_hi, d, cos, sin)
            + skew_primitive(s_lo, t_lo, d, cos, sin),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inductance::gauss;

    /// Brute-force Gauss–Legendre evaluation of the Neumann integral on
    /// `panels × panels` sub-intervals — independent of both closed forms.
    fn brute(
        a0: Vector3<f64>,
        a1: Vector3<f64>,
        b0: Vector3<f64>,
        b1: Vector3<f64>,
        panels: usize,
    ) -> f64 {
        let rule = gauss::rule(16);
        let (la, lb) = ((a1 - a0).norm(), (b1 - b0).norm());
        let mut sum = 0.0;
        for pa in 0..panels {
            for pb in 0..panels {
                let (s0, s1) = (pa as f64 / panels as f64, (pa + 1) as f64 / panels as f64);
                let (t0, t1) = (pb as f64 / panels as f64, (pb + 1) as f64 / panels as f64);
                for (s, ws) in rule.on(s0, s1) {
                    for (t, wt) in rule.on(t0, t1) {
                        let r = (a0 + (a1 - a0) * s - b0 - (b1 - b0) * t).norm();
                        sum += ws * wt / r;
                    }
                }
            }
        }
        sum * la * lb
    }

    fn v(x: f64, y: f64, z: f64) -> Vector3<f64> {
        Vector3::new(x, y, z)
    }

    #[test]
    fn equal_side_by_side_filaments_match_the_textbook_formula() {
        for (l, rho) in [(1.0_f64, 0.1_f64), (1.0, 1.0), (1.0, 25.0), (3e-3, 2e-6)] {
            let want = 2.0 * (l * (l / rho).asinh() - l.hypot(rho) + rho);
            let got = parallel(l, l, 0.0, rho).unwrap();
            assert!(((got - want) / want).abs() < 1e-12, "l = {l}, ρ = {rho}");
        }
    }

    #[test]
    fn parallel_formula_matches_brute_force_for_offset_unequal_filaments() {
        for (l1, l2, offset, rho) in [
            (1.0, 0.6, 0.3, 0.2),
            (1.0, 2.0, -0.7, 0.5),
            (1.0, 0.5, 1.4, 0.3),
            (0.8, 1.1, -2.5, 0.25),
        ] {
            let got = parallel(l1, l2, offset, rho).unwrap();
            let want = brute(
                v(0.0, 0.0, 0.0),
                v(l1, 0.0, 0.0),
                v(offset, rho, 0.0),
                v(offset + l2, rho, 0.0),
                8,
            );
            assert!(
                ((got - want) / want).abs() < 1e-9,
                "({l1}, {l2}, {offset}, {rho}): {got} vs {want}"
            );
        }
    }

    #[test]
    fn collinear_filaments_are_finite_unless_they_overlap() {
        // Touching end to end: l1·ln((l1+l2)/l1) + l2·ln((l1+l2)/l2).
        let (l1, l2) = (1.0_f64, 0.4_f64);
        let want = l1 * ((l1 + l2) / l1).ln() + l2 * ((l1 + l2) / l2).ln();
        let got = parallel(l1, l2, l1, 0.0).unwrap();
        assert!((got - want).abs() < 1e-14, "{got} vs {want}");
        // Separated by a gap g: second difference of q·ln q.
        let g = 0.25_f64;
        let xlnx = |q: f64| q * q.ln();
        let want = xlnx(l1 + g + l2) - xlnx(g + l2) - xlnx(l1 + g) + xlnx(g);
        let got = parallel(l1, l2, l1 + g, 0.0).unwrap();
        assert!((got - want).abs() < 1e-14, "{got} vs {want}");
        // Overlapping coaxial filaments diverge.
        assert!(parallel(l1, l2, 0.5, 0.0).is_none());
        assert!(parallel(l1, l1, 0.0, 0.0).is_none());
    }

    #[test]
    fn skew_formula_matches_brute_force() {
        let cases = [
            // General position.
            (
                v(0.0, 0.0, 0.0),
                v(1.0, 0.2, -0.1),
                v(0.3, 0.5, 0.4),
                v(0.9, 1.4, 0.2),
            ),
            // Coplanar, well separated.
            (
                v(0.0, 0.0, 0.0),
                v(1.0, 0.0, 0.0),
                v(1.5, 0.5, 0.0),
                v(2.5, 1.7, 0.0),
            ),
            // Common perpendicular foot inside both filaments.
            (
                v(-1.0, 0.0, 0.0),
                v(1.0, 0.0, 0.0),
                v(-0.5, -0.8, 0.3),
                v(0.4, 0.9, 0.3),
            ),
            // Nearly antiparallel.
            (
                v(0.0, 0.0, 0.0),
                v(1.0, 0.0, 0.0),
                v(1.2, 0.4, 0.1),
                v(0.1, 0.45, 0.12),
            ),
        ];
        for (a0, a1, b0, b1) in cases {
            let got = skew(a0, a1, b0, b1, 1e-6).unwrap();
            let want = brute(a0, a1, b0, b1, 16);
            assert!(
                ((got - want) / want).abs() < 1e-9,
                "{a0:?} {a1:?} {b0:?} {b1:?}: {got} vs {want}"
            );
        }
    }

    #[test]
    fn skew_formula_stays_accurate_for_nearly_parallel_close_filaments() {
        // The feet of the common perpendicular are ~d/ε away.
        for angle in [1e-2_f64, 1e-3, 1e-4, 1e-5, 2e-6] {
            for sign in [1.0, -1.0] {
                let (s, c) = angle.sin_cos();
                let (a0, a1) = (v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0));
                let centre = v(0.45, 0.05, 0.002);
                let half = v(0.4 * c, 0.4 * s, 0.0) * sign;
                let (b0, b1) = (centre - half, centre + half);
                let got = skew(a0, a1, b0, b1, 1e-6).unwrap();
                let want = brute(a0, a1, b0, b1, 16);
                // Locating the far-away feet costs ~ε_machine/sin²ε.
                assert!(
                    ((got - want) / want).abs() < 1e-8 + 2e-17 / (angle * angle),
                    "angle {angle}, sign {sign}: {got} vs {want}"
                );
            }
        }
    }

    #[test]
    fn filaments_meeting_at_a_point_match_grovers_formula() {
        // Grover's two filaments from a common point at angle ε:
        // 2·[l·atanh(m/(l + R)) + m·atanh(l/(m + R))].
        for angle in [0.3_f64, 0.8, 1.5, 2.6] {
            let (l, m) = (1.0_f64, 0.7_f64);
            let b1 = v(m * angle.cos(), m * angle.sin(), 0.0);
            let big_r = b1.metric_distance(&v(l, 0.0, 0.0));
            let want = 2.0 * (l * (m / (l + big_r)).atanh() + m * (l / (m + big_r)).atanh());
            let got = skew(v(0.0, 0.0, 0.0), v(l, 0.0, 0.0), v(0.0, 0.0, 0.0), b1, 1e-6).unwrap();
            assert!((got - want).abs() < 1e-13, "ε = {angle}: {got} vs {want}");
        }
    }

    #[test]
    fn crossing_filaments_are_finite_and_parallel_ones_are_rejected() {
        let got = skew(
            v(-1.0, 0.0, 0.0),
            v(1.0, 0.0, 0.0),
            v(-0.5, -0.5, 0.0),
            v(0.5, 0.5, 0.0),
            1e-6,
        )
        .unwrap();
        assert!(got.is_finite() && got > 0.0);
        assert!(skew(
            v(0.0, 0.0, 0.0),
            v(1.0, 0.0, 0.0),
            v(0.0, 1.0, 0.0),
            v(1.0, 1.0, 0.0),
            1e-6
        )
        .is_none());
    }
}
