//! Dense LU solve for the mesh system.
//!
//! [`lu_solve`] is Gaussian elimination with partial (row) pivoting, the same
//! factorization `PA = LU` as `nalgebra`'s [`LU`](nalgebra::linalg::LU), in
//! the *blocked right-looking* arrangement of Golub & Van Loan, *Matrix
//! Computations* (4th ed., §3.2.11 and §3.4): a narrow panel of columns is
//! factorized, then every remaining column is updated by that whole panel at
//! once.
//!
//! The arrangement is what matters for speed. An unblocked elimination sweeps
//! the entire trailing matrix once per column, so at the mesh orders of
//! interest (≈ 2 000, a 64 MB complex matrix) it is bound by memory bandwidth;
//! here each trailing column is brought into cache once per panel, and the
//! trailing columns — which are independent of one another — are updated in
//! parallel on the `rayon` thread pool. Every column sees the same sequence
//! of floating-point operations whatever the number of threads, so the result
//! does not depend on it.
//!
//! The crate's tests check the solution against `nalgebra`'s own LU.

use nalgebra::{ComplexField, DMatrix};
use rayon::prelude::*;

/// Columns per panel. The panel (`order × PANEL` entries) should stay within
/// the outer caches while a trailing column is updated against it.
const PANEL: usize = 32;

/// Below this many entries a trailing update or substitution is not worth
/// handing to the thread pool.
const PARALLEL_THRESHOLD: usize = 1 << 14;

/// Solves `a · x = b` for `x` by blocked LU factorization with partial
/// pivoting, consuming `a` as workspace. `b` may hold any number of
/// right-hand-side columns.
///
/// Returns `None` if a pivot is exactly zero (`a` is singular) — the contract
/// of `nalgebra`'s `LU::solve`. A non-finite `a` yields a non-finite `x`, not
/// an error.
///
/// # Panics
///
/// If `a` is not square or `b` has a different number of rows.
pub fn lu_solve<T>(a: DMatrix<T>, b: &DMatrix<T>) -> Option<DMatrix<T>>
where
    T: ComplexField + Copy + Send + Sync,
{
    assert!(a.is_square(), "lu_solve: the matrix must be square");
    assert_eq!(
        a.nrows(),
        b.nrows(),
        "lu_solve: right-hand side has the wrong number of rows"
    );
    let order = a.nrows();
    if order == 0 {
        return Some(b.clone());
    }
    let mut x = b.clone();
    let mut lu = a;
    let pivots = factorize(lu.as_mut_slice(), order)?;

    let lu = lu.as_slice();
    let substitute = |rhs: &mut [T]| {
        for (row, &pivot) in pivots.iter().enumerate() {
            rhs.swap(row, pivot);
        }
        // Forward: L has a unit diagonal.
        for c in 0..order {
            let (head, tail) = rhs.split_at_mut(c + 1);
            subtract_scaled(tail, head[c], &lu[c * order + c + 1..(c + 1) * order]);
        }
        // Backward.
        for c in (0..order).rev() {
            let (head, tail) = rhs.split_at_mut(c);
            tail[0] /= lu[c * order + c];
            subtract_scaled(head, tail[0], &lu[c * order..c * order + c]);
        }
    };
    if order * order >= PARALLEL_THRESHOLD {
        x.as_mut_slice().par_chunks_mut(order).for_each(substitute);
    } else {
        x.as_mut_slice().chunks_mut(order).for_each(substitute);
    }
    Some(x)
}

/// `y ← y − factor · x`, element by element.
#[inline]
fn subtract_scaled<T: ComplexField + Copy>(y: &mut [T], factor: T, x: &[T]) {
    debug_assert_eq!(y.len(), x.len());
    for (y, &x) in y.iter_mut().zip(x) {
        *y -= factor * x;
    }
}

/// In-place `PA = LU` of the column-major `order × order` matrix `a`: `U` on
/// and above the diagonal, the unit-lower-triangular `L` below it. Returns,
/// for every row, the row it was interchanged with; `None` on a zero pivot.
fn factorize<T>(a: &mut [T], order: usize) -> Option<Vec<usize>>
where
    T: ComplexField + Copy + Send + Sync,
{
    let mut pivots = vec![0; order];
    for start in (0..order).step_by(PANEL) {
        let width = PANEL.min(order - start);
        let (left, rest) = a.split_at_mut(start * order);
        let (panel, right) = rest.split_at_mut(width * order);

        factorize_panel(panel, order, start, &mut pivots[start..start + width])?;
        let (panel, pivots) = (&*panel, &pivots[start..start + width]);
        let interchange = |column: &mut [T]| {
            for (offset, &pivot) in pivots.iter().enumerate() {
                column.swap(start + offset, pivot);
            }
        };

        // Columns already factorized only follow the row interchanges.
        left.chunks_mut(order).for_each(interchange);

        let update = |column: &mut [T]| {
            interchange(column);
            // Rows `start..start + width`: forward substitution with the
            // panel's unit-lower-triangular block gives this column of `U`.
            // Rows below: subtract the panel's `L` times that column of `U`.
            // Both are "subtract a multiple of panel column `c` from the rows
            // under `start + c`".
            for c in 0..width {
                let (head, tail) = column.split_at_mut(start + c + 1);
                let l = &panel[c * order + start + c + 1..(c + 1) * order];
                subtract_scaled(tail, head[start + c], l);
            }
        };
        if (order - start) * right.len() / order >= PARALLEL_THRESHOLD {
            right.par_chunks_mut(order).for_each(update);
        } else {
            right.chunks_mut(order).for_each(update);
        }
    }
    Some(pivots)
}

/// Unblocked partial-pivoting elimination of a panel of full-height columns
/// whose first is column `start` of the matrix; only rows `start..` take
/// part. Interchanges are applied within the panel and recorded in `pivots`.
fn factorize_panel<T>(
    panel: &mut [T],
    order: usize,
    start: usize,
    pivots: &mut [usize],
) -> Option<()>
where
    T: ComplexField + Copy,
{
    let width = pivots.len();
    for c in 0..width {
        let diagonal = start + c;
        let column = &panel[c * order..(c + 1) * order];
        // First row of largest |re| + |im|, as LAPACK and nalgebra choose it.
        let mut pivot = diagonal;
        let mut largest = column[diagonal].norm1();
        for (row, value) in column.iter().enumerate().skip(diagonal + 1) {
            let magnitude = value.norm1();
            if magnitude > largest {
                (pivot, largest) = (row, magnitude);
            }
        }
        if largest == nalgebra::zero() {
            return None;
        }
        pivots[c] = pivot;
        if pivot != diagonal {
            for column in panel.chunks_mut(order) {
                column.swap(diagonal, pivot);
            }
        }

        let (done, todo) = panel.split_at_mut((c + 1) * order);
        let column = &mut done[c * order..];
        let inverse = nalgebra::one::<T>() / column[diagonal];
        for value in &mut column[diagonal + 1..] {
            *value *= inverse;
        }
        let multipliers = &column[diagonal + 1..];
        for other in todo.chunks_mut(order) {
            let (head, tail) = other.split_at_mut(diagonal + 1);
            subtract_scaled(tail, head[diagonal], multipliers);
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_complex::Complex;

    /// Deterministic pseudo-random numbers in `[-1, 1)` (SplitMix64).
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> f64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            (z >> 11) as f64 / (1u64 << 52) as f64 - 1.0
        }
    }

    fn complex_matrix(rng: &mut Rng, rows: usize, cols: usize) -> DMatrix<Complex<f64>> {
        DMatrix::from_fn(rows, cols, |_, _| Complex::new(rng.next(), rng.next()))
    }

    fn relative_difference(a: &DMatrix<Complex<f64>>, b: &DMatrix<Complex<f64>>) -> f64 {
        (a - b).norm() / b.norm()
    }

    #[test]
    fn matches_nalgebra_lu_on_general_complex_systems() {
        let mut rng = Rng(1);
        // Orders straddling the panel width, and one large enough to take the
        // parallel paths.
        for order in [1, 2, 5, PANEL - 1, PANEL, PANEL + 1, 3 * PANEL + 7, 200] {
            let a = complex_matrix(&mut rng, order, order);
            let b = complex_matrix(&mut rng, order, 3);
            let expected = a.clone().lu().solve(&b).expect("nalgebra solves it");
            let x = lu_solve(a.clone(), &b).expect("non-singular");
            assert!(
                relative_difference(&x, &expected) < 1e-10,
                "order {order}: differs from nalgebra by {:e}",
                relative_difference(&x, &expected)
            );
            let residual = (&a * &x - &b).norm() / (a.norm() * x.norm());
            assert!(residual < 1e-14, "order {order}: residual {residual:e}");
        }
    }

    #[test]
    fn pivots_when_the_leading_entry_is_zero() {
        let a = DMatrix::from_row_slice(3, 3, &[0.0, 2.0, 1.0, 1.0, 1.0, 0.0, 3.0, 0.0, 1.0]);
        let b = DMatrix::from_row_slice(3, 1, &[5.0, 3.0, 6.0]);
        let x = lu_solve(a.clone(), &b).expect("non-singular");
        assert!((&a * &x - &b).norm() < 1e-14);
    }

    #[test]
    fn real_systems_and_many_right_hand_sides() {
        let mut rng = Rng(7);
        let order = 2 * PANEL + 3;
        let a = DMatrix::from_fn(order, order, |_, _| rng.next());
        let b = DMatrix::from_fn(order, order, |_, _| rng.next());
        let x = lu_solve(a.clone(), &b).expect("non-singular");
        let expected = a.clone().lu().solve(&b).expect("nalgebra solves it");
        assert!((&x - &expected).norm() / expected.norm() < 1e-10);
    }

    #[test]
    fn singular_matrix_is_reported() {
        let a = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 2.0, 4.0]);
        // Elimination leaves an exactly zero second pivot.
        assert!(lu_solve(a, &DMatrix::from_element(2, 1, 1.0)).is_none());
        let zero = DMatrix::<f64>::zeros(3, 3);
        assert!(lu_solve(zero, &DMatrix::from_element(3, 1, 1.0)).is_none());
    }

    #[test]
    fn empty_system_is_trivially_solved() {
        let x = lu_solve(DMatrix::<f64>::zeros(0, 0), &DMatrix::zeros(0, 2)).unwrap();
        assert_eq!(x.shape(), (0, 2));
    }

    #[test]
    fn result_does_not_depend_on_the_thread_count() {
        let mut rng = Rng(3);
        let a = complex_matrix(&mut rng, 300, 300);
        let b = complex_matrix(&mut rng, 300, 2);
        let single = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap()
            .install(|| lu_solve(a.clone(), &b).unwrap());
        assert_eq!(single, lu_solve(a, &b).unwrap());
    }
}
