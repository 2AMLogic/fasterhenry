//! fasterhenry — clean-room PEEC (partial-element equivalent circuit)
//! resistance and inductance extraction for 3-D conductor geometries.
//!
//! This crate implements the published method (Ruehli's PEEC; Kamon, Tsuk &
//! White, *FASTHENRY: a multipole-accelerated 3-D inductance extraction
//! program*, IEEE T-MTT 1994; Grover/Rosa closed forms for filament partial
//! inductances) from the literature. It contains no code derived from MIT's
//! FastHenry/FastCap, whose license permits only internal, noncommercial use
//! and forbids redistribution. See CONTRIBUTING.md before adding anything.
//!
//! # Pipeline
//!
//! A [`Geometry`] of nodes and segments is cut into [`Filament`]s
//! ([`discretize`], or [`discretize_graded`] for a skin-depth-aware grid
//! graded toward the conductor surfaces); their resistances and partial
//! inductances
//! ([`mod@inductance`]) form the branch impedance `R + jωL`; a loop basis
//! ([`mesh`]) turns that into the mesh system, which [`solve()`] reduces to
//! the impedance matrix `Z(ω)` seen at user-defined [`Port`]s over a list of
//! frequencies, returned as a JSON-serializable [`SweepResult`].
//!
//! [`coupling`] is the optional sparsity lever on that pipeline: name groups
//! of segments, declare which pairs of them are coupled, and the mutual
//! inductance of the rest is never computed. The default couples everything.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod coupling;
pub mod dense;
pub mod filament;
pub mod geometry;
pub mod inductance;
pub mod mesh;
pub mod plane;
pub mod result;
pub mod solve;

pub use coupling::{Coupling, CouplingError, TruncationWarning};
pub use filament::{discretize, discretize_graded, DiscretizeError, Filament};
pub use geometry::{
    Geometry, GeometryError, LocalBasis, Node, NodeId, Segment, SegmentDef, SegmentError,
};
pub use inductance::{
    mutual_batch, mutual_batch_detailed, mutual_inductance, partial_inductance_matrix,
    partial_inductance_matrix_detailed, partial_inductance_matrix_masked,
    partial_inductance_matrix_masked_with, self_inductance, KernelError, MutualBatch, PairMask,
    MU0,
};
pub use mesh::{MeshError, MeshMatrix, Port};
pub use result::{Counts, Provenance, SweepResult, Timing};
pub use solve::{
    filament_resistance, skin_depth, solve, Discretization, Grading, MeshSystem, SkinDepthGrading,
    SolveError, Subdivision,
};

/// Crate version, for CLI `--version` and JSON output provenance.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
