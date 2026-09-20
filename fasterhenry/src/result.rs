//! The result of a frequency sweep, and its JSON form.
//!
//! [`SweepResult`] implements `serde`'s `Serialize` and `Deserialize`; with
//! `serde_json` it reads and writes
//!
//! ```json
//! {
//!   "schema_version": 1,
//!   "ports": [{ "positive": 0, "negative": 1, "name": "trace" }],
//!   "frequencies_hz": [0.0, 1000000.0],
//!   "impedance_ohm": [
//!     [[{ "re": 0.0049, "im": 0.0 }]],
//!     [[{ "re": 0.0051, "im": 0.0437 }]]
//!   ],
//!   "provenance": {
//!     "fasterhenry_version": "0.0.1",
//!     "counts": { "nodes": 2, "segments": 1, "filaments": 10,
//!                 "meshes": 10, "internal_meshes": 9, "ports": 1 },
//!     "timing": { "threads": 8, "inductance_s": 0.0004, "assembly_s": 0.0005,
//!                 "solve_s": 0.0001, "total_s": 0.0006 }
//!   }
//! }
//! ```
//!
//! * `impedance_ohm[k][i][j]` is entry `(i, j)` of `Z` at `frequencies_hz[k]`:
//!   one matrix per frequency, each a list of rows, each complex number an
//!   explicit `{ "re", "im" }` object. Rows and columns follow `ports`.
//! * Everything outside `provenance.timing` is a deterministic function of
//!   the inputs and the crate version. `provenance.timing` (wall-clock seconds
//!   and the thread count) is not; [`SweepResult::without_timing`] drops it,
//!   after which two runs can be compared for equality or as JSON text.
//! * All numbers are finite, so the JSON never contains `null` in their place.
//!
//! `serde_json` prints the shortest decimal that reads back as the same
//! `f64`; reading it back *exactly* additionally needs `serde_json`'s
//! `float_roundtrip` feature, which this workspace enables.

use nalgebra::DMatrix;
use num_complex::Complex;
use serde::{Deserialize, Serialize};

use crate::mesh::Port;

/// Port impedance matrices over a list of frequencies, with provenance. See
/// the [module documentation](self) for the JSON form.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SweepResult {
    /// Version of this layout; [`SweepResult::SCHEMA_VERSION`] when written.
    pub schema_version: u32,
    /// The ports, in the order of the rows and columns of every matrix.
    pub ports: Vec<Port>,
    /// The frequencies, in hertz, in the order requested.
    pub frequencies_hz: Vec<f64>,
    /// `Z` at each frequency, in ohms: entry `(i, j)` is the voltage across
    /// port `i` per unit current into port `j`, every other port open.
    #[serde(with = "impedance_rows")]
    pub impedance_ohm: Vec<DMatrix<Complex<f64>>>,
    /// Where the numbers came from.
    pub provenance: Provenance,
}

impl SweepResult {
    /// The layout version written by this crate.
    pub const SCHEMA_VERSION: u32 = 1;

    /// A copy without the non-deterministic [`Provenance::timing`] block:
    /// what is left depends only on the inputs and the crate version.
    #[must_use]
    pub fn without_timing(&self) -> Self {
        let mut copy = self.clone();
        copy.provenance.timing = None;
        copy
    }

    /// The resistance matrix `Re Z`, in ohms, at frequency `index`.
    ///
    /// # Panics
    ///
    /// If `index` is out of range.
    pub fn resistance(&self, index: usize) -> DMatrix<f64> {
        self.impedance_ohm[index].map(|z| z.re)
    }

    /// The inductance matrix `Im Z / ω`, in henries, at frequency `index`;
    /// `None` at zero frequency, where it is undefined.
    ///
    /// # Panics
    ///
    /// If `index` is out of range.
    pub fn inductance(&self, index: usize) -> Option<DMatrix<f64>> {
        let omega = std::f64::consts::TAU * self.frequencies_hz[index];
        (omega != 0.0).then(|| self.impedance_ohm[index].map(|z| z.im / omega))
    }
}

/// Provenance of a [`SweepResult`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    /// [`crate::VERSION`] of the crate that computed the result.
    pub fasterhenry_version: String,
    /// Size of the problem that was solved.
    pub counts: Counts,
    /// Wall-clock timing; the only non-deterministic part of a result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing: Option<Timing>,
}

/// Size of a mesh system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Counts {
    /// Nodes in the geometry.
    pub nodes: usize,
    /// Segments in the geometry.
    pub segments: usize,
    /// Filaments, i.e. circuit branches, after discretization.
    pub filaments: usize,
    /// Independent loops, including one per port: the order of the mesh
    /// system.
    pub meshes: usize,
    /// Loops without a source: the order of the matrix factorized at each
    /// frequency.
    pub internal_meshes: usize,
    /// Ports: the order of `Z`.
    pub ports: usize,
}

/// Wall-clock timing of an extraction, in seconds.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Timing {
    /// Size of the `rayon` thread pool.
    pub threads: usize,
    /// Evaluating the partial-inductance matrix (part of `assembly_s`).
    pub inductance_s: f64,
    /// Discretization, resistances, inductances and mesh reduction.
    pub assembly_s: f64,
    /// Solving every frequency.
    pub solve_s: f64,
    /// `assembly_s + solve_s`.
    pub total_s: f64,
}

/// `Vec<DMatrix<Complex<f64>>>` as nested rows of `{ re, im }` objects.
mod impedance_rows {
    use super::{Complex, DMatrix};
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Serialize, Deserialize)]
    struct Entry {
        re: f64,
        im: f64,
    }

    pub fn serialize<S: Serializer>(
        matrices: &[DMatrix<Complex<f64>>],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let rows: Vec<Vec<Vec<Entry>>> = matrices
            .iter()
            .map(|m| {
                m.row_iter()
                    .map(|row| row.iter().map(|z| Entry { re: z.re, im: z.im }).collect())
                    .collect()
            })
            .collect();
        rows.serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<DMatrix<Complex<f64>>>, D::Error> {
        let matrices = Vec::<Vec<Vec<Entry>>>::deserialize(deserializer)?;
        matrices
            .into_iter()
            .map(|rows| {
                let order = rows.len();
                if rows.iter().any(|row| row.len() != order) {
                    return Err(D::Error::custom("impedance matrix is not square"));
                }
                Ok(DMatrix::from_fn(order, order, |i, j| {
                    Complex::new(rows[i][j].re, rows[i][j].im)
                }))
            })
            .collect()
    }
}
