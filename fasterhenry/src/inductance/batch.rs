//! Batched evaluation of partial-inductance matrices.

use nalgebra::DMatrix;
use rayon::prelude::*;

use super::{evaluate, KernelError};
use crate::filament::Filament;

/// How a batch is executed. [`mutual_batch`] uses [`Execution::Parallel`];
/// the other modes exist so that the benchmark can separate the speed-up of
/// the SIMD inner quadrature from that of the thread pool.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Execution {
    /// One thread, one kernel evaluation at a time.
    Scalar,
    /// One thread, the inner quadrature in `wide::f64x4` lanes.
    Simd,
    /// SIMD lanes within each pair and `rayon` across the rows.
    #[default]
    Parallel,
}

/// Partial mutual inductances between every filament of `rows` and every
/// filament of `cols`: entry `(i, j)` is
/// [`mutual_inductance`](super::mutual_inductance)`(&rows[i], &cols[j])`,
/// in henries.
///
/// The inner Gauss–Legendre quadrature runs in SIMD lanes and the rows are
/// distributed over the `rayon` thread pool. Results do not depend on the
/// number of threads.
///
/// # Errors
///
/// The first [`KernelError`] encountered, carrying the `(row, column)` of the
/// offending pair.
pub fn mutual_batch(rows: &[Filament], cols: &[Filament]) -> Result<DMatrix<f64>, KernelError> {
    mutual_batch_with(rows, cols, Execution::Parallel)
}

/// [`mutual_batch`] with an explicit [`Execution`] mode. All modes agree to
/// rounding error.
///
/// # Errors
///
/// As [`mutual_batch`].
pub fn mutual_batch_with(
    rows: &[Filament],
    cols: &[Filament],
    execution: Execution,
) -> Result<DMatrix<f64>, KernelError> {
    let (nrows, ncols) = (rows.len(), cols.len());
    if nrows == 0 || ncols == 0 {
        return Ok(DMatrix::zeros(nrows, ncols));
    }
    let simd = execution != Execution::Scalar;
    let fill = |(i, out): (usize, &mut [f64])| -> Result<(), KernelError> {
        for (j, slot) in out.iter_mut().enumerate() {
            *slot = evaluate(&rows[i], &cols[j], simd)
                .map_err(|e| e.at(i, j))?
                .value;
        }
        Ok(())
    };
    let mut data = vec![0.0; nrows * ncols];
    if execution == Execution::Parallel {
        data.par_chunks_mut(ncols).enumerate().try_for_each(fill)?;
    } else {
        data.chunks_mut(ncols).enumerate().try_for_each(fill)?;
    }
    Ok(DMatrix::from_row_slice(nrows, ncols, &data))
}

/// The full symmetric partial-inductance matrix `L` of a set of filaments:
/// self terms on the diagonal, mutual terms elsewhere.
///
/// Equal to `mutual_batch(filaments, filaments)` — which is itself symmetric
/// to the last bit — at half the cost, since only the upper triangle is
/// evaluated.
///
/// # Errors
///
/// As [`mutual_batch`].
pub fn partial_inductance_matrix(filaments: &[Filament]) -> Result<DMatrix<f64>, KernelError> {
    let n = filaments.len();
    let upper: Vec<Vec<f64>> = (0..n)
        .into_par_iter()
        .map(|i| {
            (i..n)
                .map(|j| {
                    evaluate(&filaments[i], &filaments[j], true)
                        .map(|m| m.value)
                        .map_err(|e| e.at(i, j))
                })
                .collect()
        })
        .collect::<Result<_, _>>()?;
    Ok(DMatrix::from_fn(n, n, |i, j| {
        let (lo, hi) = (i.min(j), i.max(j));
        upper[lo][hi - lo]
    }))
}
