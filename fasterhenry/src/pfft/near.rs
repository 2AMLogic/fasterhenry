//! Near-field pairs and their precorrection.
//!
//! A pair of filaments is *near* when a lower bound on the distance between
//! their volumes (the separating-axis gap of
//! [`neumann::separation`](crate::inductance::neumann::separation)) is below
//! the near-field radius. For those pairs the grid's approximation of the
//! partial inductance is inaccurate, so the operator stores
//!
//! ```text
//! Cᵢⱼ = L̃ᵢⱼ(exact kernel) − Lᵢⱼ(grid)
//! ```
//!
//! and adds `Σⱼ Cᵢⱼ xⱼ` to the far-field result: the grid contribution of a
//! near pair cancels to rounding error and the exact value takes its place.

use nalgebra::Vector3;
use rayon::prelude::*;

use super::grid::{Grid, Projection};
use crate::filament::Filament;
use crate::inductance::neumann::{separation, Bar};
use crate::inductance::{evaluate, KernelError, MU0_OVER_4PI};

/// `|cos|` below which two filaments are orthogonal, so that both their
/// exact partial inductance and its grid approximation vanish. Matches the
/// kernel's own threshold.
const ORTHOGONAL_COS: f64 = 1e-14;

/// Axis-aligned bounding box of a filament's volume.
fn bounds(bar: &Bar) -> (Vector3<f64>, Vector3<f64>) {
    let mut reach = Vector3::zeros();
    for k in 0..3 {
        reach += bar.axes[k].abs() * bar.half[k];
    }
    (bar.centre - reach, bar.centre + reach)
}

/// For each filament `i`, the filaments `j ≥ i` (including `i` itself)
/// within `radius` of it, ascending.
pub(crate) fn near_pairs(filaments: &[Filament], radius: f64) -> Vec<Vec<u32>> {
    let n = filaments.len();
    if n == 0 {
        return Vec::new();
    }
    let bars: Vec<Bar> = filaments.iter().map(Bar::new).collect();
    let boxes: Vec<_> = bars.iter().map(bounds).collect();
    let mut lo = boxes[0].0;
    let mut hi = boxes[0].1;
    for (a, b) in &boxes {
        lo = lo.inf(a);
        hi = hi.sup(b);
    }
    // Uniform bins about the radius in size, coarsened if that would make
    // far more bins than filaments.
    let extent = hi - lo;
    let mut size = radius.max(extent.max() * 1e-9).max(f64::MIN_POSITIVE);
    let bin_dims = |size: f64| extent.map(|e| (e / size).floor() as usize + 1);
    while bin_dims(size).iter().product::<usize>() > 8 * n + 64 {
        size *= 1.5;
    }
    let dims = bin_dims(size);
    let cell = |x: f64, axis: usize| {
        (((x - lo[axis]) / size).floor().max(0.0) as usize).min(dims[axis] - 1)
    };
    let range = |a: &Vector3<f64>, b: &Vector3<f64>| {
        let first = [cell(a.x, 0), cell(a.y, 1), cell(a.z, 2)];
        let last = [cell(b.x, 0), cell(b.y, 1), cell(b.z, 2)];
        (first, last)
    };
    let mut bins: Vec<Vec<u32>> = vec![Vec::new(); dims.iter().product()];
    for (index, (a, b)) in boxes.iter().enumerate() {
        let (first, last) = range(a, b);
        for i in first[0]..=last[0] {
            for j in first[1]..=last[1] {
                for k in first[2]..=last[2] {
                    bins[(i * dims[1] + j) * dims[2] + k].push(index as u32);
                }
            }
        }
    }
    let grow = Vector3::repeat(radius);
    (0..n)
        .into_par_iter()
        .map(|index| {
            let (a, b) = &boxes[index];
            let (first, last) = range(&(a - grow), &(b + grow));
            let mut found = Vec::new();
            for i in first[0]..=last[0] {
                for j in first[1]..=last[1] {
                    for k in first[2]..=last[2] {
                        found.extend(
                            bins[(i * dims[1] + j) * dims[2] + k]
                                .iter()
                                .filter(|&&other| other as usize >= index),
                        );
                    }
                }
            }
            found.sort_unstable();
            found.dedup();
            found.retain(|&other| {
                let other = other as usize;
                other == index || separation(&bars[index], &bars[other]) < radius
            });
            found
        })
        .collect()
}

/// The grid kernel `1/|r_g − r_h|` for an integer offset, with the
/// singular self term set to zero (it only ever enters near pairs, whose
/// grid contribution the precorrection removes).
pub(crate) fn grid_kernel(offset: [i64; 3], spacing: f64) -> f64 {
    let r2 = (offset[0] * offset[0] + offset[1] * offset[1] + offset[2] * offset[2]) as f64;
    if r2 == 0.0 {
        0.0
    } else {
        1.0 / (spacing * r2.sqrt())
    }
}

/// Near-field correction matrix in compressed sparse rows, both triangles.
#[derive(Clone, Debug, Default)]
pub(crate) struct NearField {
    pub(crate) offsets: Vec<usize>,
    pub(crate) columns: Vec<u32>,
    pub(crate) values: Vec<f64>,
}

/// [`grid_kernel`] tabulated over the integer offsets `|dₐ| ≤ reachₐ`,
/// `z` fastest, so that a run of offsets along `z` is a contiguous slice.
struct KernelTable {
    reach: [usize; 3],
    dims: [usize; 3],
    values: Vec<f64>,
}

impl KernelTable {
    fn new(reach: [usize; 3], spacing: f64) -> Self {
        let dims = reach.map(|r| 2 * r + 1);
        let mut values = vec![0.0; dims.iter().product()];
        values
            .par_chunks_mut(dims[1] * dims[2])
            .enumerate()
            .for_each(|(x, plane)| {
                let dx = x as i64 - reach[0] as i64;
                for (rest, value) in plane.iter_mut().enumerate() {
                    let dy = (rest / dims[2]) as i64 - reach[1] as i64;
                    let dz = (rest % dims[2]) as i64 - reach[2] as i64;
                    *value = grid_kernel([dx, dy, dz], spacing);
                }
            });
        Self {
            reach,
            dims,
            values,
        }
    }

    /// Index of offset `d`, which must be within reach.
    fn index(&self, d: [i64; 3]) -> usize {
        let at = |a: usize| (d[a] + self.reach[a] as i64) as usize;
        (at(0) * self.dims[1] + at(1)) * self.dims[2] + at(2)
    }
}

/// Grid coordinates of a projection's entries, with their weights.
fn points(grid: &Grid, projection: &Projection) -> Vec<([i64; 3], f64)> {
    projection
        .entries
        .iter()
        .map(|&(g, w)| (grid.point(g as usize).map(|c| c as i64), w))
        .collect()
}

/// Inclusive grid box spanned by the projections of `partners`.
fn partner_box(partners: &[usize], projections: &[Projection]) -> ([usize; 3], [usize; 3]) {
    let mut lo = [usize::MAX; 3];
    let mut hi = [0usize; 3];
    for &j in partners {
        for axis in 0..3 {
            lo[axis] = lo[axis].min(projections[j].lo[axis]);
            hi[axis] = hi[axis].max(projections[j].hi[axis]);
        }
    }
    (lo, hi)
}

/// Computes the precorrected near-field matrix.
pub(crate) fn precorrect(
    filaments: &[Filament],
    grid: &Grid,
    projections: &[Projection],
    near: &[Vec<u32>],
) -> Result<NearField, KernelError> {
    let n = filaments.len();
    // Partners j ≥ i that are not orthogonal to i: both their exact partial
    // inductance and its grid approximation vanish otherwise.
    let partners: Vec<Vec<usize>> = (0..n)
        .into_par_iter()
        .map(|i| {
            let direction = filaments[i].direction();
            near[i]
                .iter()
                .map(|&j| j as usize)
                .filter(|&j| direction.dot(&filaments[j].direction()).abs() >= ORTHOGONAL_COS)
                .collect()
        })
        .collect();
    // Every grid offset the precorrection evaluates lies between a point of
    // a filament's projection and a point of its partners' box.
    let reach = (0..n)
        .into_par_iter()
        .filter(|&i| !partners[i].is_empty())
        .map(|i| {
            let (lo, hi) = partner_box(&partners[i], projections);
            let own = &projections[i];
            [0, 1, 2].map(|a| (hi[a] - own.lo[a].min(hi[a])).max(own.hi[a] - lo[a].min(own.hi[a])))
        })
        .reduce(|| [0; 3], |a, b| [0, 1, 2].map(|k| a[k].max(b[k])));
    let table = KernelTable::new(reach, grid.spacing);

    let upper: Vec<Vec<(u32, f64)>> = (0..n)
        .into_par_iter()
        .map(|i| row(i, filaments, grid, projections, &partners[i], &table))
        .collect::<Result<_, _>>()?;
    // Mirror the upper triangle into full rows.
    let mut counts = vec![0usize; n];
    for (i, entries) in upper.iter().enumerate() {
        for &(j, _) in entries {
            counts[i] += 1;
            if j as usize != i {
                counts[j as usize] += 1;
            }
        }
    }
    let mut offsets = Vec::with_capacity(n + 1);
    offsets.push(0);
    for count in &counts {
        offsets.push(offsets.last().unwrap() + count);
    }
    let total = *offsets.last().unwrap();
    let mut columns = vec![0u32; total];
    let mut values = vec![0.0; total];
    let mut fill = offsets[..n].to_vec();
    for (i, entries) in upper.iter().enumerate() {
        for &(j, value) in entries {
            let j = j as usize;
            columns[fill[i]] = j as u32;
            values[fill[i]] = value;
            fill[i] += 1;
            if j != i {
                columns[fill[j]] = i as u32;
                values[fill[j]] = value;
                fill[j] += 1;
            }
        }
    }
    Ok(NearField {
        offsets,
        columns,
        values,
    })
}

/// Correction entries `(j, Cᵢⱼ)` of row `i` for its near partners `j ≥ i`.
fn row(
    i: usize,
    filaments: &[Filament],
    grid: &Grid,
    projections: &[Projection],
    partners: &[usize],
    table: &KernelTable,
) -> Result<Vec<(u32, f64)>, KernelError> {
    if partners.is_empty() {
        return Ok(Vec::new());
    }
    let fi = &filaments[i];
    let grid_values = grid_interactions(i, partners, grid, projections, table);
    partners
        .iter()
        .zip(grid_values)
        .map(|(&j, approx)| {
            let fj = &filaments[j];
            let exact = evaluate(fi, fj, true).map_err(|e| e.at(i, j))?.value;
            let cos = fi.direction().dot(&fj.direction());
            Ok((j as u32, exact - MU0_OVER_4PI * cos * approx))
        })
        .collect()
}

/// `Σ_g Σ_h W_ig W_jh K(g − h)` for every partner `j`, by whichever of two
/// schemes is cheaper: the direct double sum, or the potential
/// `ψ = K ⊛ Wᵢ` of `i`'s projection over the box spanned by the partners'
/// projections followed by one inner product per partner.
fn grid_interactions(
    i: usize,
    partners: &[usize],
    grid: &Grid,
    projections: &[Projection],
    table: &KernelTable,
) -> Vec<f64> {
    let source = points(grid, &projections[i]);
    let (lo, hi) = partner_box(partners, projections);
    let box_dims = [0, 1, 2].map(|a| hi[a] - lo[a] + 1);
    let box_len: usize = box_dims.iter().product();
    let partner_entries: usize = partners.iter().map(|&j| projections[j].entries.len()).sum();
    let direct_cost = source.len() * partner_entries;
    let box_cost = source.len() * box_len + partner_entries;

    if direct_cost <= box_cost {
        return partners
            .iter()
            .map(|&j| {
                points(grid, &projections[j])
                    .iter()
                    .map(|&(h, wh)| {
                        let potential: f64 = source
                            .iter()
                            .map(|&(g, wg)| {
                                wg * table.values
                                    [table.index([h[0] - g[0], h[1] - g[1], h[2] - g[2]])]
                            })
                            .sum();
                        wh * potential
                    })
                    .sum()
            })
            .collect();
    }

    // ψ over the box, one source point at a time: each (x, y) row of the box
    // takes a contiguous run of the table, a vectorizable multiply-add.
    let [bx, by, bz] = box_dims;
    let mut psi = vec![0.0; box_len];
    for &(g, wg) in &source {
        for x in 0..bx {
            for y in 0..by {
                let start = table.index([
                    (lo[0] + x) as i64 - g[0],
                    (lo[1] + y) as i64 - g[1],
                    lo[2] as i64 - g[2],
                ]);
                let kernel = &table.values[start..start + bz];
                let row = &mut psi[(x * by + y) * bz..(x * by + y + 1) * bz];
                for (value, &k) in row.iter_mut().zip(kernel) {
                    *value += wg * k;
                }
            }
        }
    }
    partners
        .iter()
        .map(|&j| {
            projections[j]
                .entries
                .iter()
                .map(|&(h, wh)| {
                    let p = grid.point(h as usize);
                    let local = ((p[0] - lo[0]) * by + (p[1] - lo[1])) * bz + (p[2] - lo[2]);
                    wh * psi[local]
                })
                .sum()
        })
        .collect()
}
