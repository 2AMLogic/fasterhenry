//! Numerical evaluation of the Neumann integral for two bars in arbitrary
//! relative position and orientation.
//!
//! Two schemes, chosen by how close the bars are:
//!
//! * **Point quadrature** ([`point_orders`], [`point_sum`]). Each bar is
//!   replaced by a tensor Gauss–Legendre point cloud over its length, width
//!   and height, and `Σ wᵢ wⱼ / |rᵢ − rⱼ|` is accumulated — optionally four
//!   inner points at a time in `wide::f64x4` lanes. The order along each of
//!   the six dimensions adapts to the geometry through
//!   [`gauss::OrderTable`]: it grows as the separation of the bars shrinks
//!   relative to that dimension's extent. Used when the bars are separated
//!   by roughly half a length or more.
//! * **Sampled filaments** ([`sampled_filaments`]). For close bars the two
//!   length integrations are done exactly by the closed forms of
//!   [`super::lines`], and only the two cross-sections are sampled, again
//!   with adaptive order. If the bars touch or overlap the cross-section
//!   integrand is weakly singular and the fixed maximum order is used; the
//!   result is then flagged as not resolved.

use std::cell::RefCell;
use std::sync::OnceLock;

use nalgebra::Vector3;
use wide::f64x4;

use super::gauss;
use super::lines;
use crate::filament::Filament;

/// Target relative accuracy of the point quadrature.
const POINT_TOLERANCE: f64 = 1e-8;
/// Largest order per dimension for the point quadrature.
const POINT_MAX_ORDER: usize = 16;
/// Capacity of a point cloud.
const CLOUD_CAPACITY: usize = 256;
/// Target relative accuracy of the cross-section sampling.
const SAMPLE_TOLERANCE: f64 = 1e-6;
/// Largest order per cross-section dimension for sampled filaments.
const SAMPLE_MAX_ORDER: usize = 8;
/// Order tables for the two schemes, built on first use.
fn point_table() -> &'static gauss::OrderTable {
    static TABLE: OnceLock<gauss::OrderTable> = OnceLock::new();
    TABLE.get_or_init(|| gauss::OrderTable::new(POINT_TOLERANCE, POINT_MAX_ORDER))
}

fn sample_table() -> &'static gauss::OrderTable {
    static TABLE: OnceLock<gauss::OrderTable> = OnceLock::new();
    TABLE.get_or_init(|| gauss::OrderTable::new(SAMPLE_TOLERANCE, SAMPLE_MAX_ORDER))
}

/// Below this `|sin ε|` two filaments count as parallel.
pub(crate) const PARALLEL_SIN: f64 = 1e-6;

/// An oriented box: a filament's volume.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Bar {
    /// Centroid.
    pub(crate) centre: Vector3<f64>,
    /// Unit vectors along length, width, height.
    pub(crate) axes: [Vector3<f64>; 3],
    /// Half extents along `axes`.
    pub(crate) half: [f64; 3],
}

impl Bar {
    pub(crate) fn new(f: &Filament) -> Self {
        Self {
            centre: f.center(),
            axes: [f.direction(), f.width_dir(), f.height_dir()],
            half: [0.5 * f.length(), 0.5 * f.width(), 0.5 * f.height()],
        }
    }

    /// Radius of the sphere about the centre that contains the bar.
    fn circumradius(&self) -> f64 {
        (self.half[0] * self.half[0] + self.half[1] * self.half[1] + self.half[2] * self.half[2])
            .sqrt()
    }

    /// Half the extent of the bar's projection onto the unit vector `n`.
    fn projected_half_extent(&self, n: &Vector3<f64>) -> f64 {
        (0..3)
            .map(|k| self.half[k] * n.dot(&self.axes[k]).abs())
            .sum()
    }
}

/// A lower bound on the distance between two bars from the separating-axis
/// theorem: the largest gap between their projections onto the 15 candidate
/// axes (face normals and edge cross products). Non-positive exactly when the
/// bars touch or overlap.
///
/// Well-separated bars (centres more than eight combined circumradii apart)
/// skip the 15 projections for the bounding-sphere bound, which is then
/// within 12.5 % of the truth.
pub(crate) fn separation(a: &Bar, b: &Bar) -> f64 {
    let delta = b.centre - a.centre;
    let (distance, reach) = (delta.norm(), a.circumradius() + b.circumradius());
    if distance > 8.0 * reach {
        return distance - reach;
    }
    let mut best = f64::NEG_INFINITY;
    let mut consider = |n: Vector3<f64>| {
        let gap = n.dot(&delta).abs() - a.projected_half_extent(&n) - b.projected_half_extent(&n);
        best = best.max(gap);
    };
    for k in 0..3 {
        consider(a.axes[k]);
        consider(b.axes[k]);
    }
    for i in 0..3 {
        for j in 0..3 {
            let n = a.axes[i].cross(&b.axes[j]);
            let norm = n.norm();
            if norm > 1e-9 {
                consider(n / norm);
            }
        }
    }
    best
}

/// Gauss–Legendre orders along (length, width, height) of `bar` for the
/// point quadrature against something `gap` away, or `None` if the bars are
/// too close for it.
pub(crate) fn point_orders(bar: &Bar, gap: f64) -> Option<[usize; 3]> {
    let table = point_table();
    let mut orders = [1; 3];
    for (order, &half) in orders.iter_mut().zip(&bar.half) {
        *order = table.order_for(half, gap)?;
    }
    (orders.iter().product::<usize>() <= CLOUD_CAPACITY).then_some(orders)
}

/// A tensor Gauss–Legendre point cloud filling a bar. Weights sum to the
/// bar's length: the cross-section weights are normalised to one, which is
/// the `1/A` of the current density.
pub(crate) struct Cloud {
    x: [f64; CLOUD_CAPACITY],
    y: [f64; CLOUD_CAPACITY],
    z: [f64; CLOUD_CAPACITY],
    w: [f64; CLOUD_CAPACITY],
    len: usize,
}

impl Cloud {
    const fn empty() -> Self {
        Self {
            x: [0.0; CLOUD_CAPACITY],
            y: [0.0; CLOUD_CAPACITY],
            z: [0.0; CLOUD_CAPACITY],
            w: [0.0; CLOUD_CAPACITY],
            len: 0,
        }
    }

    /// Overwrites the cloud with the points of `bar`; `orders` must come from
    /// [`point_orders`], which bounds their product by the capacity.
    fn fill(&mut self, bar: &Bar, orders: [usize; 3]) {
        let [rl, rw, rh] = orders.map(gauss::rule);
        let mut i = 0;
        for (xl, wl) in rl.on(-bar.half[0], bar.half[0]) {
            for (xw, ww) in rw.on(-0.5, 0.5) {
                for (xh, wh) in rh.on(-0.5, 0.5) {
                    let p = bar.centre
                        + bar.axes[0] * xl
                        + bar.axes[1] * (2.0 * bar.half[1] * xw)
                        + bar.axes[2] * (2.0 * bar.half[2] * xh);
                    (self.x[i], self.y[i], self.z[i]) = (p.x, p.y, p.z);
                    self.w[i] = wl * ww * wh;
                    i += 1;
                }
            }
        }
        self.len = i;
        // Pad the last SIMD lane with zero-weight points far away, so whole
        // lanes can be processed without a remainder loop; 1e150² does not
        // overflow.
        for pad in i..i.next_multiple_of(4) {
            (self.x[pad], self.y[pad], self.z[pad]) = (1e150, 1e150, 1e150);
            self.w[pad] = 0.0;
        }
    }
}

thread_local! {
    /// Per-thread scratch clouds: refilled for every pair, never reallocated.
    static SCRATCH: RefCell<Box<(Cloud, Cloud)>> =
        RefCell::new(Box::new((Cloud::empty(), Cloud::empty())));
}

/// `1/(A₁A₂) ∫∫ dV dV'/|r − r'|` by tensor Gauss–Legendre quadrature of the
/// given orders over both bars; `simd` evaluates the inner quadrature in
/// four-wide lanes.
pub(crate) fn point_sum(a: &Bar, oa: [usize; 3], b: &Bar, ob: [usize; 3], simd: bool) -> f64 {
    SCRATCH.with_borrow_mut(|scratch| {
        let (ca, cb) = &mut **scratch;
        ca.fill(a, oa);
        cb.fill(b, ob);
        if simd {
            point_sum_simd(ca, cb)
        } else {
            point_sum_scalar(ca, cb)
        }
    })
}

/// `Σᵢ Σⱼ wᵢ wⱼ / |rᵢ − rⱼ|`, one pair at a time.
fn point_sum_scalar(a: &Cloud, b: &Cloud) -> f64 {
    let mut total = 0.0;
    for i in 0..a.len {
        let mut inner = 0.0;
        for j in 0..b.len {
            let (dx, dy, dz) = (a.x[i] - b.x[j], a.y[i] - b.y[j], a.z[i] - b.z[j]);
            inner += b.w[j] / (dx * dx + dy * dy + dz * dz).sqrt();
        }
        total += a.w[i] * inner;
    }
    total
}

/// `Σᵢ Σⱼ wᵢ wⱼ / |rᵢ − rⱼ|`, with the inner quadrature over `b` evaluated
/// four points at a time.
fn point_sum_simd(a: &Cloud, b: &Cloud) -> f64 {
    let lanes = b.len.div_ceil(4);
    let pack = |v: &[f64; CLOUD_CAPACITY], lane: usize| {
        f64x4::new([
            v[4 * lane],
            v[4 * lane + 1],
            v[4 * lane + 2],
            v[4 * lane + 3],
        ])
    };
    let mut total = 0.0;
    for i in 0..a.len {
        let (px, py, pz) = (
            f64x4::splat(a.x[i]),
            f64x4::splat(a.y[i]),
            f64x4::splat(a.z[i]),
        );
        let mut inner = f64x4::ZERO;
        for lane in 0..lanes {
            let (dx, dy, dz) = (
                px - pack(&b.x, lane),
                py - pack(&b.y, lane),
                pz - pack(&b.z, lane),
            );
            inner += pack(&b.w, lane) / (dx * dx + dy * dy + dz * dz).sqrt();
        }
        total += a.w[i] * inner.reduce_add();
    }
    total
}

/// Outcome of [`sampled_filaments`].
pub(crate) struct Sampled {
    /// `1/(A₁A₂) ∫∫ dV dV'/|r − r'|`, in metres.
    pub(crate) value: f64,
    /// Whether every cross-section order met its a-priori accuracy target.
    pub(crate) resolved: bool,
}

/// Cross-section-sampled line-to-line closed forms. `parallel` selects the
/// parallel-filament kernel (the direction of `b` is then taken as exactly
/// `±` that of `a`); otherwise the skew kernel is used. Returns `None` if a
/// pair of sample filaments is coaxial and overlapping (a divergent sample).
pub(crate) fn sampled_filaments(a: &Bar, b: &Bar, gap: f64, parallel: bool) -> Option<Sampled> {
    let table = sample_table();
    let mut resolved = true;
    let mut order = |half: f64, bump: usize| match table.order_for(half, gap) {
        Some(n) => n,
        None => {
            resolved = false;
            table.max_order() + bump
        }
    };
    // Unequal orders keep the two sample grids from coinciding when
    // overlapping parallel bars share an axis.
    let orders_a = [order(a.half[1], 0), order(a.half[2], 0)];
    let orders_b = [order(b.half[1], 1), order(b.half[2], 1)];

    let samples = |bar: &Bar, orders: [usize; 2]| {
        let mut out = Vec::with_capacity(orders[0] * orders[1]);
        for (xw, ww) in gauss::rule(orders[0]).on(-0.5, 0.5) {
            for (xh, wh) in gauss::rule(orders[1]).on(-0.5, 0.5) {
                let offset =
                    bar.axes[1] * (2.0 * bar.half[1] * xw) + bar.axes[2] * (2.0 * bar.half[2] * xh);
                out.push((bar.centre + offset, ww * wh));
            }
        }
        out
    };
    let (sa, sb) = (samples(a, orders_a), samples(b, orders_b));
    let (la, lb) = (2.0 * a.half[0], 2.0 * b.half[0]);
    let axis = a.axes[0];
    let mut value = 0.0;
    for &(pa, wa) in &sa {
        for &(pb, wb) in &sb {
            let kernel = if parallel {
                let delta = pb - pa;
                let along = delta.dot(&axis);
                let rho = (delta - axis * along).norm();
                lines::parallel(la, lb, along + 0.5 * (la - lb), rho)?
            } else {
                let (ha, hb) = (a.axes[0] * a.half[0], b.axes[0] * b.half[0]);
                lines::skew(pa - ha, pa + ha, pb - hb, pb + hb, 0.0)?
            };
            value += wa * wb * kernel;
        }
    }
    Some(Sampled { value, resolved })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Node, Segment};

    fn bar(a: [f64; 3], b: [f64; 3], width: f64, height: f64) -> Bar {
        let segment = Segment::new(Node::from(a), Node::from(b), width, height, 5.8e7);
        Bar::new(&Filament::new(&segment).unwrap())
    }

    #[test]
    fn separation_is_exact_for_face_to_face_bars_and_negative_on_overlap() {
        let a = bar([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], 0.2, 0.1);
        let b = bar([0.0, 0.0, 0.5], [1.0, 0.0, 0.5], 0.2, 0.1);
        assert!((separation(&a, &b) - 0.4).abs() < 1e-15);
        let crossing = bar([0.5, -0.5, 0.3], [0.5, 0.5, 0.3], 0.2, 0.1);
        assert!((separation(&a, &crossing) - 0.2).abs() < 1e-15);
        let overlapping = bar([0.5, -0.5, 0.05], [0.9, 0.5, 0.05], 0.2, 0.1);
        assert!(separation(&a, &overlapping) <= 0.0);
        // Never larger than the true distance: here 1, between the end faces.
        let beyond = bar([2.0, 0.0, 0.0], [3.0, 0.3, 0.0], 0.2, 0.1);
        let bound = separation(&a, &beyond);
        assert!(bound > 0.8 && bound <= 1.0, "{bound}");
    }

    #[test]
    fn simd_and_scalar_point_sums_agree() {
        let a = bar([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], 0.2, 0.1);
        let b = bar([0.3, 1.5, 0.4], [1.1, 2.0, 0.2], 0.1, 0.3);
        for orders in [[1, 1, 1], [3, 1, 1], [5, 2, 3], [7, 3, 3]] {
            let (mut ca, mut cb) = (Box::new(Cloud::empty()), Box::new(Cloud::empty()));
            ca.fill(&a, orders);
            cb.fill(&b, orders);
            let weight: f64 = ca.w[..ca.len].iter().sum();
            assert!((weight - 1.0).abs() < 1e-14, "weights sum to the length");
            let (scalar, simd) = (point_sum_scalar(&ca, &cb), point_sum_simd(&ca, &cb));
            assert!(
                ((scalar - simd) / scalar).abs() < 1e-14,
                "{orders:?}: {scalar} vs {simd}"
            );
            assert_eq!(point_sum(&a, orders, &b, orders, true), simd);
            assert_eq!(point_sum(&a, orders, &b, orders, false), scalar);
        }
    }

    #[test]
    fn point_orders_refuse_close_bars_and_relax_with_distance() {
        let a = bar([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], 0.1, 0.1);
        assert!(point_orders(&a, 0.01).is_none());
        let near = point_orders(&a, 1.0).unwrap();
        let far = point_orders(&a, 100.0).unwrap();
        assert!(near[0] > far[0], "{near:?} vs {far:?}");
        assert_eq!(point_orders(&a, 1e6).unwrap(), [1, 1, 1]);
    }
}
