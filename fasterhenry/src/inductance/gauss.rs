//! Gauss–Legendre quadrature rules on `[-1, 1]`.
//!
//! The nodes of the `n`-point rule are the roots of the Legendre polynomial
//! `P_n`; they are found here by Newton iteration on the three-term
//! recurrence, starting from the classical Chebyshev-like initial guess, and
//! the weights follow from `w = 2 / ((1 − x²) P_n'(x)²)` (Abramowitz & Stegun
//! 25.4.29). Rules are computed once and cached for every order up to
//! [`MAX_ORDER`].

use std::sync::OnceLock;

/// Largest cached rule.
pub(crate) const MAX_ORDER: usize = 64;

/// An `n`-point Gauss–Legendre rule on `[-1, 1]`.
#[derive(Debug)]
pub(crate) struct Rule {
    /// Nodes, ascending.
    pub(crate) nodes: Vec<f64>,
    /// Weights, summing to 2.
    pub(crate) weights: Vec<f64>,
}

impl Rule {
    fn new(n: usize) -> Self {
        let mut nodes = vec![0.0; n];
        let mut weights = vec![0.0; n];
        let nf = n as f64;
        for k in 0..n.div_ceil(2) {
            // Root k counted from the top of the interval.
            let mut x = (std::f64::consts::PI * (k as f64 + 0.75) / (nf + 0.5)).cos();
            let mut derivative = 1.0;
            for _ in 0..100 {
                // P_0 .. P_n by (j+1) P_{j+1} = (2j+1) x P_j − j P_{j−1}.
                let (mut p_prev, mut p) = (1.0, x);
                for j in 1..n {
                    let jf = j as f64;
                    let p_next = ((2.0 * jf + 1.0) * x * p - jf * p_prev) / (jf + 1.0);
                    p_prev = p;
                    p = p_next;
                }
                derivative = nf * (x * p - p_prev) / (x * x - 1.0);
                let step = p / derivative;
                x -= step;
                if step.abs() <= 4.0 * f64::EPSILON * x.abs().max(1e-3) {
                    break;
                }
            }
            if n % 2 == 1 && k == n / 2 {
                x = 0.0;
                // Re-evaluate the derivative exactly at the centre node.
                let (mut p_prev, mut p) = (1.0, x);
                for j in 1..n {
                    let jf = j as f64;
                    let p_next = ((2.0 * jf + 1.0) * x * p - jf * p_prev) / (jf + 1.0);
                    p_prev = p;
                    p = p_next;
                }
                derivative = nf * (x * p - p_prev) / (x * x - 1.0);
            }
            let weight = 2.0 / ((1.0 - x * x) * derivative * derivative);
            nodes[n - 1 - k] = x;
            nodes[k] = -x;
            weights[n - 1 - k] = weight;
            weights[k] = weight;
        }
        Self { nodes, weights }
    }

    /// Nodes and weights mapped affinely onto `[a, b]`.
    pub(crate) fn on(&self, a: f64, b: f64) -> impl Iterator<Item = (f64, f64)> + '_ {
        let (mid, half) = (0.5 * (a + b), 0.5 * (b - a));
        self.nodes
            .iter()
            .zip(&self.weights)
            .map(move |(&x, &w)| (mid + half * x, half * w))
    }
}

/// The cached `n`-point rule; `n` is clamped to `1..=MAX_ORDER`.
pub(crate) fn rule(n: usize) -> &'static Rule {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    let rules = RULES.get_or_init(|| (1..=MAX_ORDER).map(Rule::new).collect());
    &rules[n.clamp(1, MAX_ORDER) - 1]
}

/// Number of Gauss–Legendre points needed to integrate, to relative accuracy
/// `tolerance`, a function over an interval of half-length `half_extent`
/// whose nearest singularity lies `distance` away from the interval.
///
/// A function analytic inside the Bernstein ellipse of parameter `ρ` is
/// integrated by the `n`-point rule with error `O(ρ^(−2n))` (Trefethen,
/// *Approximation Theory and Approximation Practice*, Thm 19.3). Of all
/// points at distance `δ` (in units of the half-length) from `[-1, 1]`, the
/// one lying on the smallest ellipse is `iδ`, for which
/// `ρ = δ + √(1 + δ²) = exp(asinh δ)`; that conservative value is used. So
/// `n` points suffice once `δ ≥ sinh(ln(1/tolerance) / 2n)`, and the rule is
/// a table of those thresholds.
#[derive(Debug)]
pub(crate) struct OrderTable {
    /// `thresholds[n − 1]` is the smallest `δ` for which `n` points suffice.
    thresholds: Vec<f64>,
}

impl OrderTable {
    /// Table for orders `1..=max_order`.
    pub(crate) fn new(tolerance: f64, max_order: usize) -> Self {
        let log = (1.0 / tolerance).ln();
        Self {
            thresholds: (1..=max_order)
                .map(|n| (log / (2.0 * n as f64)).sinh())
                .collect(),
        }
    }

    /// Largest order in the table.
    pub(crate) fn max_order(&self) -> usize {
        self.thresholds.len()
    }

    /// The order to use, or `None` when even the largest order of the table
    /// is not enough (the near-singular regime).
    pub(crate) fn order_for(&self, half_extent: f64, distance: f64) -> Option<usize> {
        if half_extent <= 0.0 {
            return Some(1);
        }
        if distance.is_nan() || distance <= 0.0 {
            return None;
        }
        let delta = distance / half_extent;
        // Thresholds decrease with n; take the first one that δ reaches.
        self.thresholds
            .iter()
            .position(|&needed| delta >= needed)
            .map(|index| index + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_sum_to_two_and_nodes_are_symmetric() {
        for n in 1..=MAX_ORDER {
            let r = rule(n);
            assert_eq!(r.nodes.len(), n);
            let sum: f64 = r.weights.iter().sum();
            assert!((sum - 2.0).abs() < 1e-13, "n = {n}: sum = {sum}");
            for k in 0..n {
                assert!((r.nodes[k] + r.nodes[n - 1 - k]).abs() < 1e-15);
                assert!(r.nodes[k] > -1.0 && r.nodes[k] < 1.0);
                if k > 0 {
                    assert!(r.nodes[k] > r.nodes[k - 1]);
                }
            }
        }
    }

    #[test]
    fn integrates_polynomials_of_degree_2n_minus_1_exactly() {
        for n in [1, 2, 3, 5, 8, 16, 33, 64] {
            let r = rule(n);
            let degree = (2 * n - 1).min(40);
            for p in 0..=degree {
                // ∫₀¹ xᵖ dx = 1 / (p + 1).
                let got: f64 = r.on(0.0, 1.0).map(|(x, w)| w * x.powi(p as i32)).sum();
                let want = 1.0 / (p as f64 + 1.0);
                assert!((got - want).abs() < 1e-13, "n = {n}, p = {p}: {got}");
            }
        }
    }

    #[test]
    fn order_estimate_grows_as_the_singularity_approaches() {
        let table = OrderTable::new(1e-9, 64);
        assert_eq!(table.max_order(), 64);
        assert_eq!(table.order_for(0.0, 1.0), Some(1));
        assert_eq!(table.order_for(1.0, 0.0), None);
        assert_eq!(table.order_for(1.0, f64::NAN), None);
        let far = table.order_for(1.0, 1e6).unwrap();
        let mid = table.order_for(1.0, 10.0).unwrap();
        let near = table.order_for(1.0, 1.0).unwrap();
        assert!(far <= mid && mid < near, "{far} {mid} {near}");
        assert_eq!(far, 1);
        assert_eq!(table.order_for(1.0, 1e-3), None);

        // The estimate is honest: 1/(a − x) on [-1, 1] with the pole at
        // distance δ from the interval reaches the requested accuracy.
        for delta in [0.5_f64, 1.0, 3.0, 30.0] {
            let n = table.order_for(1.0, delta).unwrap();
            let a = 1.0 + delta;
            let got: f64 = rule(n).on(-1.0, 1.0).map(|(x, w)| w / (a - x)).sum();
            let want = ((a + 1.0) / (a - 1.0)).ln();
            assert!(((got - want) / want).abs() < 1e-9, "δ = {delta}, n = {n}");
        }
    }
}
