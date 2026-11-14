//! The matrix-free solver path: a pFFT operator driven by GMRES.
//!
//! Builds a serpentine copper trace in code and extracts its port impedance
//! at 1 GHz through `IterativeSystem` instead of the dense `MeshSystem`.
//! That path never assembles the partial-inductance matrix `L`: the
//! precorrected-FFT operator evaluates `L·x` in near-linear time and memory,
//! and GMRES solves against it. Driving the system directly — rather than
//! calling the one-shot `solve` front end, as the sibling `two_port` example
//! does — is what makes the per-port GMRES outcome visible.
//!
//! Run it with:
//!
//! ```text
//! cargo run --release --example iterative
//! ```
//!
//! **`--release` is not optional here.** The pFFT path is far more sensitive
//! to the build profile than the dense one: this run takes well under a second
//! in release and roughly 35× longer — about twenty seconds — in debug, and
//! plain `cargo run --example iterative` builds in debug. `Z` and the
//! iteration count are identical either way; only the wall clock changes.
//!
//! This example demonstrates the API, not a scaling win. At the 480 filaments
//! below, the dense path is still the faster one — about 8× faster on this same
//! geometry — because the pFFT operator's grid and near-field tables cost more
//! to build than the dense matrix does at this size. It is problems at
//! `DENSE_PATH_MAX_FILAMENTS` and beyond where the matrix-free path earns its
//! keep; the crossover measurements are in `docs/benchmarks.md`. The geometry
//! here is sized to finish in seconds, not to beat dense LU.
//!
//! The geometry is authored here from dimensions, not read from any deck (see
//! `CONTRIBUTING.md`).

use std::error::Error;
use std::time::Instant;

use nalgebra::DMatrix;
use num_complex::Complex;

use fasterhenry::{
    Discretization, Geometry, GeometryError, IterativeParams, IterativeSystem, Node, Port,
    SegmentDef, Subdivision,
};

/// Conductivity of copper, in S/m.
const COPPER: f64 = 5.8e7;
/// Hairpin turns in the serpentine; each contributes two segments.
const TURNS: usize = 40;
/// Length of one straight run along x, in metres.
const RUN: f64 = 1e-3;
/// Centre-to-centre spacing of adjacent runs along y, in metres.
const PITCH: f64 = 50e-6;
/// Trace width, in metres.
const WIDTH: f64 = 20e-6;
/// Trace thickness, in metres.
const THICKNESS: f64 = 10e-6;
/// The frequency to extract at, in hertz.
const FREQUENCY: f64 = 1e9;

fn main() -> Result<(), Box<dyn Error>> {
    let (geometry, port) = serpentine()?;
    let ports = [port];

    // A 3 × 2 grid across the trace's cross-section: six filaments per
    // segment, which puts this run in the few-hundred-filament range.
    let discretization = Discretization::Uniform(Subdivision::new(3, 2));
    let params = IterativeParams::default();

    // Assembly is frequency-independent — discretization, the loop basis, the
    // pFFT operator and the preconditioner — so one `IterativeSystem` can be
    // solved at as many frequencies as you like.
    let started = Instant::now();
    let system = IterativeSystem::assemble(&geometry, &ports, &discretization, &params)?;
    let assembled_s = started.elapsed().as_secs_f64();

    let counts = system.counts();
    println!(
        "serpentine of {TURNS} hairpin turns: {:.2} mm runs at {:.0} µm pitch,",
        RUN * 1e3,
        PITCH * 1e6
    );
    println!(
        "{:.0} × {:.0} µm cross-section, σ = {COPPER:e} S/m",
        WIDTH * 1e6,
        THICKNESS * 1e6
    );
    println!(
        "{} nodes, {} segments → {} filaments, {} meshes ({} internal), {} port",
        counts.nodes,
        counts.segments,
        counts.filaments,
        counts.meshes,
        counts.internal_meshes,
        counts.ports
    );

    // The operator's grid is what the pFFT method trades the dense matrix for;
    // `memory_bytes` is the whole point of the path at scale.
    let stats = system.operator_stats();
    println!(
        "pFFT operator: {:?} grid ({:?} padded) at h = {:.3} µm, {} near-field entries, {:.1} MiB",
        stats.grid_dims,
        stats.fft_dims,
        stats.grid_spacing * 1e6,
        stats.near_entries,
        stats.memory_bytes as f64 / (1024.0 * 1024.0)
    );
    println!("assembled in {assembled_s:.3} s\n");

    // One GMRES solve per port, against the matrix-free operator.
    let started = Instant::now();
    let solution = system.solve(FREQUENCY)?;
    let solved_s = started.elapsed().as_secs_f64();

    println!("f = {FREQUENCY:e} Hz, solved in {solved_s:.3} s");
    for (index, outcome) in solution.gmres.iter().enumerate() {
        println!(
            "  port {index}: {} GMRES iterations, relative residual {:.2e}, converged = {}",
            outcome.iterations, outcome.relative_residual, outcome.converged
        );
    }

    print_impedance(&solution.impedance, FREQUENCY);

    Ok(())
}

/// A serpentine trace of [`TURNS`] hairpin turns in the z = 0 plane, with one
/// port across the whole chain.
///
/// Each turn is two segments: a straight run along ±x, then a jog along +y
/// into the next run. Successive runs are antiparallel and a [`PITCH`] apart,
/// so the trace couples strongly to itself — the regime the pFFT operator is
/// built for.
fn serpentine() -> Result<(Geometry, Port), GeometryError> {
    let mut geometry = Geometry::new();
    let start = geometry.add_node(Node::new(0.0, 0.0, 0.0))?;
    let mut previous = start;

    for turn in 0..TURNS {
        let y = turn as f64 * PITCH;
        // Even turns run out towards +x, odd turns back towards x = 0.
        let x_far = if turn % 2 == 0 { RUN } else { 0.0 };
        for (x, y) in [(x_far, y), (x_far, y + PITCH)] {
            let node = geometry.add_node(Node::new(x, y, 0.0))?;
            geometry.add_segment(SegmentDef::new(previous, node, WIDTH, THICKNESS, COPPER))?;
            previous = node;
        }
    }

    // The chain's two free ends: current in at the start, out at the far end.
    Ok((geometry, Port::new(start, previous)))
}

/// Prints `Z` and the `R`/`L` it decomposes into at `frequency` hertz.
fn print_impedance(impedance: &DMatrix<Complex<f64>>, frequency: f64) {
    let omega = std::f64::consts::TAU * frequency;
    for (index, z) in impedance.iter().enumerate() {
        let row = index % impedance.nrows();
        let column = index / impedance.nrows();
        println!(
            "  Z[({row}, {column})] = {:.6} + {:.6}j Ω  →  R = {:.3} mΩ, L = {:.3} nH",
            z.re,
            z.im,
            z.re * 1e3,
            z.im / omega * 1e9
        );
    }
}
