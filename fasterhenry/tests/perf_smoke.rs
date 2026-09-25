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
//!
//! [`iterative_path_holds_its_scaling_headline`] is the same kind of guard
//! for the matrix-free path and the dense/iterative size threshold (issue
//! #44): a problem more than twice the size, solved by GMRES on the
//! precorrected-FFT operator, within `FASTERHENRY_PERF_ITERATIVE_BUDGET_S`
//! (default 120 s) and inside a fraction of the memory the dense path would
//! need for it. The full measurement it guards is
//! `fasterhenry/benches/scaling.rs`; the committed numbers are in
//! `docs/benchmarks.md`.

use std::time::Instant;

use fasterhenry::{
    solve, Discretization, Geometry, GmresParams, IterativeParams, IterativeSystem, Node, Port,
    SegmentDef,
};

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

/// A square meander of `bars` conductor runs along ±x, each cut into `bars`
/// collinear segments one pitch long and joined to the next by a jog along
/// +y; the port spans the whole chain. The footprint stays square and the
/// segments stay short as `bars` grows, which is what makes it a fair
/// scaling fixture for the pFFT operator — see the module documentation of
/// `fasterhenry/benches/scaling.rs`, whose fixture this mirrors.
fn meander(bars: usize) -> (Geometry, Port) {
    let (pitch, width, thickness) = (100e-6, 20e-6, 10e-6);
    let mut geometry = Geometry::new();
    let mut previous = geometry.add_node(Node::new(0.0, 0.0, 0.0)).unwrap();
    let first = previous;
    let add = |geometry: &mut Geometry, previous: &mut _, x: f64, y: f64| {
        let node = geometry.add_node(Node::new(x, y, 0.0)).unwrap();
        geometry
            .add_segment(SegmentDef::new(*previous, node, width, thickness, COPPER))
            .unwrap();
        *previous = node;
    };
    for bar in 0..bars {
        let y = bar as f64 * pitch;
        for step in 1..=bars {
            let along = if bar % 2 == 0 { step } else { bars - step };
            add(&mut geometry, &mut previous, along as f64 * pitch, y);
        }
        let x = if bar % 2 == 0 { bars } else { 0 } as f64 * pitch;
        add(&mut geometry, &mut previous, x, y + pitch);
    }
    (geometry, Port::new(first, previous))
}

/// The matrix-free path's side of the scaling claim (issue #44): at 4 760
/// filaments — past the ~3 000-filament wall-clock crossover measured in
/// `docs/benchmarks.md`, where the dense path already costs 2.3× the time
/// and 2.5× the memory — GMRES on the precorrected-FFT operator solves the
/// same problem within a wall-clock budget, in a fraction of the dense
/// path's working set, and in a handful of iterations.
///
/// The memory assertion is on the two paths' *model* working sets — what
/// each must hold by construction, which is machine-independent — so it
/// regresses only if the operator or the preconditioner stops being linear,
/// never because the runner was busy. Its factor is deliberately half the
/// measured one: this guards the headline, it does not re-derive it.
#[test]
#[ignore = "release-mode performance smoke; see the module documentation"]
fn iterative_path_holds_its_scaling_headline() {
    let budget: f64 = std::env::var("FASTERHENRY_PERF_ITERATIVE_BUDGET_S")
        .ok()
        .map(|s| {
            s.parse()
                .expect("FASTERHENRY_PERF_ITERATIVE_BUDGET_S must be a number")
        })
        .unwrap_or(120.0);

    let (geometry, port) = meander(34);
    let discretization = Discretization::uniform(2, 2);
    let start = Instant::now();
    let system = IterativeSystem::assemble(
        &geometry,
        &[port],
        &discretization,
        &IterativeParams::default(),
    )
    .expect("the meander assembles");
    let solution = system.solve(1e6).expect("the meander solves");
    let elapsed = start.elapsed().as_secs_f64();

    let counts = system.counts();
    let stats = system.operator_stats();
    let fft_points: usize = stats.fft_dims.iter().product();
    // The terms `IterativeSystem::sweep` budgets against: the operator, the
    // Krylov basis, and the transient FFT buffers of one product.
    let iterative_bytes = stats.memory_bytes
        + (GmresParams::default().restart + 1) * counts.internal_meshes * 16
        + 4 * fft_points * 16;
    // The dense path at the same size: `L`, plus the complex internal-loop
    // block and the copy `lu_solve` factorizes.
    let dense_bytes = 8 * counts.filaments * counts.filaments
        + 2 * 16 * counts.internal_meshes * counts.internal_meshes;
    let z = solution.impedance[(0, 0)];
    let iterations: Vec<usize> = solution.gmres.iter().map(|o| o.iterations).collect();
    println!(
        "iterative smoke: {} filaments, {} internal meshes, {} threads: wall {:.3} s, \
         GMRES {:?}, model memory {:.0} MB vs dense {:.0} MB ({:.1}×); \
         Z(1 MHz) = {:.6e} + j{:.6e} ohm",
        counts.filaments,
        counts.internal_meshes,
        rayon::current_num_threads(),
        elapsed,
        iterations,
        iterative_bytes as f64 / 1e6,
        dense_bytes as f64 / 1e6,
        dense_bytes as f64 / iterative_bytes as f64,
        z.re,
        z.im,
    );

    assert_eq!(counts.filaments, 4 * 34 * 35);
    assert_eq!(counts.internal_meshes, 3 * 34 * 35);
    assert!(z.re > 0.0 && z.im > 0.0, "Z = {z}");
    for (port, outcome) in solution.gmres.iter().enumerate() {
        assert!(outcome.converged, "port {port}: {outcome:?}");
        // The Jacobi preconditioner keeps this at single digits; a
        // regression there would show up as an iteration count, not a time.
        assert!(outcome.iterations < 50, "port {port}: {outcome:?}");
    }
    assert!(
        2 * iterative_bytes < dense_bytes,
        "the matrix-free path holds {iterative_bytes} B against the dense path's \
         {dense_bytes} B: short of the 2.5× the scaling table measured here, and \
         of the 2× this guards"
    );
    assert!(
        elapsed < budget,
        "{} filaments took {elapsed:.2} s on the iterative path, over the {budget} s budget",
        counts.filaments
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
