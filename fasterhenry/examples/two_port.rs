//! Two coupled conductors on the dense solver path, end to end.
//!
//! Builds a geometry of two parallel copper bars in code, puts a port across
//! each one, and sweeps the 2 × 2 port impedance matrix `Z(ω)` from DC to
//! 1 GHz with the one-shot `solve` front end. The bars share no node, so the
//! off-diagonal terms of `Z` are pure mutual coupling — the thing a
//! two-conductor extraction exists to produce.
//!
//! Run it with:
//!
//! ```text
//! cargo run --example two_port
//! ```
//!
//! A debug build is fast enough here: 64 filaments solve in well under a
//! second. The sibling `iterative` example shows the matrix-free path for
//! problems too large for dense LU.
//!
//! The printed DC resistance is checked against the closed form
//! `R = l / (σ·A)` so a reader can see at a glance that the numbers mean
//! something. The geometry is authored here from dimensions, not read from any
//! deck (see `CONTRIBUTING.md`).

use std::error::Error;

use nalgebra::DMatrix;

use fasterhenry::{solve, Discretization, Geometry, Node, Port, SegmentDef, Subdivision};

/// Conductivity of copper, in S/m.
const COPPER: f64 = 5.8e7;
/// Length of each bar along x, in metres.
const LENGTH: f64 = 2e-3;
/// Width of each bar — the in-plane cross-section dimension — in metres.
const WIDTH: f64 = 100e-6;
/// Thickness of each bar, in metres.
const THICKNESS: f64 = 20e-6;
/// Centre-to-centre separation of the two bars along y, in metres.
const SEPARATION: f64 = 200e-6;
/// Frequencies to sweep, in hertz: DC, a low frequency, and 1 GHz.
const FREQUENCIES: [f64; 3] = [0.0, 1e6, 1e9];

fn main() -> Result<(), Box<dyn Error>> {
    // Two bars running along x, offset from each other along y. Each is a
    // single segment between its own pair of nodes, so the two conductors are
    // separate connected components joined by nothing but their mutual
    // inductance. A port spans each bar's own two nodes, which is what makes
    // that a valid input rather than an unconnected-port error.
    let mut geometry = Geometry::new();
    let mut ports = Vec::new();
    for (index, y) in [0.0, SEPARATION].into_iter().enumerate() {
        let a = geometry.add_node(Node::new(0.0, y, 0.0))?;
        let b = geometry.add_node(Node::new(LENGTH, y, 0.0))?;
        geometry.add_segment(SegmentDef::new(a, b, WIDTH, THICKNESS, COPPER))?;
        ports.push(Port::new(a, b).named(format!("bar{index}")));
    }

    // An 8 × 4 filament grid across each cross-section resolves the current
    // crowding at the top of the sweep: 32 filaments per bar, 64 in total.
    // `Discretization::uniform(8, 4)` is the same thing spelled shorter.
    let discretization = Discretization::Uniform(Subdivision::new(8, 4));

    let result = solve(&geometry, &ports, &discretization, &FREQUENCIES)?;

    let counts = result.provenance.counts;
    println!(
        "two coupled copper bars: {:.3} mm long, {:.1} × {:.1} µm in cross-section,",
        LENGTH * 1e3,
        WIDTH * 1e6,
        THICKNESS * 1e6
    );
    println!("{:.1} µm apart, σ = {COPPER:e} S/m", SEPARATION * 1e6);
    println!(
        "{} nodes, {} segments → {} filaments, {} meshes, {} ports\n",
        counts.nodes, counts.segments, counts.filaments, counts.meshes, counts.ports
    );

    // The DC diagonal is the bar's own resistance and nothing else: at ω = 0
    // there is no inductive term, and two conductors sharing no node share no
    // resistive drop, so the off-diagonal entries are zero.
    let r_dc_closed_form = LENGTH / (COPPER * WIDTH * THICKNESS);
    let r_dc = result.resistance(0)[(0, 0)];
    println!("DC check: R = l / (σ·A) = {r_dc_closed_form:.9} Ω");
    println!(
        "          solved R[(0, 0)] = {r_dc:.9} Ω  (relative error {:.2e})\n",
        ((r_dc - r_dc_closed_form) / r_dc_closed_form).abs()
    );

    for (index, &frequency) in FREQUENCIES.iter().enumerate() {
        if frequency == 0.0 {
            println!("f = DC");
        } else {
            println!("f = {frequency:e} Hz");
        }

        print_matrix("R = Re Z", &result.resistance(index), 1e3, "mΩ");
        match result.inductance(index) {
            Some(inductance) => print_matrix("L = Im Z / ω", &inductance, 1e12, "pH"),
            // Inductance is `Im Z / ω`, which is undefined at ω = 0; the
            // solver returns `None` there rather than a division by zero.
            None => println!("  L = Im Z / ω  undefined at DC"),
        }
        println!();
    }

    // The mutual terms are the point of a two-conductor run: a non-zero
    // L[(0, 1)] is one bar's flux linking the other.
    let mutual_ph = result
        .inductance(FREQUENCIES.len() - 1)
        .expect("defined at f > 0")[(0, 1)]
        * 1e12;
    println!("mutual inductance at 1 GHz: {mutual_ph:.3} pH");

    Ok(())
}

/// Prints `matrix` row by row under `label`, every entry multiplied by
/// `scale` and reported in `unit`.
fn print_matrix(label: &str, matrix: &DMatrix<f64>, scale: f64, unit: &str) {
    println!("  {label}  [{unit}]");
    for row in matrix.row_iter() {
        print!("   ");
        for value in row.iter() {
            print!(" {:>14.6}", value * scale);
        }
        println!();
    }
}
