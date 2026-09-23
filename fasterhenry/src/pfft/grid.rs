//! The uniform grid of the pFFT operator and the projection of filaments
//! onto it.
//!
//! # Interpolation stencil
//!
//! A point `r` is represented on the grid by tensor-product Lagrange
//! interpolation over the `m = order + 1` grid planes nearest to it along
//! each axis: for a smooth `f`,
//!
//! ```text
//! f(r) ≈ Σ_g ℓ_g(r) f(r_g),     ℓ_g(r) = ℓ_{g_x}(x) ℓ_{g_y}(y) ℓ_{g_z}(z)
//! ```
//!
//! over the `m³` stencil points `g`. The stencil is centred on `r` (the
//! first plane is `⌊u − m/2⌋ + 1` for `u` the coordinate in grid units), so
//! `r` always lies within the central cell and the interpolation error is
//! `O((h/d)^m)` for a function whose nearest singularity is `d` away.
//!
//! # Projection
//!
//! The volume integral of a smooth function over a filament, divided by the
//! filament's cross-section area, is approximated by a tensor
//! Gauss–Legendre rule over the filament's length, width and height (each
//! split into pieces no longer than one grid spacing), and every quadrature
//! node is then interpolated onto its stencil:
//!
//! ```text
//! (1/A) ∫_V f dV ≈ Σ_q w_q f(r_q) ≈ Σ_g W_g f(r_g),    W_g = Σ_q w_q ℓ_g(r_q)
//! ```
//!
//! The weights `W_g` are the filament's projection. They sum to the
//! filament's length, and reproduce polynomials up to the interpolation
//! order exactly.

use nalgebra::Vector3;
use rayon::prelude::*;

use crate::filament::Filament;
use crate::inductance::gauss::{self, OrderTable};

/// Relative accuracy asked of the projection quadrature for an interaction
/// at the near-field radius. Well below the interpolation error, so the
/// quadrature never limits the operator's accuracy.
const QUADRATURE_TOLERANCE: f64 = 1e-10;
/// Largest Gauss–Legendre order per dimension of a projection piece.
const QUADRATURE_MAX_ORDER: usize = 12;

/// A uniform rectilinear grid.
#[derive(Clone, Debug)]
pub(crate) struct Grid {
    /// Position of grid point `(0, 0, 0)`.
    pub(crate) origin: Vector3<f64>,
    /// Spacing `h`, equal along all three axes.
    pub(crate) spacing: f64,
    /// Number of grid points along each axis.
    pub(crate) dims: [usize; 3],
    /// Interpolation points per axis, `order + 1`.
    pub(crate) stencil: usize,
}

impl Grid {
    /// A grid of spacing `spacing` that holds the full interpolation stencil
    /// of every point inside the box `[lo, hi]`.
    pub(crate) fn new(lo: Vector3<f64>, hi: Vector3<f64>, spacing: f64, stencil: usize) -> Self {
        let pad = stencil.div_ceil(2) as f64;
        let origin = lo - Vector3::repeat(pad * spacing);
        let mut dims = [0; 3];
        for (axis, dim) in dims.iter_mut().enumerate() {
            let span = (hi[axis] - origin[axis]) / spacing;
            // The last stencil plane of a point at `span` is at most
            // `⌊span + m/2⌋`; one more plane absorbs rounding.
            *dim = (span + 0.5 * stencil as f64).floor() as usize + 2;
        }
        Self {
            origin,
            spacing,
            dims,
            stencil,
        }
    }

    /// Total number of grid points.
    pub(crate) fn len(&self) -> usize {
        self.dims.iter().product()
    }

    /// Linear index of grid point `(i, j, k)`; `k` varies fastest.
    pub(crate) fn index(&self, point: [usize; 3]) -> usize {
        (point[0] * self.dims[1] + point[1]) * self.dims[2] + point[2]
    }

    /// Grid point of a linear index.
    pub(crate) fn point(&self, index: usize) -> [usize; 3] {
        let k = index % self.dims[2];
        let rest = index / self.dims[2];
        [rest / self.dims[1], rest % self.dims[1], k]
    }

    /// First stencil plane along `axis` for coordinate `x`, and the
    /// Lagrange weights of the `stencil` planes from there on, written to
    /// `weights`.
    fn axis_stencil(&self, axis: usize, x: f64, weights: &mut [f64]) -> usize {
        let m = self.stencil;
        let u = (x - self.origin[axis]) / self.spacing;
        let first = ((u - 0.5 * m as f64).floor() + 1.0).max(0.0) as usize;
        // Rounding can only push a point on the boundary one plane out; an
        // off-centre stencil is still an exact interpolant, just a slightly
        // less accurate one.
        let first = first.min(self.dims[axis] - m);
        let t = u - first as f64;
        for (k, weight) in weights.iter_mut().enumerate().take(m) {
            let mut value = 1.0;
            for l in 0..m {
                if l != k {
                    value *= (t - l as f64) / (k as f64 - l as f64);
                }
            }
            *weight = value;
        }
        first
    }
}

/// A filament's projection onto the grid: `(grid index, weight)` pairs,
/// sorted by index and without duplicates, and the inclusive bounding box
/// of the indices in grid coordinates.
#[derive(Clone, Debug, Default)]
pub(crate) struct Projection {
    pub(crate) entries: Vec<(u32, f64)>,
    pub(crate) lo: [usize; 3],
    pub(crate) hi: [usize; 3],
}

/// Projects every filament onto `grid`. `far_distance` is the smallest
/// separation at which the projection is used for an interaction (the
/// near-field radius); it sets the quadrature order.
pub(crate) fn project_all(
    grid: &Grid,
    filaments: &[Filament],
    far_distance: f64,
) -> Vec<Projection> {
    let table = OrderTable::new(QUADRATURE_TOLERANCE, QUADRATURE_MAX_ORDER);
    filaments
        .par_iter()
        .map(|f| project(grid, f, far_distance, &table))
        .collect()
}

fn project(grid: &Grid, f: &Filament, far_distance: f64, table: &OrderTable) -> Projection {
    let axes = [f.direction(), f.width_dir(), f.height_dir()];
    let extents = [f.length(), f.width(), f.height()];
    let corner = f.start() - 0.5 * f.width() * axes[1] - 0.5 * f.height() * axes[2];
    // Per dimension: the (offset, weight) nodes of a composite
    // Gauss–Legendre rule over [0, extent], pieces no longer than h.
    let rules: Vec<Vec<(f64, f64)>> = extents
        .iter()
        .map(|&extent| {
            let pieces = (extent / grid.spacing).ceil().max(1.0) as usize;
            let piece = extent / pieces as f64;
            let order = table
                .order_for(0.5 * piece, far_distance)
                .unwrap_or(QUADRATURE_MAX_ORDER);
            let rule = gauss::rule(order);
            (0..pieces)
                .flat_map(|p| rule.on(p as f64 * piece, (p + 1) as f64 * piece))
                .collect()
        })
        .collect();
    // Cross-section weights sum to the area; dividing by it gives the 1/A of
    // a uniform current density, so the projection weights sum to the length.
    let inverse_area = 1.0 / f.area();

    let m = grid.stencil;
    let mut raw = Vec::with_capacity(rules.iter().map(Vec::len).product::<usize>() * m * m * m);
    let mut wx = vec![0.0; m];
    let mut wy = vec![0.0; m];
    let mut wz = vec![0.0; m];
    for &(s, ws) in &rules[0] {
        for &(a, wa) in &rules[1] {
            for &(b, wb) in &rules[2] {
                let r = corner + s * axes[0] + a * axes[1] + b * axes[2];
                let weight = ws * wa * wb * inverse_area;
                let fx = grid.axis_stencil(0, r.x, &mut wx);
                let fy = grid.axis_stencil(1, r.y, &mut wy);
                let fz = grid.axis_stencil(2, r.z, &mut wz);
                for (i, &lx) in wx.iter().enumerate() {
                    for (j, &ly) in wy.iter().enumerate() {
                        let row = grid.index([fx + i, fy + j, fz]);
                        let lxy = weight * lx * ly;
                        for (k, &lz) in wz.iter().enumerate() {
                            raw.push((row + k, lxy * lz));
                        }
                    }
                }
            }
        }
    }
    raw.sort_unstable_by_key(|&(index, _)| index);
    let mut entries: Vec<(u32, f64)> = Vec::new();
    for (index, weight) in raw {
        match entries.last_mut() {
            Some(last) if last.0 as usize == index => last.1 += weight,
            _ => entries.push((index as u32, weight)),
        }
    }
    let mut lo = [usize::MAX; 3];
    let mut hi = [0; 3];
    for &(index, _) in &entries {
        let point = grid.point(index as usize);
        for axis in 0..3 {
            lo[axis] = lo[axis].min(point[axis]);
            hi[axis] = hi[axis].max(point[axis]);
        }
    }
    Projection { entries, lo, hi }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Node, Segment};

    fn grid(order: usize) -> Grid {
        Grid::new(
            Vector3::new(-1.0, -0.5, 0.0),
            Vector3::new(2.0, 1.0, 0.3),
            0.25,
            order + 1,
        )
    }

    #[test]
    fn stencils_reproduce_polynomials_up_to_the_order() {
        for order in 1..=6 {
            let g = grid(order);
            let m = order + 1;
            let mut weights = vec![0.0; m];
            for &x in &[-1.0, -0.37, 0.0, 0.1234, 0.99, 2.0] {
                let first = g.axis_stencil(0, x, &mut weights);
                assert!(first + m <= g.dims[0]);
                for degree in 0..=order as i32 {
                    let sum: f64 = weights
                        .iter()
                        .enumerate()
                        .map(|(k, w)| {
                            w * (g.origin.x + (first + k) as f64 * g.spacing).powi(degree)
                        })
                        .sum();
                    assert!((sum - x.powi(degree)).abs() < 1e-11, "{order} {x} {degree}");
                }
                // Centred: the point lies within the central cell(s).
                let u = (x - g.origin.x) / g.spacing - first as f64;
                assert!(u >= 0.5 * m as f64 - 1.0 - 1e-9 && u <= 0.5 * m as f64 + 1e-9);
            }
        }
    }

    #[test]
    fn projection_weights_sum_to_the_length_and_reproduce_linear_functions() {
        let g = grid(3);
        let segment = Segment::new(
            Node::new(-0.8, -0.3, 0.1),
            Node::new(1.7, 0.8, 0.2),
            0.05,
            0.02,
            1.0,
        );
        let f = Filament::new(&segment).unwrap();
        let p = project(&g, &f, 1.0, &OrderTable::new(1e-10, 12));
        let total: f64 = p.entries.iter().map(|&(_, w)| w).sum();
        assert!((total - f.length()).abs() < 1e-12 * f.length());
        // ∫ x dV / A over the bar = length · centroid.x.
        let first_moment: f64 = p
            .entries
            .iter()
            .map(|&(i, w)| {
                let point = g.point(i as usize);
                w * (g.origin.x + point[0] as f64 * g.spacing)
            })
            .sum();
        assert!((first_moment - f.length() * f.center().x).abs() < 1e-12);
        for axis in 0..3 {
            assert!(p.lo[axis] <= p.hi[axis] && p.hi[axis] < g.dims[axis]);
        }
    }

    #[test]
    fn index_and_point_are_inverse() {
        let g = grid(2);
        for index in [0, 1, 17, g.len() / 3, g.len() - 1] {
            assert_eq!(g.index(g.point(index)), index);
        }
    }
}
