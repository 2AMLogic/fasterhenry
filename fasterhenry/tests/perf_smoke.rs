//! Performance smoke test: a 2 000-filament problem, assembled and solved at
//! one frequency, within a wall-clock budget.
//!
//! The test is `#[ignore]`d because an unoptimized build is one to two orders
//! of magnitude too slow for it to mean anything. CI runs it explicitly in
//! release mode:
//!
//! ```text
//! cargo test --release -p fasterhenry --test perf_smoke -- --ignored --nocapture
//! ```
//!
//! The budget is 10 s (the project's acceptance criterion on a GitHub
//! `ubuntu-latest` runner) and can be overridden, in seconds, with the
//! `FASTERHENRY_PERF_BUDGET_S` environment variable on slower or busier hosts.

use std::time::Instant;

use fasterhenry::{solve, Discretization, Geometry, Node, Port, SegmentDef};

const COPPER: f64 = 5.8e7;

/// A planar serpentine: `bars` long traces along ±x at a fixed pitch, each
/// joined to the next by a short jog along +y, with a final jog as lead-out.
/// `2·bars` segments in one chain; the port spans the whole chain.
fn serpentine(bars: usize) -> (Geometry, Port) {
    let (length, pitch, width, thickness) = (2e-3, 60e-6, 20e-6, 10e-6);
    let mut geometry = Geometry::new();
    let mut previous = geometry.add_node(Node::new(0.0, 0.0, 0.0)).unwrap();
    let first = previous;
    for bar in 0..bars {
        let y = bar as f64 * pitch;
        // Even bars run towards +x, odd bars back towards −x.
        let x_far = if bar % 2 == 0 { length } else { 0.0 };
        for (x, y) in [(x_far, y), (x_far, y + pitch)] {
            let node = geometry.add_node(Node::new(x, y, 0.0)).unwrap();
            geometry
                .add_segment(SegmentDef::new(previous, node, width, thickness, COPPER))
                .unwrap();
            previous = node;
        }
    }
    (geometry, Port::new(first, previous))
}

#[test]
#[ignore = "release-mode performance smoke; see the module documentation"]
fn two_thousand_filaments_solve_within_budget() {
    let budget: f64 = std::env::var("FASTERHENRY_PERF_BUDGET_S")
        .ok()
        .map(|s| {
            s.parse()
                .expect("FASTERHENRY_PERF_BUDGET_S must be a number")
        })
        .unwrap_or(10.0);

    let (geometry, port) = serpentine(10);
    let start = Instant::now();
    let result = solve(
        &geometry,
        &[port],
        &Discretization::uniform(10, 10),
        &[1.0e9],
    )
    .expect("the serpentine solves");
    let elapsed = start.elapsed().as_secs_f64();

    let counts = result.provenance.counts;
    let timing = result.provenance.timing.expect("solve records timing");
    let z = result.impedance_ohm[0][(0, 0)];
    println!(
        "perf smoke: {} filaments, {} meshes ({} internal), {} threads: \
         inductance {:.3} s, assembly {:.3} s, solve {:.3} s, wall {:.3} s; \
         Z(1 GHz) = {:.6e} + j{:.6e} ohm",
        counts.filaments,
        counts.meshes,
        counts.internal_meshes,
        timing.threads,
        timing.inductance_s,
        timing.assembly_s,
        timing.solve_s,
        elapsed,
        z.re,
        z.im,
    );

    assert_eq!(counts.filaments, 2000);
    assert_eq!(counts.internal_meshes, 20 * 99);
    // Passive and inductive, and more resistive than at DC (skin/proximity).
    let dc: f64 = geometry
        .segments()
        .map(|s| s.length() / (s.sigma * s.area()))
        .sum();
    assert!(z.re > dc && z.im > 0.0, "Z = {z}, R_dc = {dc}");
    assert!(
        elapsed < budget,
        "2000 filaments took {elapsed:.2} s, over the {budget} s budget"
    );
}

/// Times the blocked LU of [`fasterhenry::dense`] against `nalgebra`'s
/// unblocked LU on a complex symmetric system of the order the smoke test
/// factorizes, and checks that they agree. Informational: it documents why
/// the solver does not simply call `nalgebra`'s `LU`.
#[test]
#[ignore = "release-mode timing comparison; see the module documentation"]
fn blocked_lu_against_nalgebra_lu() {
    use nalgebra::DMatrix;
    use num_complex::Complex;

    let order = 1980;
    // A diagonally dominant complex symmetric matrix from a fixed recurrence.
    let mut state = 0x2545_F491_4F6C_DD1D_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64 - 0.5
    };
    let mut a = DMatrix::from_element(order, order, Complex::new(0.0, 0.0));
    for i in 0..order {
        for j in i..order {
            let value = Complex::new(next(), next());
            a[(i, j)] = value;
            a[(j, i)] = value;
        }
        a[(i, i)] += Complex::new(order as f64, 0.0);
    }
    let b = DMatrix::from_fn(order, 1, |_, _| Complex::new(next(), next()));

    let start = Instant::now();
    let blocked = fasterhenry::dense::lu_solve(a.clone(), &b).expect("non-singular");
    let blocked_s = start.elapsed().as_secs_f64();

    let start = Instant::now();
    let reference = a.clone().lu().solve(&b).expect("non-singular");
    let nalgebra_s = start.elapsed().as_secs_f64();

    let difference = (&blocked - &reference).norm() / reference.norm();
    println!(
        "LU of order {order}: blocked {blocked_s:.3} s, nalgebra {nalgebra_s:.3} s, \
         relative difference {difference:.2e}"
    );
    assert!(difference < 1e-12, "solutions differ by {difference:e}");
}
