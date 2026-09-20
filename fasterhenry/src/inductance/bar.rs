//! Exact closed form for the Neumann volume integral between two parallel
//! rectangular bars whose cross-section axes are aligned.
//!
//! For uniform current density the partial inductance of two such bars is
//!
//! ```text
//! M = μ0/(4π) · 1/(A₁A₂) · ∫_{V₁} ∫_{V₂} dV dV' / |r − r'|
//! ```
//!
//! Because the integrand depends only on the coordinate differences, the
//! sixfold integral is the second difference, along each axis, of a function
//! `F(x, y, z)` with `∂⁶F / ∂x²∂y²∂z² = 1/r`. That primitive was published by
//! Hoer & Love, "Exact inductance equations for rectangular conductors with
//! applications to more complicated geometries", *J. Res. NBS* 69C (1965),
//! eq. (14), and restated by Ruehli, "Inductance calculations in a complex
//! integrated circuit environment", *IBM J. Res. Dev.* 16 (1972); it is the
//! exact counterpart of the Grover/Rosa geometric-mean-distance formulas. The
//! expression used here was checked symbolically to satisfy the sixth-order
//! differential identity above before being committed.
//!
//! The result is an alternating sum of 64 terms of magnitude `~D⁵` (`D` the
//! largest corner-to-corner distance) whose sum is `~A₁A₂·l²/D`, so it loses
//! `log10(D⁴ / (A₁A₂))`-ish digits to cancellation: badly for long thin bars,
//! and totally for distant ones. [`BarIntegral::relative_error`] estimates
//! that loss so the caller can fall back to another method.

/// Second-difference corner offsets `b − a` of two intervals with extents
/// `e1`, `e2` whose centres are `offset` apart, in the order matching
/// [`SIGNS`].
pub(crate) fn corner_differences(e1: f64, e2: f64, offset: f64) -> [f64; 4] {
    let (sum, diff) = (0.5 * (e1 + e2), 0.5 * (e1 - e2));
    [offset + sum, offset + diff, offset - diff, offset - sum]
}

/// Signs of the four terms of a second difference; see
/// [`corner_differences`].
pub(crate) const SIGNS: [f64; 4] = [1.0, -1.0, -1.0, 1.0];

/// `coefficient · x · ln(x + r)`, evaluated without cancellation for `x < 0`
/// and with the removable singularity at `y = z = 0` removed.
fn log_term(coefficient: f64, x: f64, r: f64, rest_sq: f64) -> f64 {
    if coefficient == 0.0 || x == 0.0 {
        return 0.0;
    }
    // x + r = (r² − x²)/(r − x) = (y² + z²)/(r − x) when x is negative.
    let argument = if x >= 0.0 { x + r } else { rest_sq / (r - x) };
    coefficient * x * argument.ln()
}

/// `−(x y z³ / 6) · atan(x y / (z r))`, zero when any factor vanishes.
fn atan_term(x: f64, y: f64, z: f64, r: f64) -> f64 {
    let product = x * y * z;
    if product == 0.0 {
        return 0.0;
    }
    -(product * z * z / 6.0) * (x * y / (z * r)).atan()
}

/// The Hoer–Love primitive `F`, with `∂⁶F/∂x²∂y²∂z² = 1/√(x² + y² + z²)`.
fn primitive(x: f64, y: f64, z: f64) -> f64 {
    let (x2, y2, z2) = (x * x, y * y, z * z);
    let (x4, y4, z4) = (x2 * x2, y2 * y2, z2 * z2);
    let r = (x2 + y2 + z2).sqrt();
    log_term(y2 * z2 / 4.0 - y4 / 24.0 - z4 / 24.0, x, r, y2 + z2)
        + log_term(x2 * z2 / 4.0 - x4 / 24.0 - z4 / 24.0, y, r, x2 + z2)
        + log_term(x2 * y2 / 4.0 - x4 / 24.0 - y4 / 24.0, z, r, x2 + y2)
        + (x4 + y4 + z4 - 3.0 * (x2 * y2 + y2 * z2 + z2 * x2)) * r / 60.0
        + atan_term(x, y, z, r)
        + atan_term(x, z, y, r)
        + atan_term(y, z, x, r)
}

/// Value of `∫∫ dV dV'/|r − r'|` and an estimate of its rounding error.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BarIntegral {
    /// The sixfold integral, in m⁵.
    pub(crate) value: f64,
    /// Estimated relative rounding error of `value`.
    pub(crate) relative_error: f64,
}

/// Evaluates the 64-term closed form. Each argument holds the four corner
/// differences along one axis, from [`corner_differences`].
pub(crate) fn bar_integral(qx: &[f64; 4], qy: &[f64; 4], qz: &[f64; 4]) -> BarIntegral {
    let mut value = 0.0;
    let mut magnitude = 0.0;
    for (i, &x) in qx.iter().enumerate() {
        for (j, &y) in qy.iter().enumerate() {
            for (k, &z) in qz.iter().enumerate() {
                let term = SIGNS[i] * SIGNS[j] * SIGNS[k] * primitive(x, y, z);
                value += term;
                magnitude += term.abs();
            }
        }
    }
    // Every term carries a few ulps of error from ln/atan and the products.
    let relative_error = if value > 0.0 {
        8.0 * f64::EPSILON * magnitude / value
    } else {
        f64::INFINITY
    };
    BarIntegral {
        value,
        relative_error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_is_finite_on_the_coordinate_planes_and_axes() {
        for &(x, y, z) in &[
            (0.0, 0.0, 0.0),
            (1.0, 0.0, 0.0),
            (-1.0, 0.0, 0.0),
            (0.0, -2.0, 0.0),
            (0.0, 0.0, -3.0),
            (1.0, -2.0, 0.0),
            (-1.0, 0.0, 3.0),
            (0.0, 2.0, -3.0),
        ] {
            assert!(primitive(x, y, z).is_finite(), "F({x}, {y}, {z})");
        }
    }

    #[test]
    fn negative_argument_branch_matches_the_direct_logarithm() {
        // Where x + r is well conditioned both forms must agree.
        let (x, y, z) = (-0.3_f64, 0.7_f64, 1.1_f64);
        let r = (x * x + y * y + z * z).sqrt();
        let direct = 2.5 * x * (x + r).ln();
        assert!((log_term(2.5, x, r, y * y + z * z) - direct).abs() < 1e-15);
    }

    /// The Newtonian self-potential of the unit cube, `∫∫ dV dV'/r`, has the
    /// classical closed form below (≈ 1.8823126443896602, i.e. twice the
    /// well-known self-energy constant 0.9411563…). The expression was
    /// confirmed for this test to 17 digits by an independent tanh–sinh
    /// integration of `8 ∫∫∫ (1−u)(1−v)(1−w)/√(u²+v²+w²)` over the unit cube.
    #[test]
    fn unit_cube_self_potential() {
        let q = corner_differences(1.0, 1.0, 0.0);
        let got = bar_integral(&q, &q, &q);
        let (s2, s3) = (2.0_f64.sqrt(), 3.0_f64.sqrt());
        let want = 2.0
            * ((1.0 + s2 - 2.0 * s3) / 5.0 - std::f64::consts::PI / 3.0
                + (1.0 + s2).ln()
                + (2.0 + s3).ln());
        assert!((got.value - want).abs() < 1e-12, "{} vs {want}", got.value);
        assert!(got.relative_error < 1e-12);
    }

    #[test]
    fn error_estimate_flags_long_thin_and_distant_bars() {
        let thin = bar_integral(
            &corner_differences(1.0, 1.0, 0.0),
            &corner_differences(1e-4, 1e-4, 0.0),
            &corner_differences(1e-4, 1e-4, 0.0),
        );
        assert!(thin.relative_error > 1e-6, "{thin:?}");
        let distant = bar_integral(
            &corner_differences(1.0, 1.0, 0.0),
            &corner_differences(0.1, 0.1, 300.0),
            &corner_differences(0.1, 0.1, 0.0),
        );
        assert!(distant.relative_error > 1e-6, "{distant:?}");
    }
}
