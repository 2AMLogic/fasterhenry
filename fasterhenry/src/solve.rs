//! Assembly of the PEEC mesh system and its solution for the port impedance
//! matrix `Z(ω)`.
//!
//! # Formulation
//!
//! Following Kamon, Tsuk & White (IEEE T-MTT 1994, §II.B): with one branch per
//! filament, the branch voltages and currents obey `v = Z_b · I`, where
//!
//! ```text
//! Z_b = R + jωL,   R = diag(lₖ / (σₖ·Aₖ)),   L = partial-inductance matrix.
//! ```
//!
//! Writing the branch currents in terms of loop currents, `I = Mᵀ·I_m` with
//! the loop basis `M` of [`crate::mesh`], Kirchhoff's voltage law around every
//! loop gives the mesh system
//!
//! ```text
//! (M · Z_b · Mᵀ) · I_m = V_s ,
//! ```
//!
//! where `V_s` is zero for the internal loops and the port voltage for each
//! port loop. Partitioning the loops into internal (`e`) and port (`p`),
//!
//! ```text
//! ⎡ Z_ee  Z_ep ⎤ ⎡ I_e ⎤   ⎡ 0   ⎤
//! ⎣ Z_pe  Z_pp ⎦ ⎣ I_p ⎦ = ⎣ V_p ⎦      ⇒      V_p = Z(ω) · I_p ,
//!
//! Z(ω) = Z_pp − Z_pe · Z_ee⁻¹ · Z_ep .
//! ```
//!
//! That Schur complement *is* the port impedance matrix, so one dense LU
//! factorization of `Z_ee` ([`crate::dense::lu_solve`]: partial pivoting,
//! blocked and parallel) and one solve with a right-hand side per port is
//! all a frequency costs. It is algebraically identical to exciting one port
//! at a time and inverting the resulting admittance matrix, without the
//! second inversion. When there are no internal loops — every segment a
//! single filament, no closed rings — `Z(ω) = Z_pp` and nothing is solved.
//!
//! `Z_ee` is never singular for a valid geometry: `R` is positive definite and
//! `L` positive semidefinite, so the Hermitian part of `M_e (R + jωL) M_eᵀ` is
//! positive definite for independent loops, at every `ω ≥ 0`. In particular a
//! floating conductor without a port is harmless — it only adds internal
//! loops, in which eddy currents are induced.
//!
//! # Frequency independence and the DC limit
//!
//! `M R Mᵀ` and `M L Mᵀ` are real, frequency independent, and assembled once
//! by [`MeshSystem::assemble`] (the `O(b²)` kernel evaluations dominate);
//! each frequency then only forms `R_m + jωL_m` and factorizes it. At exactly
//! `f = 0` the system is solved in real arithmetic from `M R Mᵀ` alone, so the
//! result is the resistive network's, with an imaginary part of exactly zero
//! and no division by `ω` anywhere.
//!
//! # Units and conventions
//!
//! SI: geometry in metres, conductivity in S/m, frequencies in **hertz**
//! (`ω = 2πf`), impedances in ohms. Port orientation is documented in
//! [`crate::mesh`]; `Z` is symmetric (reciprocity) up to rounding.

use std::time::Instant;

use nalgebra::{ComplexField, DMatrix};
use num_complex::Complex;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::dense::lu_solve;
use crate::filament::{discretize, DiscretizeError, Filament};
use crate::geometry::Geometry;
use crate::inductance::{partial_inductance_matrix, KernelError};
use crate::mesh::{MeshError, MeshMatrix, Port};
use crate::result::{Counts, Provenance, SweepResult, Timing};

/// Working memory the frequency sweep may spend on concurrent factorizations
/// before it reduces the number of frequencies solved at once.
const SWEEP_MEMORY_BUDGET_BYTES: usize = 2 << 30;

/// Number of filaments across a segment's width (`nw`) and height (`nh`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Subdivision {
    /// Filaments across the width.
    pub nw: usize,
    /// Filaments across the height.
    pub nh: usize,
}

impl Subdivision {
    /// `nw` filaments across the width and `nh` across the height.
    pub const fn new(nw: usize, nh: usize) -> Self {
        Self { nw, nh }
    }

    /// One filament per segment: uniform current density, no skin effect.
    pub const SINGLE: Self = Self::new(1, 1);
}

/// How the segments of a geometry are cut into filaments.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Discretization {
    /// The same subdivision for every segment.
    Uniform(Subdivision),
    /// One subdivision per segment, in segment order.
    PerSegment(Vec<Subdivision>),
}

impl Discretization {
    /// The same `nw × nh` subdivision for every segment.
    pub const fn uniform(nw: usize, nh: usize) -> Self {
        Self::Uniform(Subdivision::new(nw, nh))
    }

    fn resolve(&self, segment_count: usize) -> Result<Vec<Subdivision>, SolveError> {
        match self {
            Self::Uniform(subdivision) => Ok(vec![*subdivision; segment_count]),
            Self::PerSegment(list) if list.len() == segment_count => Ok(list.clone()),
            Self::PerSegment(list) => Err(SolveError::Mesh(MeshError::SegmentCountMismatch {
                expected: segment_count,
                got: list.len(),
            })),
        }
    }
}

/// Why an impedance extraction failed.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum SolveError {
    /// A segment could not be cut into filaments.
    #[error("segment {segment}: {source}")]
    Discretize {
        /// Index of the offending segment.
        segment: usize,
        /// The underlying error.
        source: DiscretizeError,
    },
    /// A partial inductance could not be evaluated; the indices in the error
    /// are filament (branch) indices.
    #[error(transparent)]
    Kernel(#[from] KernelError),
    /// The ports or the discretization do not fit the geometry.
    #[error(transparent)]
    Mesh(#[from] MeshError),
    /// A frequency is negative or not finite.
    #[error("frequency {index} is {value} Hz; frequencies must be finite and non-negative")]
    InvalidFrequency {
        /// Position in the frequency list.
        index: usize,
        /// The offending value.
        value: f64,
    },
    /// The mesh system could not be solved at this frequency: the
    /// factorization hit a zero pivot or produced a non-finite impedance.
    /// A valid geometry never does; see the [module documentation](self).
    #[error("the mesh system is singular or ill-conditioned at {frequency} Hz")]
    Singular {
        /// The frequency, in hertz.
        frequency: f64,
    },
}

/// The assembled, frequency-independent mesh system of a geometry and its
/// ports. Assemble once, then evaluate [`impedance`](Self::impedance) or
/// [`sweep`](Self::sweep) at as many frequencies as needed.
#[derive(Clone, Debug)]
pub struct MeshSystem {
    ports: Vec<Port>,
    filaments: Vec<Filament>,
    resistances: Vec<f64>,
    mesh: MeshMatrix,
    /// `M R Mᵀ` and `M L Mᵀ`, partitioned into internal (`e`) and port (`p`)
    /// loops. The `pe` blocks are the transposes of the `ep` blocks.
    r_ee: DMatrix<f64>,
    r_ep: DMatrix<f64>,
    r_pp: DMatrix<f64>,
    l_ee: DMatrix<f64>,
    l_ep: DMatrix<f64>,
    l_pp: DMatrix<f64>,
    counts: Counts,
    inductance_seconds: f64,
    assembly_seconds: f64,
}

impl MeshSystem {
    /// Discretizes `geometry`, evaluates every filament's resistance
    /// `l / (σ·A)` and the partial-inductance matrix, builds the loop basis
    /// for `ports`, and reduces both to mesh quantities.
    ///
    /// # Errors
    ///
    /// * [`SolveError::Discretize`] for a zero `nw` or `nh`;
    /// * [`SolveError::Mesh`] for ports that do not fit the geometry (none
    ///   given, unknown or repeated node, terminals on different conductors)
    ///   or a per-segment discretization of the wrong length;
    /// * [`SolveError::Kernel`] from the inductance kernels, which can only
    ///   happen for overlapping parallel segments whose cross-sections are
    ///   rotated against each other.
    pub fn assemble(
        geometry: &Geometry,
        ports: &[Port],
        discretization: &Discretization,
    ) -> Result<Self, SolveError> {
        let start = Instant::now();
        let subdivisions = discretization.resolve(geometry.segment_count())?;

        let mut filaments = Vec::new();
        let mut per_segment = Vec::with_capacity(subdivisions.len());
        for (index, (segment, subdivision)) in geometry.segments().zip(&subdivisions).enumerate() {
            let bundle =
                discretize(&segment, subdivision.nw, subdivision.nh).map_err(|source| {
                    SolveError::Discretize {
                        segment: index,
                        source,
                    }
                })?;
            per_segment.push(bundle.len());
            filaments.extend(bundle);
        }
        // Validate the ports before paying for the inductance matrix.
        let mesh = MeshMatrix::build(geometry, &per_segment, ports)?;

        let resistances: Vec<f64> = filaments.iter().map(filament_resistance).collect();

        let inductance_start = Instant::now();
        let inductance = partial_inductance_matrix(&filaments)?;
        let inductance_seconds = inductance_start.elapsed().as_secs_f64();

        let branches = filaments.len();
        let r_mesh = congruence(&mesh, branches, |branch, sign, row| {
            row[branch] += sign * resistances[branch];
        });
        let l_mesh = congruence(&mesh, branches, |branch, sign, row| {
            // `L` is symmetric: its column `branch` is its row `branch`.
            for (slot, value) in row.iter_mut().zip(inductance.column(branch).iter()) {
                *slot += sign * value;
            }
        });
        drop(inductance);

        let (internal, port_count) = (mesh.internal_count(), mesh.port_count());
        let block = |m: &DMatrix<f64>, r0, c0, nr, nc| m.view((r0, c0), (nr, nc)).into_owned();
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
            r_ee: block(&r_mesh, 0, 0, internal, internal),
            r_ep: block(&r_mesh, 0, internal, internal, port_count),
            r_pp: block(&r_mesh, internal, internal, port_count, port_count),
            l_ee: block(&l_mesh, 0, 0, internal, internal),
            l_ep: block(&l_mesh, 0, internal, internal, port_count),
            l_pp: block(&l_mesh, internal, internal, port_count, port_count),
            mesh,
            counts,
            inductance_seconds,
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

    /// DC resistance `l / (σ·A)` of every filament, in ohms, in the order of
    /// [`filaments`](Self::filaments).
    pub fn filament_resistances(&self) -> &[f64] {
        &self.resistances
    }

    /// The loop basis.
    pub fn mesh(&self) -> &MeshMatrix {
        &self.mesh
    }

    /// Problem size: nodes, segments, filaments, meshes, ports.
    pub fn counts(&self) -> Counts {
        self.counts
    }

    /// The port impedance matrix `Z(ω)`, in ohms, at `frequency` hertz
    /// (`ω = 2πf`): entry `(i, j)` is the voltage across port `i` per unit
    /// current into port `j` with every other port open.
    ///
    /// # Errors
    ///
    /// [`SolveError::InvalidFrequency`] for a negative or non-finite
    /// frequency (reported with index 0), [`SolveError::Singular`] if the
    /// factorization breaks down.
    pub fn impedance(&self, frequency: f64) -> Result<DMatrix<Complex<f64>>, SolveError> {
        check_frequency(0, frequency)?;
        let singular = SolveError::Singular { frequency };
        let z = if frequency == 0.0 {
            schur_complement(self.r_ee.clone(), &self.r_ep, self.r_pp.clone())
                .ok_or(singular.clone())?
                .map(|re| Complex::new(re, 0.0))
        } else {
            let omega = std::f64::consts::TAU * frequency;
            let complex = |r: &DMatrix<f64>, l: &DMatrix<f64>| {
                r.zip_map(l, |r, l| Complex::new(r, omega * l))
            };
            schur_complement(
                complex(&self.r_ee, &self.l_ee),
                &complex(&self.r_ep, &self.l_ep),
                complex(&self.r_pp, &self.l_pp),
            )
            .ok_or(singular.clone())?
        };
        if z.iter()
            .all(|entry| entry.re.is_finite() && entry.im.is_finite())
        {
            Ok(z)
        } else {
            Err(singular)
        }
    }

    /// `Z(ω)` at every one of `frequencies` (hertz), with provenance.
    ///
    /// Frequencies are solved concurrently on the `rayon` thread pool, as many
    /// at a time as fit a fixed memory budget; every frequency is an
    /// independent deterministic computation, so the impedances do not depend
    /// on the number of threads.
    ///
    /// # Errors
    ///
    /// As [`impedance`](Self::impedance); the first failing frequency in list
    /// order is reported.
    pub fn sweep(&self, frequencies: &[f64]) -> Result<SweepResult, SolveError> {
        let start = Instant::now();
        for (index, &frequency) in frequencies.iter().enumerate() {
            check_frequency(index, frequency)?;
        }
        let internal = self.counts.internal_meshes;
        // The matrix being factorized plus its right-hand sides and scratch.
        let bytes_per_solve = 2 * internal * (internal + self.counts.ports) * 16;
        let concurrent = (SWEEP_MEMORY_BUDGET_BYTES / bytes_per_solve.max(1)).max(1);

        let mut impedance = Vec::with_capacity(frequencies.len());
        for chunk in frequencies.chunks(concurrent) {
            let solved: Vec<_> = chunk.par_iter().map(|&f| self.impedance(f)).collect();
            for z in solved {
                impedance.push(z?);
            }
        }
        let solve_seconds = start.elapsed().as_secs_f64();
        Ok(SweepResult {
            schema_version: SweepResult::SCHEMA_VERSION,
            ports: self.ports.clone(),
            frequencies_hz: frequencies.to_vec(),
            impedance_ohm: impedance,
            provenance: Provenance {
                fasterhenry_version: crate::VERSION.to_owned(),
                counts: self.counts,
                timing: Some(Timing {
                    threads: rayon::current_num_threads(),
                    inductance_s: self.inductance_seconds,
                    assembly_s: self.assembly_seconds,
                    solve_s: solve_seconds,
                    total_s: self.assembly_seconds + solve_seconds,
                }),
            },
        })
    }
}

/// Extracts the port impedance matrix of `geometry` at every one of
/// `frequencies` (hertz): [`MeshSystem::assemble`] followed by
/// [`MeshSystem::sweep`].
///
/// # Errors
///
/// As those two.
///
/// # Example
///
/// ```
/// use fasterhenry::{solve, Discretization, Geometry, Node, Port, SegmentDef};
///
/// // A 10 mm × 1 mm × 35 µm copper trace, 5 × 2 filaments.
/// let mut geometry = Geometry::new();
/// let a = geometry.add_node(Node::new(0.0, 0.0, 0.0))?;
/// let b = geometry.add_node(Node::new(10e-3, 0.0, 0.0))?;
/// geometry.add_segment(SegmentDef::new(a, b, 1e-3, 35e-6, 5.8e7))?;
///
/// let result = solve(
///     &geometry,
///     &[Port::new(a, b)],
///     &Discretization::uniform(5, 2),
///     &[0.0, 1e6, 1e9],
/// )?;
///
/// // At DC the trace is its resistance l / (σ·w·h) …
/// let dc = result.impedance_ohm[0][(0, 0)];
/// assert!((dc.re - 10e-3 / (5.8e7 * 1e-3 * 35e-6)).abs() < 1e-12 * dc.re);
/// assert_eq!(dc.im, 0.0);
/// // … and the skin effect raises it with frequency.
/// assert!(result.impedance_ohm[2][(0, 0)].re > dc.re);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn solve(
    geometry: &Geometry,
    ports: &[Port],
    discretization: &Discretization,
    frequencies: &[f64],
) -> Result<SweepResult, SolveError> {
    MeshSystem::assemble(geometry, ports, discretization)?.sweep(frequencies)
}

/// DC resistance of a filament, `R = l / (σ·A)`, in ohms.
pub fn filament_resistance(filament: &Filament) -> f64 {
    filament.length() / (filament.sigma() * filament.area())
}

fn check_frequency(index: usize, value: f64) -> Result<(), SolveError> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(SolveError::InvalidFrequency { index, value })
    }
}

/// `pp − epᵀ · ee⁻¹ · ep` by LU factorization with partial pivoting; `None`
/// if `ee` is exactly singular. The transpose is plain, not conjugated: the
/// mesh impedance matrix is complex *symmetric*.
fn schur_complement<T: ComplexField + Copy + Send + Sync>(
    ee: DMatrix<T>,
    ep: &DMatrix<T>,
    pp: DMatrix<T>,
) -> Option<DMatrix<T>> {
    if ee.is_empty() {
        return Some(pp);
    }
    let solved = lu_solve(ee, ep)?;
    Some(pp - ep.transpose() * solved)
}

/// The congruence `M · B · Mᵀ` of a symmetric branch matrix `B` with the loop
/// basis. `accumulate(branch, sign, row)` must add `sign ·` (row `branch` of
/// `B`) to `row`.
///
/// Only the upper triangle is computed and then mirrored, so the result is
/// symmetric to the last bit.
fn congruence(
    mesh: &MeshMatrix,
    branches: usize,
    accumulate: impl Fn(usize, f64, &mut [f64]) + Sync,
) -> DMatrix<f64> {
    let loops = mesh.loop_count();
    if loops == 0 || branches == 0 {
        return DMatrix::zeros(loops, loops);
    }
    // Row `i` of `M · B`, then its products with rows `j ≥ i` of `M`.
    let mut upper = vec![0.0; loops * loops];
    upper.par_chunks_mut(loops).enumerate().for_each_init(
        || vec![0.0; branches],
        |product, (i, out)| {
            product.fill(0.0);
            for &(branch, sign) in mesh.row(i) {
                accumulate(branch, sign, product);
            }
            for (j, slot) in out.iter_mut().enumerate().skip(i) {
                *slot = mesh
                    .row(j)
                    .iter()
                    .map(|&(branch, sign)| sign * product[branch])
                    .sum();
            }
        },
    );
    DMatrix::from_fn(loops, loops, |i, j| upper[i.min(j) * loops + i.max(j)])
}
