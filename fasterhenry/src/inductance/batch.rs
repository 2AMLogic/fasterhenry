//! Batched evaluation of partial-inductance matrices.

use nalgebra::DMatrix;
use rayon::prelude::*;

use super::{evaluate, KernelError, Mutual};
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

/// Partial-inductance values together with entries whose accuracy criterion
/// was not met. See [`Mutual::resolved`] for the reduced-accuracy regime.
#[derive(Clone, Debug, PartialEq)]
pub struct MutualBatch {
    /// Partial inductances in henries, with the same shape and values as the
    /// corresponding value-only batch function.
    pub values: DMatrix<f64>,
    /// Zero-based `(row, column)` entries with [`Mutual::resolved`]` == false`,
    /// sorted in row-major order, without duplicates. An empty list means
    /// every entry met its accuracy criterion.
    ///
    /// A symmetric matrix reports both `(i, j)` and `(j, i)`, so the length
    /// counts matrix entries, not unordered filament pairs.
    pub unresolved_pairs: Vec<(usize, usize)>,
}

/// Partial mutual inductances between every filament of `rows` and every
/// filament of `cols`: entry `(i, j)` is
/// [`mutual_inductance`](super::mutual_inductance)`(&rows[i], &cols[j])`,
/// in henries.
///
/// The inner Gauss–Legendre quadrature runs in SIMD lanes and the rows are
/// distributed over the `rayon` thread pool. Results do not depend on the
/// number of threads.
/// Use [`mutual_batch_detailed`] to also identify reduced-accuracy entries.
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
    let data = evaluate_batch(rows, cols, execution, |m| m.value)?;
    Ok(DMatrix::from_row_slice(rows.len(), cols.len(), &data))
}

/// [`mutual_batch`] with a report of reduced-accuracy entries. Uses
/// [`Execution::Parallel`], and evaluates each pair only once.
///
/// # Errors
///
/// As [`mutual_batch`]. Unresolved pairs are reported, not treated as errors.
pub fn mutual_batch_detailed(
    rows: &[Filament],
    cols: &[Filament],
) -> Result<MutualBatch, KernelError> {
    mutual_batch_detailed_with(rows, cols, Execution::Parallel)
}

/// [`mutual_batch_detailed`] with an explicit [`Execution`] mode. The report
/// order is independent of the execution mode and thread count.
///
/// # Errors
///
/// As [`mutual_batch`].
pub fn mutual_batch_detailed_with(
    rows: &[Filament],
    cols: &[Filament],
    execution: Execution,
) -> Result<MutualBatch, KernelError> {
    let data = evaluate_batch(rows, cols, execution, |m| (m.value, m.resolved))?;
    let (nrows, ncols) = (rows.len(), cols.len());
    let unresolved_pairs = data
        .iter()
        .enumerate()
        .filter(|(_, (_, resolved))| !resolved)
        .map(|(index, _)| (index / ncols, index % ncols))
        .collect();
    Ok(MutualBatch {
        values: DMatrix::from_fn(nrows, ncols, |i, j| data[i * ncols + j].0),
        unresolved_pairs,
    })
}

// Keep the value-only path's storage and execution policy: it retains only
// f64s, while the detailed path retains each pair's flag as well.
fn evaluate_batch<T: Clone + Default + Send>(
    rows: &[Filament],
    cols: &[Filament],
    execution: Execution,
    select: impl Fn(Mutual) -> T + Sync,
) -> Result<Vec<T>, KernelError> {
    let (nrows, ncols) = (rows.len(), cols.len());
    if nrows == 0 || ncols == 0 {
        return Ok(Vec::new());
    }
    let simd = execution != Execution::Scalar;
    let fill = |(i, out): (usize, &mut [T])| -> Result<(), KernelError> {
        for (j, slot) in out.iter_mut().enumerate() {
            *slot = select(evaluate(&rows[i], &cols[j], simd).map_err(|e| e.at(i, j))?);
        }
        Ok(())
    };
    let mut data = vec![T::default(); nrows * ncols];
    if execution == Execution::Parallel {
        data.par_chunks_mut(ncols).enumerate().try_for_each(fill)?;
    } else {
        data.chunks_mut(ncols).enumerate().try_for_each(fill)?;
    }
    Ok(data)
}

/// The full symmetric partial-inductance matrix `L` of a set of filaments:
/// self terms on the diagonal, mutual terms elsewhere.
///
/// Equal to `mutual_batch(filaments, filaments)` — which is itself symmetric
/// to the last bit — at half the cost, since only the upper triangle is
/// evaluated.
/// Use [`partial_inductance_matrix_detailed`] to also identify reduced-accuracy
/// entries.
///
/// # Errors
///
/// As [`mutual_batch`].
pub fn partial_inductance_matrix(filaments: &[Filament]) -> Result<DMatrix<f64>, KernelError> {
    let upper = evaluate_upper(filaments, |m| m.value)?;
    let n = filaments.len();
    Ok(DMatrix::from_fn(n, n, |i, j| {
        let (lo, hi) = (i.min(j), i.max(j));
        upper[lo][hi - lo]
    }))
}

/// [`partial_inductance_matrix`] with a report of reduced-accuracy entries.
/// Only the upper triangle is evaluated, then values and accuracy flags are
/// mirrored. The report includes both triangles in row-major order.
///
/// # Errors
///
/// As [`partial_inductance_matrix`], with errors attributed to the evaluated
/// upper-triangle pair. Unresolved pairs are reported, not treated as errors.
pub fn partial_inductance_matrix_detailed(
    filaments: &[Filament],
) -> Result<MutualBatch, KernelError> {
    let upper = evaluate_upper(filaments, |m| (m.value, m.resolved))?;
    let n = filaments.len();
    let mut unresolved_pairs = Vec::new();
    for (i, row) in upper.iter().enumerate() {
        for (offset, &(_, resolved)) in row.iter().enumerate() {
            if !resolved {
                let j = i + offset;
                unresolved_pairs.push((i, j));
                if i != j {
                    unresolved_pairs.push((j, i));
                }
            }
        }
    }
    unresolved_pairs.sort_unstable();
    Ok(MutualBatch {
        values: DMatrix::from_fn(n, n, |i, j| {
            let (lo, hi) = (i.min(j), i.max(j));
            upper[lo][hi - lo].0
        }),
        unresolved_pairs,
    })
}

fn evaluate_upper<T: Send>(
    filaments: &[Filament],
    select: impl Fn(Mutual) -> T + Sync,
) -> Result<Vec<Vec<T>>, KernelError> {
    let n = filaments.len();
    (0..n)
        .into_par_iter()
        .map(|i| {
            (i..n)
                .map(|j| {
                    evaluate(&filaments[i], &filaments[j], true)
                        .map(&select)
                        .map_err(|e| e.at(i, j))
                })
                .collect()
        })
        .collect()
}
