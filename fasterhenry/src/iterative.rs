//! Matrix-free port impedance extraction: GMRES on the precorrected-FFT
//! operator.
//!
//! [`IterativeSystem`] computes the same port impedance matrix as
//! [`MeshSystem`](crate::MeshSystem),
//!
//! ```text
//! Z(ω) = Z_pp − Z_pe · Z_ee⁻¹ · Z_ep ,     Z_xy = M_x (R + jωL) M_yᵀ ,
//! ```
//!
//! (see [`mod@crate::solve`] for the formulation), without ever forming the
//! partial-inductance matrix `L` or the dense internal-loop block `Z_ee`.
//! Instead:
//!
//! * `L·x` is the matrix-free [`PfftOperator`], whose near-field pairs are
//!   exact and whose far field is accurate to about `1e-7` at the default
//!   [`PfftParams`];
//! * the action of `Z_ee` on a vector of internal loop currents `x` is
//!   `M_e · (R·c + jω·L·c)` with `c = M_eᵀ·x` the branch currents: two
//!   sparse products with the loop basis, a diagonal, and two real pFFT
//!   products (one each for the real and imaginary part of `c`, since `L`
//!   is real);
//! * `Z_ee⁻¹ · Z_ep` is solved column by column — one restarted
//!   [GMRES](crate::gmres) solve per port — with a Jacobi (diagonal)
//!   preconditioner;
//! * the port columns `Z_ep` and the block `Z_pp` need one pFFT product per
//!   port, done once at assembly, since `R` and `L` are frequency
//!   independent.
//!
//! Memory is therefore `O(n)` in the number of filaments — the operator, a
//! few branch vectors and `restart + 1` Krylov vectors of the internal loop
//! count — instead of the dense path's `O(n²)`, and each GMRES iteration
//! costs two pFFT products.
//!
//! # One GMRES solve per port
//!
//! GMRES solves one right-hand side at a time. The ports are solved one
//! after another, each from a zero initial guess; a block variant would
//! share one Krylov space between the ports, but ports are few, the pFFT
//! product is already parallel internally, and independent solves keep every
//! column's convergence test and failure report separate.
//!
//! # Accuracy
//!
//! Two approximations separate the result from the dense path: the pFFT
//! far field (see [`crate::pfft`]) and the GMRES tolerance (relative
//! residual [`GmresParams::tolerance`], `1e-10` by default). At the defaults
//! the result matches [`MeshSystem`](crate::MeshSystem) to far better than
//! `1e-4` relative on the crate's fixtures, which the tests check. The
//! result is symmetrized, `Z ← (Z + Zᵀ)/2`, since reciprocity holds exactly
//! and only the approximations break it.
//!
//! # The DC limit
//!
//! At `f = 0` the internal system is `M_e R M_eᵀ`: sparse, real, symmetric
//! positive definite, and independent of `L`. It is solved by GMRES in real
//! arithmetic without a single pFFT product, and the imaginary part of the
//! result is exactly zero — as for the dense path.
//!
//! # Scope
//!
//! Every pair of filaments is coupled; a [`Coupling`](crate::Coupling)
//! truncation is a dense-path feature. Which path to use for a given size
//! stays the caller's decision, with a default for callers that would rather
//! not choose: [`SolverChoice::Auto`](crate::SolverChoice::Auto) keeps
//! [`MeshSystem`](crate::MeshSystem) up to
//! [`DENSE_PATH_MAX_FILAMENTS`](crate::DENSE_PATH_MAX_FILAMENTS) filaments
//! and selects this path above it (measurements in `docs/benchmarks.md`).

use std::time::Instant;

use nalgebra::{ComplexField, DMatrix};
use num_complex::Complex;
use rayon::prelude::*;

use crate::currents::{assemble_solution, CurrentSolution, Layout};
use crate::filament::Filament;
use crate::geometry::Geometry;
use crate::gmres::{gmres, GmresOutcome, GmresParams};
use crate::inductance::{mutual_inductance, self_inductance, KernelError};
use crate::mesh::{MeshMatrix, Port};
use crate::pfft::{PfftOperator, PfftParams, PfftStats};
use crate::result::{Counts, SweepResult};
use crate::solve::{
    check_excitation, check_frequency, complex_diagonal, complex_impedance, discretize_geometry,
    filament_resistance, sweep_impedances, Discretization, SolveError,
};

type C64 = Complex<f64>;

/// Loops of at most this many branches get the exact diagonal
/// `Σₐ Σ_b sₐ s_b L_ab` in the Jacobi preconditioner; longer loops (long
/// conductor loops through the spanning forest) use the sum of their
/// branches' self inductances, which bounds the cost of the preconditioner
/// at `O(n)` kernel evaluations. Either way the preconditioner only affects
/// the iteration count, never the answer.
pub const EXACT_DIAGONAL_MAX_LOOP_LENGTH: usize = 32;

/// Parameters of an [`IterativeSystem`]: the pFFT operator's accuracy and
/// GMRES's stopping rule.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IterativeParams {
    /// The precorrected-FFT operator's parameters.
    pub pfft: PfftParams,
    /// GMRES's tolerance, restart length and iteration cap.
    pub gmres: GmresParams,
}

/// `Z(ω)` at one frequency with the GMRES outcome of every port's solve.
#[derive(Clone, Debug, PartialEq)]
pub struct IterativeSolution {
    /// The port impedance matrix, in ohms.
    pub impedance: DMatrix<C64>,
    /// One entry per port, in port order; empty when there are no internal
    /// loops (nothing is solved).
    pub gmres: Vec<GmresOutcome>,
}

/// The assembled, frequency-independent, matrix-free mesh system of a
/// geometry and its ports. See the [module documentation](self).
#[derive(Debug)]
pub struct IterativeSystem {
    ports: Vec<Port>,
    filaments: Vec<Filament>,
    resistances: Vec<f64>,
    mesh: MeshMatrix,
    operator: PfftOperator,
    gmres: GmresParams,
    /// `M R Mᵀ` and `M L Mᵀ` restricted to (internal × port) and
    /// (port × port) loops.
    r_ep: DMatrix<f64>,
    l_ep: DMatrix<f64>,
    r_pp: DMatrix<f64>,
    l_pp: DMatrix<f64>,
    /// Diagonals of `M_e R M_eᵀ` and (an approximation of) `M_e L M_eᵀ`,
    /// the Jacobi preconditioner.
    r_diag: Vec<f64>,
    l_diag: Vec<f64>,
    layout: Layout,
    counts: Counts,
    operator_seconds: f64,
    assembly_seconds: f64,
}

impl IterativeSystem {
    /// Discretizes `geometry`, builds the loop basis for `ports` and the
    /// pFFT operator of the filaments, and precomputes the port columns and
    /// the preconditioner.
    ///
    /// # Errors
    ///
    /// As [`MeshSystem::assemble`](crate::MeshSystem::assemble), plus
    /// [`SolveError::Pfft`] if the operator cannot be built from
    /// `params.pfft`.
    pub fn assemble(
        geometry: &Geometry,
        ports: &[Port],
        discretization: &Discretization,
        params: &IterativeParams,
    ) -> Result<Self, SolveError> {
        let start = Instant::now();
        let (filaments, per_segment) = discretize_geometry(geometry, discretization)?;
        let mesh = MeshMatrix::build(geometry, &per_segment, ports)?;
        let resistances: Vec<f64> = filaments.iter().map(filament_resistance).collect();

        let operator_start = Instant::now();
        let operator = PfftOperator::new(&filaments, &params.pfft)?;
        let operator_seconds = operator_start.elapsed().as_secs_f64();

        let (internal, port_count) = (mesh.internal_count(), mesh.port_count());
        let branches = filaments.len();
        let mut r_ep = DMatrix::zeros(internal, port_count);
        let mut l_ep = DMatrix::zeros(internal, port_count);
        let mut r_pp = DMatrix::zeros(port_count, port_count);
        let mut l_pp = DMatrix::zeros(port_count, port_count);
        for p in 0..port_count {
            let mut current = vec![0.0; branches];
            for &(branch, sign) in mesh.row(internal + p) {
                current[branch] += sign;
            }
            let flux = operator.apply(&current);
            let drop: Vec<f64> = current
                .iter()
                .zip(&resistances)
                .map(|(c, r)| c * r)
                .collect();
            let r_column = gather_all(&mesh, &drop);
            let l_column = gather_all(&mesh, &flux);
            for i in 0..internal {
                r_ep[(i, p)] = r_column[i];
                l_ep[(i, p)] = l_column[i];
            }
            for q in 0..port_count {
                r_pp[(q, p)] = r_column[internal + q];
                l_pp[(q, p)] = l_column[internal + q];
            }
        }
        // Symmetric in exact arithmetic; make it so to the last bit.
        let r_pp = (&r_pp + r_pp.transpose()) * 0.5;
        let l_pp = (&l_pp + l_pp.transpose()) * 0.5;

        let diagonals: Vec<(f64, f64)> = (0..internal)
            .into_par_iter()
            .map(|i| {
                let row = mesh.row(i);
                let r: f64 = row.iter().map(|&(branch, _)| resistances[branch]).sum();
                loop_inductance(&filaments, row).map(|l| (r, l))
            })
            .collect::<Result<_, KernelError>>()?;
        let (r_diag, l_diag) = diagonals.into_iter().unzip();

        let counts = Counts {
            nodes: geometry.nodes().len(),
            segments: geometry.segment_count(),
            filaments: branches,
            meshes: mesh.loop_count(),
            internal_meshes: internal,
            ports: port_count,
        };
        Ok(Self {
            ports: ports.to_vec(),
            filaments,
            resistances,
            mesh,
            operator,
            gmres: params.gmres,
            r_ep,
            l_ep,
            r_pp,
            l_pp,
            r_diag,
            l_diag,
            layout: Layout::new(geometry, &per_segment),
            counts,
            operator_seconds,
            assembly_seconds: start.elapsed().as_secs_f64(),
        })
    }

    /// The ports, in the order of the rows and columns of `Z`.
    pub fn ports(&self) -> &[Port] {
        &self.ports
    }

    /// Every filament (branch), segment by segment.
    pub fn filaments(&self) -> &[Filament] {
        &self.filaments
    }

    /// The loop basis.
    pub fn mesh(&self) -> &MeshMatrix {
        &self.mesh
    }

    /// Problem size: nodes, segments, filaments, meshes, ports.
    pub fn counts(&self) -> Counts {
        self.counts
    }

    /// Sizes of the pFFT operator.
    pub fn operator_stats(&self) -> PfftStats {
        self.operator.stats()
    }

    /// The GMRES parameters every solve uses.
    pub fn gmres_params(&self) -> GmresParams {
        self.gmres
    }

    /// The port impedance matrix `Z(ω)`, in ohms, at `frequency` hertz; the
    /// iterative counterpart of
    /// [`MeshSystem::impedance`](crate::MeshSystem::impedance).
    ///
    /// # Errors
    ///
    /// [`SolveError::InvalidFrequency`] for a negative or non-finite
    /// frequency (reported with index 0), [`SolveError::NotConverged`] if a
    /// GMRES solve misses its tolerance, [`SolveError::Singular`] if the
    /// result is not finite.
    pub fn impedance(&self, frequency: f64) -> Result<DMatrix<C64>, SolveError> {
        self.solve(frequency).map(|solution| solution.impedance)
    }

    /// As [`impedance`](Self::impedance), also returning how every port's
    /// GMRES solve went.
    ///
    /// # Errors
    ///
    /// As [`impedance`](Self::impedance).
    pub fn solve(&self, frequency: f64) -> Result<IterativeSolution, SolveError> {
        check_frequency(0, frequency)?;
        let (z, outcomes) = if frequency == 0.0 {
            let (z, outcomes) = schur_complement_gmres(
                &self.r_ep,
                &self.r_pp,
                |x| self.apply_dc(x),
                &self.r_diag,
                &self.gmres,
                frequency,
            )?;
            (z.map(|re| C64::new(re, 0.0)), outcomes)
        } else {
            let omega = std::f64::consts::TAU * frequency;
            let complex = |r, l| complex_impedance(r, l, omega);
            let diagonal = complex_diagonal(&self.r_diag, &self.l_diag, omega);
            schur_complement_gmres(
                &complex(&self.r_ep, &self.l_ep),
                &complex(&self.r_pp, &self.l_pp),
                |x| self.apply_ac(omega, x),
                &diagonal,
                &self.gmres,
                frequency,
            )?
        };
        let z = (&z + z.transpose()).map(|e| e * 0.5);
        if z.iter().all(|e| e.re.is_finite() && e.im.is_finite()) {
            Ok(IterativeSolution {
                impedance: z,
                gmres: outcomes,
            })
        } else {
            Err(SolveError::Singular { frequency })
        }
    }

    /// Filament currents for the port-current phasors `excitation` (one per
    /// port, amperes) at `frequency` hertz; the iterative counterpart of
    /// [`MeshSystem::currents`](crate::MeshSystem::currents).
    ///
    /// The internal loop currents `−Z_ee⁻¹·Z_ep·I_p` come from one
    /// Jacobi-preconditioned GMRES solve on the pFFT operator. The power
    /// balance of the result is accurate to a small multiple of the GMRES
    /// tolerance (see [`crate::currents`]).
    ///
    /// # Errors
    ///
    /// As [`impedance`](Self::impedance), plus
    /// [`SolveError::InvalidExcitation`].
    pub fn currents(
        &self,
        frequency: f64,
        excitation: &[C64],
    ) -> Result<CurrentSolution, SolveError> {
        check_frequency(0, frequency)?;
        check_excitation(self.counts.ports, excitation)?;
        let omega = std::f64::consts::TAU * frequency;
        let complex = |r, l| complex_impedance(r, l, omega);
        let ep = complex(&self.r_ep, &self.l_ep);
        let pp = complex(&self.r_pp, &self.l_pp);
        let current = DMatrix::from_column_slice(excitation.len(), 1, excitation);
        let internal_count = self.counts.internal_meshes;
        let mut internal = vec![C64::new(0.0, 0.0); internal_count];
        if internal_count > 0 {
            let rhs: Vec<C64> = (&ep * &current).iter().copied().collect();
            let diagonal = complex_diagonal(&self.r_diag, &self.l_diag, omega);
            let outcome = gmres(
                |x: &[C64]| {
                    if frequency == 0.0 {
                        // `L` drops out at DC: two real products, no pFFT.
                        let re: Vec<f64> = x.iter().map(|c| c.re).collect();
                        let im: Vec<f64> = x.iter().map(|c| c.im).collect();
                        let (re, im) = (self.apply_dc(&re), self.apply_dc(&im));
                        re.iter().zip(&im).map(|(&a, &b)| C64::new(a, b)).collect()
                    } else {
                        self.apply_ac(omega, x)
                    }
                },
                |v: &[C64]| v.iter().zip(&diagonal).map(|(&v, &d)| v / d).collect(),
                &rhs,
                &mut internal,
                &self.gmres,
            );
            if !outcome.converged {
                return Err(SolveError::NotConverged {
                    frequency,
                    port: 0,
                    iterations: outcome.iterations,
                    relative_residual: outcome.relative_residual,
                });
            }
            for x in &mut internal {
                *x = -*x;
            }
        }
        let internal_column = DMatrix::from_column_slice(internal_count, 1, &internal);
        let voltage = &pp * &current + ep.transpose() * &internal_column;
        if internal
            .iter()
            .chain(voltage.iter())
            .any(|e| !(e.re.is_finite() && e.im.is_finite()))
        {
            return Err(SolveError::Singular { frequency });
        }
        Ok(assemble_solution(
            &self.layout,
            &self.filaments,
            &self.resistances,
            &self.mesh,
            frequency,
            excitation,
            &internal,
            voltage.as_slice(),
        ))
    }

    /// `Z(ω)` at every one of `frequencies` (hertz), with provenance; the
    /// iterative counterpart of [`MeshSystem::sweep`](crate::MeshSystem::sweep).
    ///
    /// Frequencies are solved concurrently, as many at a time as fit a
    /// fixed memory budget; each is an independent deterministic
    /// computation, so the result does not depend on the number of threads.
    /// `Provenance::timing.inductance_s` reports the time spent building the
    /// pFFT operator.
    ///
    /// # Errors
    ///
    /// As [`impedance`](Self::impedance); the first failing frequency in list
    /// order is reported.
    pub fn sweep(&self, frequencies: &[f64]) -> Result<SweepResult, SolveError> {
        let stats = self.operator.stats();
        let fft_points: usize = stats.fft_dims.iter().product();
        // Krylov basis and work vectors, branch vectors, FFT buffers.
        let bytes_per_solve = (self.gmres.restart + 4) * self.counts.internal_meshes * 16
            + 8 * self.counts.filaments * 8
            + 4 * fft_points * 16;
        sweep_impedances(
            frequencies,
            &self.ports,
            self.counts,
            bytes_per_solve,
            self.operator_seconds,
            self.assembly_seconds,
            |frequency| self.impedance(frequency),
        )
    }

    /// Branch currents `M_eᵀ·x` of internal loop currents `x`.
    fn scatter<T: ComplexField<RealField = f64> + Copy>(&self, x: &[T]) -> Vec<T> {
        let mut branch = vec![T::zero(); self.filaments.len()];
        for (i, &xi) in x.iter().enumerate() {
            for &(b, sign) in self.mesh.row(i) {
                branch[b] += xi.scale(sign);
            }
        }
        branch
    }

    /// `M_e R M_eᵀ · x`.
    fn apply_dc(&self, x: &[f64]) -> Vec<f64> {
        let mut drop = self.scatter(x);
        for (d, r) in drop.iter_mut().zip(&self.resistances) {
            *d *= r;
        }
        gather(&self.mesh, x.len(), &drop)
    }

    /// `M_e (R + jωL) M_eᵀ · x`, with `L` the pFFT operator.
    fn apply_ac(&self, omega: f64, x: &[C64]) -> Vec<C64> {
        let current = self.scatter(x);
        let re: Vec<f64> = current.iter().map(|c| c.re).collect();
        let im: Vec<f64> = current.iter().map(|c| c.im).collect();
        let (flux_re, flux_im) =
            rayon::join(|| self.operator.apply(&re), || self.operator.apply(&im));
        let drop: Vec<C64> = (0..current.len())
            .map(|b| {
                let r = self.resistances[b];
                C64::new(
                    r * re[b] - omega * flux_im[b],
                    r * im[b] + omega * flux_re[b],
                )
            })
            .collect();
        gather(&self.mesh, x.len(), &drop)
    }
}

/// `pp − epᵀ · ee⁻¹ · ep` with `ee⁻¹` applied by one Jacobi-preconditioned
/// GMRES solve per column of `ep`. The transpose is plain, not conjugated:
/// the mesh impedance matrix is complex symmetric.
fn schur_complement_gmres<T>(
    ep: &DMatrix<T>,
    pp: &DMatrix<T>,
    apply_ee: impl Fn(&[T]) -> Vec<T>,
    diagonal: &[T],
    params: &GmresParams,
    frequency: f64,
) -> Result<(DMatrix<T>, Vec<GmresOutcome>), SolveError>
where
    T: ComplexField<RealField = f64> + Copy,
{
    let internal = ep.nrows();
    if internal == 0 {
        return Ok((pp.clone(), Vec::new()));
    }
    let mut solved = DMatrix::zeros(internal, ep.ncols());
    let mut outcomes = Vec::with_capacity(ep.ncols());
    for port in 0..ep.ncols() {
        let rhs: Vec<T> = ep.column(port).iter().copied().collect();
        let mut x = vec![T::zero(); internal];
        let outcome = gmres(
            &apply_ee,
            |v: &[T]| v.iter().zip(diagonal).map(|(&v, &d)| v / d).collect(),
            &rhs,
            &mut x,
            params,
        );
        if !outcome.converged {
            return Err(SolveError::NotConverged {
                frequency,
                port,
                iterations: outcome.iterations,
                relative_residual: outcome.relative_residual,
            });
        }
        solved.column_mut(port).copy_from_slice(&x);
        outcomes.push(outcome);
    }
    Ok((pp - ep.transpose() * solved, outcomes))
}

/// `M_{0..loops} · v` for a branch vector `v`: the first `loops` loops.
fn gather<T: ComplexField<RealField = f64> + Copy>(
    mesh: &MeshMatrix,
    loops: usize,
    v: &[T],
) -> Vec<T> {
    (0..loops)
        .into_par_iter()
        .with_min_len(256)
        .map(|i| {
            mesh.row(i)
                .iter()
                .fold(T::zero(), |sum, &(b, sign)| sum + v[b].scale(sign))
        })
        .collect()
}

/// `M · v` over every loop, internal and port.
fn gather_all(mesh: &MeshMatrix, v: &[f64]) -> Vec<f64> {
    gather(mesh, mesh.loop_count(), v)
}

/// `Σₐ Σ_b sₐ s_b L_ab` over the branches of one loop — the loop's
/// diagonal entry of `M L Mᵀ` — or, for loops longer than
/// [`EXACT_DIAGONAL_MAX_LOOP_LENGTH`], the sum of the self terms alone.
fn loop_inductance(filaments: &[Filament], row: &[(usize, f64)]) -> Result<f64, KernelError> {
    let own: f64 = row
        .iter()
        .map(|&(branch, _)| self_inductance(&filaments[branch]))
        .sum();
    if row.len() > EXACT_DIAGONAL_MAX_LOOP_LENGTH {
        return Ok(own);
    }
    let mut mutual = 0.0;
    for (k, &(a, sign_a)) in row.iter().enumerate() {
        for &(b, sign_b) in &row[k + 1..] {
            let m = mutual_inductance(&filaments[a], &filaments[b]).map_err(|e| e.at(a, b))?;
            mutual += sign_a * sign_b * m;
        }
    }
    Ok(own + 2.0 * mutual)
}
