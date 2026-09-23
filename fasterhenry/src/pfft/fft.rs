//! Aperiodic grid-to-grid convolution with a translation-invariant kernel,
//! by zero-padded 3-D FFT.
//!
//! On an `N₀ × N₁ × N₂` grid, `φ(g) = Σ_h K(g − h) q(h)` is a linear
//! convolution whose kernel offsets span `(−Nₐ, Nₐ)` along each axis. On a
//! periodic grid of `Mₐ ≥ 2Nₐ − 1` points the offsets no longer alias, so the
//! circular convolution computed by FFT — forward transform, pointwise
//! product with the kernel's precomputed spectrum, inverse transform —
//! equals the linear one on the original grid.
//!
//! The kernel here is real and even, so its spectrum is real, and a real
//! convolution maps real sources to real potentials. Two real source
//! vectors are therefore convolved in one complex pass as `q₁ + i q₂`.
//!
//! # Pruning
//!
//! The sources are zero outside the leading `N₀ × N₁ × N₂` corner of the
//! padded grid, and the potentials are only wanted there. The 3-D transform
//! is done axis by axis — axis 2, then 1, then 0 forward; the reverse
//! inverse — so every 1-D transform whose input line is known to be zero,
//! or whose output is never read, is skipped: forward, only `N₀·N₁` of the
//! `M₀·M₁` axis-2 lines and `N₀` of the `M₀` axis-1 planes are transformed;
//! inverse, symmetrically. Only the axis-0 transforms run over the whole
//! padded grid, and those are fused with the product by the spectrum in one
//! cache-resident pass. That is a little over half the work of a full
//! padded 3-D FFT pair, and only `N₀` of the `M₀` planes are ever stored in
//! grid order.

use std::sync::Arc;

use num_complex::Complex64;
use rayon::prelude::*;
use rustfft::{Fft, FftPlanner};

/// Complex values per parallel task in the line passes: small tasks cost
/// more in scheduling than they save.
const PARALLEL_CHUNK: usize = 1 << 14;
/// Axis-0 columns gathered per block, so the strided gather reads runs of
/// contiguous values instead of single ones.
const COLUMN_BLOCK: usize = 16;

const ZERO: Complex64 = Complex64::new(0.0, 0.0);

/// A planned convolution on a fixed grid.
pub(crate) struct Convolver {
    dims: [usize; 3],
    padded: [usize; 3],
    /// Real spectrum of the kernel on the padded grid, in axis-0-column
    /// order (index `c·M₀ + i` for column `c = j·M₂ + k`), including the
    /// `1/M` normalization of the inverse transform.
    spectrum: Vec<f64>,
    forward: [Arc<dyn Fft<f64>>; 3],
    inverse: [Arc<dyn Fft<f64>>; 3],
}

impl std::fmt::Debug for Convolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Convolver")
            .field("dims", &self.dims)
            .field("padded", &self.padded)
            .finish_non_exhaustive()
    }
}

/// Smallest `2^a 3^b 5^c` at or above `n` — sizes on which FFTs are fast.
pub(crate) fn smooth_size(n: usize) -> usize {
    let mut m = n.max(1);
    loop {
        let mut r = m;
        for p in [2, 3, 5] {
            while r % p == 0 {
                r /= p;
            }
        }
        if r == 1 {
            return m;
        }
        m += 1;
    }
}

/// Padded size along one axis for aperiodic convolution on `n` points.
pub(crate) fn padded_size(n: usize) -> usize {
    smooth_size(2 * n - 1)
}

impl Convolver {
    /// Plans the convolution of grids of `dims` points with `kernel`, a
    /// function of the integer offset `g − h`. `kernel` must be even in
    /// every component of the offset.
    pub(crate) fn new(dims: [usize; 3], kernel: impl Fn([i64; 3]) -> f64 + Sync) -> Self {
        let padded = dims.map(padded_size);
        let mut planner = FftPlanner::new();
        let forward = padded.map(|m| planner.plan_fft_forward(m));
        let inverse = padded.map(|m| planner.plan_fft_inverse(m));
        let mut convolver = Self {
            dims,
            padded,
            spectrum: Vec::new(),
            forward,
            inverse,
        };

        // The kernel on the padded grid, wrapped: offset `d` sits at `d` for
        // `d ≥ 0` and at `M + d` for `d < 0`; offsets beyond `±(N − 1)` never
        // occur between grid points and stay zero.
        let offset = |k: usize, axis: usize| -> Option<i64> {
            let (n, m) = (dims[axis], padded[axis]);
            if k < n {
                Some(k as i64)
            } else if k + n > m {
                Some(k as i64 - m as i64)
            } else {
                None
            }
        };
        let [m0, m1, m2] = padded;
        let mut data = vec![ZERO; m0 * m1 * m2];
        data.par_chunks_mut(m1 * m2)
            .enumerate()
            .for_each(|(i, slab)| {
                let Some(di) = offset(i, 0) else { return };
                for (rest, value) in slab.iter_mut().enumerate() {
                    let (j, k) = (rest / m2, rest % m2);
                    if let (Some(dj), Some(dk)) = (offset(j, 1), offset(k, 2)) {
                        *value = Complex64::new(kernel([di, dj, dk]), 0.0);
                    }
                }
            });
        // The full (unpruned) forward transform of the kernel.
        convolver.planes(&mut data, m1, false);
        let mut columns = convolver.gather(&data);
        drop(data);
        lines(&mut columns, m0, &convolver.forward[0]);
        let scale = 1.0 / (m0 * m1 * m2) as f64;
        // An even real kernel has a real spectrum; the imaginary parts are
        // rounding error and are dropped.
        convolver.spectrum = columns.iter().map(|z| z.re * scale).collect();
        convolver
    }

    /// Points of the padded grid.
    pub(crate) fn padded_len(&self) -> usize {
        self.padded.iter().product()
    }

    /// Dimensions of the padded grid.
    pub(crate) fn padded_dims(&self) -> [usize; 3] {
        self.padded
    }

    /// Convolves `first` and, if given, `second` with the kernel. Both are
    /// indexed like the unpadded grid (`k` fastest). Returns the two
    /// potentials; the second is empty when `second` is `None`.
    pub(crate) fn convolve(&self, first: &[f64], second: Option<&[f64]>) -> (Vec<f64>, Vec<f64>) {
        let [n0, n1, n2] = self.dims;
        let [m0, m1, m2] = self.padded;
        // Only the first N₀ planes are ever non-zero in grid order.
        let mut data = vec![ZERO; n0 * m1 * m2];
        data.par_chunks_mut(m1 * m2)
            .enumerate()
            .for_each(|(i, slab)| {
                for j in 0..n1 {
                    let source = (i * n1 + j) * n2;
                    let target = &mut slab[j * m2..j * m2 + n2];
                    for (k, value) in target.iter_mut().enumerate() {
                        let im = second.map_or(0.0, |s| s[source + k]);
                        *value = Complex64::new(first[source + k], im);
                    }
                }
            });
        self.planes(&mut data, n1, false);
        let mut columns = self.gather(&data);
        // Axis 0 forward, the spectrum product and axis 0 inverse, a batch of
        // columns at a time while it is in cache.
        let batch = m0 * (PARALLEL_CHUNK / m0).max(1);
        let (forward, inverse) = (&self.forward[0], &self.inverse[0]);
        let scratch_len = forward
            .get_inplace_scratch_len()
            .max(inverse.get_inplace_scratch_len());
        columns
            .par_chunks_mut(batch)
            .zip(self.spectrum.par_chunks(batch))
            .for_each_init(
                || vec![ZERO; scratch_len],
                |scratch, (chunk, spectrum)| {
                    forward.process_with_scratch(chunk, scratch);
                    chunk.iter_mut().zip(spectrum).for_each(|(z, &s)| *z *= s);
                    inverse.process_with_scratch(chunk, scratch);
                },
            );
        self.scatter(&columns, &mut data);
        drop(columns);
        self.planes(&mut data, n1, true);

        let extract = |part: fn(&Complex64) -> f64| {
            let mut out = vec![0.0; n0 * n1 * n2];
            out.par_chunks_mut(n1 * n2)
                .enumerate()
                .for_each(|(i, slab)| {
                    for j in 0..n1 {
                        let source = (i * m1 + j) * m2;
                        for k in 0..n2 {
                            slab[j * n2 + k] = part(&data[source + k]);
                        }
                    }
                });
            out
        };
        let out_first = extract(|z| z.re);
        let out_second = if second.is_some() {
            extract(|z| z.im)
        } else {
            Vec::new()
        };
        (out_first, out_second)
    }

    /// Axes 2 and 1 of the transform, plane by plane, over the planes in
    /// `data` (each `M₁ × M₂`). Only the first `rows` axis-2 lines of each
    /// plane are transformed: forward, the others are zero; inverse, they
    /// are not needed. Forward does axis 2 then 1, inverse 1 then 2.
    fn planes(&self, data: &mut [Complex64], rows: usize, inverse: bool) {
        let [_, m1, m2] = self.padded;
        let plans = if inverse {
            &self.inverse
        } else {
            &self.forward
        };
        let scratch_len = plans[1]
            .get_inplace_scratch_len()
            .max(plans[2].get_inplace_scratch_len());
        data.par_chunks_mut(m1 * m2).for_each_init(
            || (vec![ZERO; m1 * m2], vec![ZERO; scratch_len]),
            |(transposed, scratch), plane| {
                let axis2 = |plane: &mut [Complex64], scratch: &mut [Complex64]| {
                    if m2 > 1 {
                        plans[2].process_with_scratch(&mut plane[..rows * m2], scratch);
                    }
                };
                if !inverse {
                    axis2(plane, scratch);
                }
                if m1 > 1 {
                    transpose(plane, transposed, m1, m2);
                    plans[1].process_with_scratch(transposed, scratch);
                    transpose(transposed, plane, m2, m1);
                }
                if inverse {
                    axis2(plane, scratch);
                }
            },
        );
    }

    /// The axis-0 lines of the padded grid as contiguous columns
    /// (`columns[c·M₀ + i] = data[i·M₁M₂ + c]`), zero beyond the planes held
    /// in `data`.
    fn gather(&self, data: &[Complex64]) -> Vec<Complex64> {
        let [m0, m1, m2] = self.padded;
        let stride = m1 * m2;
        let planes = data.len() / stride;
        let mut columns = vec![ZERO; m0 * stride];
        columns
            .par_chunks_mut(COLUMN_BLOCK * m0)
            .enumerate()
            .for_each(|(block, out)| {
                let first = block * COLUMN_BLOCK;
                let width = out.len() / m0;
                for i in 0..planes {
                    let row = &data[i * stride + first..i * stride + first + width];
                    for (b, &value) in row.iter().enumerate() {
                        out[b * m0 + i] = value;
                    }
                }
            });
        columns
    }

    /// The inverse of [`gather`](Self::gather), for the planes `data` holds.
    fn scatter(&self, columns: &[Complex64], data: &mut [Complex64]) {
        let [m0, m1, m2] = self.padded;
        let stride = m1 * m2;
        // A few planes per task, so each column's consecutive entries are
        // read together.
        const PLANES_PER_TASK: usize = 4;
        data.par_chunks_mut(PLANES_PER_TASK * stride)
            .enumerate()
            .for_each(|(task, out)| {
                let first = task * PLANES_PER_TASK;
                let planes = out.len() / stride;
                for c in 0..stride {
                    let column = &columns[c * m0 + first..c * m0 + first + planes];
                    for (p, &value) in column.iter().enumerate() {
                        out[p * stride + c] = value;
                    }
                }
            });
    }
}

/// Transforms every contiguous line of length `len` in `data`, in parallel.
fn lines(data: &mut [Complex64], len: usize, plan: &Arc<dyn Fft<f64>>) {
    // Batches of lines amortize the scratch buffer and rayon's overhead.
    let batch = len * (PARALLEL_CHUNK / len).max(1);
    data.par_chunks_mut(batch).for_each_init(
        || vec![ZERO; plan.get_inplace_scratch_len()],
        |scratch, chunk| plan.process_with_scratch(chunk, scratch),
    );
}

/// `to` (`cols × rows`) = transpose of `from` (`rows × cols`), row-major.
fn transpose(from: &[Complex64], to: &mut [Complex64], rows: usize, cols: usize) {
    for r in 0..rows {
        for c in 0..cols {
            to[c * rows + r] = from[r * cols + c];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smooth_sizes() {
        assert_eq!(smooth_size(1), 1);
        assert_eq!(smooth_size(7), 8);
        assert_eq!(smooth_size(11), 12);
        assert_eq!(smooth_size(49), 50);
        assert_eq!(padded_size(1), 1);
        assert_eq!(padded_size(4), 8);
    }

    fn direct_sum_check(dims: [usize; 3]) {
        let kernel = |d: [i64; 3]| {
            let r2 = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) as f64;
            if r2 == 0.0 {
                3.0
            } else {
                1.0 / r2.sqrt()
            }
        };
        let conv = Convolver::new(dims, kernel);
        let len = dims.iter().product::<usize>();
        let a: Vec<f64> = (0..len).map(|i| ((i * 37) % 11) as f64 - 5.0).collect();
        let b: Vec<f64> = (0..len).map(|i| ((i * 13) % 7) as f64 * 0.5).collect();
        let (pa, pb) = conv.convolve(&a, Some(&b));
        let (pa_alone, empty) = conv.convolve(&a, None);
        assert!(empty.is_empty());
        let point = |i: usize| {
            let k = i % dims[2];
            let rest = i / dims[2];
            [(rest / dims[1]) as i64, (rest % dims[1]) as i64, k as i64]
        };
        for g in 0..len {
            let (mut da, mut db) = (0.0, 0.0);
            for h in 0..len {
                let (pg, ph) = (point(g), point(h));
                let k = kernel([pg[0] - ph[0], pg[1] - ph[1], pg[2] - ph[2]]);
                da += k * a[h];
                db += k * b[h];
            }
            assert!((pa[g] - da).abs() < 1e-12 * da.abs().max(1.0), "{dims:?}");
            assert!((pb[g] - db).abs() < 1e-12 * db.abs().max(1.0), "{dims:?}");
            assert!(
                (pa_alone[g] - da).abs() < 1e-12 * da.abs().max(1.0),
                "{dims:?}"
            );
        }
    }

    #[test]
    fn fft_convolution_equals_direct_sum() {
        // Flat, thin and single-point axes, and a grid with more axis-0
        // columns than one gather block.
        for dims in [[5, 1, 7], [1, 1, 1], [1, 6, 1], [3, 4, 5], [7, 5, 6]] {
            direct_sum_check(dims);
        }
    }
}
