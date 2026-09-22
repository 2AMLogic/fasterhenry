//! Validation of coupling truncation (the `.couples` knob) at the solve
//! level.
//!
//! The fixture is two islands — nearly closed square loops, each with its own
//! port across a small gap — placed a controlled distance apart. Nothing here
//! is a hard-coded reference number: every expectation is either an identity
//! (a declared-coupled assembly equals the full one bit for bit), a bound the
//! test computes from the full assembly itself (the dropped mutual as a
//! fraction of the self impedance), or the sign of a comparison.

use std::f64::consts::TAU;

use fasterhenry::coupling::Coupling;
use fasterhenry::{
    Discretization, Geometry, MeshSystem, Node, NodeId, Port, SegmentDef, Subdivision,
};
use nalgebra::DMatrix;
use num_complex::Complex;

const COPPER: f64 = 5.8e7;
/// Side of a square-loop island.
const SIDE: f64 = 10e-3;
/// Trace width and thickness of an island's conductor.
const WIDTH: f64 = 0.4e-3;
const THICKNESS: f64 = 35e-6;
/// The extent of one island: the diagonal of its bounding box, which is what
/// [`Coupling::truncation_warnings`] measures separations against.
const EXTENT: f64 = std::f64::consts::SQRT_2 * SIDE;
/// High enough that the inductive part dominates the resistive one.
const FREQUENCY: f64 = 1e9;

/// Appends one island — a square loop of side [`SIDE`] whose left side is
/// broken by a gap — at `x = x0`, and returns its port. Four segments.
fn island(geometry: &mut Geometry, x0: f64) -> Port {
    let gap = SIDE / 20.0;
    let node = |geometry: &mut Geometry, x: f64, y: f64| {
        NodeId(geometry.add_node(Node::new(x0 + x, y, 0.0)).unwrap().0)
    };
    let start = node(geometry, 0.0, 0.0);
    let n1 = node(geometry, SIDE, 0.0);
    let n2 = node(geometry, SIDE, SIDE);
    let n3 = node(geometry, 0.0, SIDE);
    let end = node(geometry, 0.0, gap);
    for (a, b) in [(start, n1), (n1, n2), (n2, n3), (n3, end)] {
        geometry
            .add_segment(SegmentDef::new(a, b, WIDTH, THICKNESS, COPPER))
            .unwrap();
    }
    Port::new(start, end)
}

/// Two islands whose bounding boxes are `separation` apart along x, and the
/// group tags `"a"` and `"b"` that name them.
fn two_islands(separation: f64) -> (Geometry, Vec<Port>, Vec<String>) {
    let mut geometry = Geometry::new();
    let first = island(&mut geometry, 0.0);
    let second = island(&mut geometry, SIDE + separation);
    let groups = ["a", "a", "a", "a", "b", "b", "b", "b"]
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    (geometry, vec![first, second], groups)
}

/// Two filaments across each trace's width, so the islands carry internal
/// loops and the solve exercises the Schur complement rather than `Z_pp`
/// alone.
fn discretization() -> Discretization {
    Discretization::Uniform(Subdivision::new(2, 1))
}

fn impedance(
    geometry: &Geometry,
    ports: &[Port],
    coupling: &Coupling,
) -> (DMatrix<Complex<f64>>, Vec<String>) {
    let system =
        MeshSystem::assemble_with_coupling(geometry, ports, &discretization(), coupling).unwrap();
    let warnings = system
        .truncation_warnings()
        .iter()
        .map(ToString::to_string)
        .collect();
    (system.impedance(FREQUENCY).unwrap(), warnings)
}

fn relative_difference(a: &DMatrix<Complex<f64>>, b: &DMatrix<Complex<f64>>) -> f64 {
    let norm = |m: &DMatrix<Complex<f64>>| m.iter().map(|z| z.norm_sqr()).sum::<f64>().sqrt();
    norm(&(a - b)) / norm(a)
}

/// Islands more than ten times their own extent apart: dropping their mutual
/// inductance moves every port impedance by less than 0.1 %, and the term
/// dropped is itself below 0.1 % of the self impedance. No warning.
#[test]
fn distant_islands_survive_truncation_within_one_part_in_a_thousand() {
    let (geometry, ports, groups) = two_islands(11.0 * EXTENT - SIDE);
    let (full, _) = impedance(&geometry, &ports, &Coupling::all_pairs());
    let (truncated, warnings) = impedance(&geometry, &ports, &Coupling::truncated(groups));

    assert!(
        warnings.is_empty(),
        "islands 11x their extent apart must not warn: {warnings:?}"
    );

    // The whole matrix, including the deliberately dropped off-diagonal.
    let error = relative_difference(&full, &truncated);
    assert!(
        error < 1e-3,
        "truncation changed Z by {:.3e} relative, which is over 0.1 %",
        error
    );
    // And each port's own impedance, which is what an extraction reports.
    for i in 0..2 {
        let diagonal = (full[(i, i)] - truncated[(i, i)]).norm() / full[(i, i)].norm();
        assert!(
            diagonal < 1e-3,
            "port {i} self impedance moved by {diagonal:.3e} relative"
        );
    }

    // The mutual term itself is what truncation drops: exactly zero after,
    // and — the far-field claim — under 0.1 % of the self impedance before.
    assert_ne!(full[(0, 1)], Complex::new(0.0, 0.0));
    assert_eq!(truncated[(0, 1)], Complex::new(0.0, 0.0));
    assert_eq!(truncated[(1, 0)], Complex::new(0.0, 0.0));
    let mutual_fraction = full[(0, 1)].norm() / full[(0, 0)].norm();
    assert!(
        mutual_fraction < 1e-3,
        "the dropped mutual is {mutual_fraction:.3e} of the self impedance"
    );
}

/// Islands closer than their own extent: the truncation is reported, and the
/// term it drops is large enough to matter.
#[test]
fn near_islands_warn_and_the_dropped_term_is_significant() {
    let separation = 0.5 * EXTENT;
    let (geometry, ports, groups) = two_islands(separation);
    let (full, _) = impedance(&geometry, &ports, &Coupling::all_pairs());
    let (truncated, warnings) = impedance(&geometry, &ports, &Coupling::truncated(groups));

    assert_eq!(warnings.len(), 1, "expected one warning, got {warnings:?}");
    let warning = &warnings[0];
    for expected in ["'a'", "'b'", "truncat"] {
        assert!(
            warning.contains(expected),
            "warning should mention {expected}: {warning}"
        );
    }

    let mutual_fraction = full[(0, 1)].norm() / full[(0, 0)].norm();
    assert!(
        mutual_fraction > 1e-3,
        "at half an extent apart the mutual should matter, got {mutual_fraction:.3e}"
    );
    assert!(relative_difference(&full, &truncated) > 1e-3);
}

/// Declaring the two groups coupled reproduces the untruncated assembly bit
/// for bit, as does the default all-pairs coupling.
#[test]
fn declared_coupling_and_the_default_reproduce_the_full_assembly() {
    let (geometry, ports, groups) = two_islands(0.5 * EXTENT);
    let system = MeshSystem::assemble(&geometry, &ports, &discretization()).unwrap();
    let reference = system.impedance(FREQUENCY).unwrap();

    let (default, warnings) = impedance(&geometry, &ports, &Coupling::all_pairs());
    assert_eq!(default, reference, "the default must not truncate anything");
    assert!(warnings.is_empty());

    let declared = Coupling::truncated(groups).coupled("a", "b");
    let (coupled, warnings) = impedance(&geometry, &ports, &declared);
    assert_eq!(coupled, reference, "a declared pair must be kept exactly");
    assert!(
        warnings.is_empty(),
        "nothing was truncated, so nothing to warn about: {warnings:?}"
    );
}

/// Group tags are optional: untagged segments share the default group, which
/// couples to nothing it has not been coupled to.
#[test]
fn untagged_segments_share_the_default_group() {
    let (geometry, ports, mut groups) = two_islands(11.0 * EXTENT - SIDE);
    for group in groups.iter_mut().take(4) {
        group.clear();
    }
    let (full, _) = impedance(&geometry, &ports, &Coupling::all_pairs());
    let (truncated, _) = impedance(&geometry, &ports, &Coupling::truncated(groups));
    assert_eq!(truncated[(0, 1)], Complex::new(0.0, 0.0));
    assert!(relative_difference(&full, &truncated) < 1e-3);
}

/// A `.couples` name that no segment carries is a typo, not a silent no-op.
#[test]
fn coupling_an_undeclared_group_is_an_error() {
    let (geometry, ports, groups) = two_islands(EXTENT);
    let coupling = Coupling::truncated(groups).coupled("a", "typo");
    let error = MeshSystem::assemble(&geometry, &ports, &discretization())
        .map(drop)
        .and(
            MeshSystem::assemble_with_coupling(&geometry, &ports, &discretization(), &coupling)
                .map(drop),
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("typo"),
        "error should name the unknown group: {error}"
    );
}

/// The saving is in kernel evaluations that never happen, not in entries
/// zeroed afterwards: a counting kernel is called exactly once per kept pair
/// and never once for a truncated one.
#[test]
fn truncation_skips_the_kernel_rather_than_zeroing_afterwards() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use fasterhenry::{discretize, mutual_inductance, partial_inductance_matrix_masked_with};

    let (geometry, _, groups) = two_islands(EXTENT);
    let (nw, nh) = (2, 1);
    let mut filaments = Vec::new();
    let mut per_segment = Vec::new();
    for segment in geometry.segments() {
        let bundle = discretize(&segment, nw, nh).unwrap();
        per_segment.push(bundle.len());
        filaments.extend(bundle);
    }
    // Eight segments of two filaments each.
    assert_eq!(filaments.len(), 16);

    // A test double around the real kernel: same values, counted calls.
    let calls = |coupling: &Coupling| {
        let mask = coupling.pair_mask(&per_segment).unwrap();
        let counter = AtomicUsize::new(0);
        let matrix = partial_inductance_matrix_masked_with(&filaments, &mask, |a, b| {
            counter.fetch_add(1, Ordering::Relaxed);
            mutual_inductance(a, b)
        })
        .unwrap();
        (counter.load(Ordering::Relaxed), mask, matrix)
    };

    let (full_calls, full_mask, full) = calls(&Coupling::all_pairs());
    // n(n+1)/2 = 136 unordered pairs, self terms included.
    assert_eq!(full_mask.total_pairs(), 16 * 17 / 2);
    assert_eq!(full_calls, full_mask.total_pairs());
    assert_eq!(full_calls, full_mask.kept_pairs());

    let (truncated_calls, mask, truncated) = calls(&Coupling::truncated(groups.clone()));
    // Two groups of eight filaments: the 64 cross pairs are never evaluated.
    assert_eq!(mask.kept_pairs(), full_mask.total_pairs() - 8 * 8);
    assert_eq!(
        truncated_calls,
        mask.kept_pairs(),
        "the kernel must be called once per kept pair and never for a dropped one"
    );
    assert!(truncated_calls < full_calls);

    // Declaring the pair puts every evaluation back.
    let (declared_calls, _, declared) = calls(&Coupling::truncated(groups).coupled("a", "b"));
    assert_eq!(declared_calls, full_calls);
    assert_eq!(declared, full, "a declared pair is computed in full");

    // What survives truncation is bit-for-bit the full assembly; what does
    // not is exactly zero — and it was not zero already. (Perpendicular
    // filaments have no mutual inductance to begin with, so only the count of
    // genuinely dropped terms is asserted, not every entry.)
    let mut dropped_nonzero = 0usize;
    for i in 0..filaments.len() {
        for j in 0..filaments.len() {
            if mask.keeps(i, j) {
                assert_eq!(truncated[(i, j)], full[(i, j)], "kept entry ({i}, {j})");
            } else {
                assert_eq!(truncated[(i, j)], 0.0, "truncated entry ({i}, {j})");
                dropped_nonzero += usize::from(full[(i, j)] != 0.0);
            }
        }
    }
    assert!(
        dropped_nonzero > 0,
        "the fixture must actually drop some non-zero mutual terms"
    );
}

/// The inductive part of each island's impedance is a physical loop
/// inductance either way — a guard that the fixture itself is sane.
#[test]
fn each_island_is_a_physical_loop() {
    let (geometry, ports, groups) = two_islands(11.0 * EXTENT - SIDE);
    let (truncated, _) = impedance(&geometry, &ports, &Coupling::truncated(groups));
    for i in 0..2 {
        let inductance = truncated[(i, i)].im / (TAU * FREQUENCY);
        assert!(
            (10e-9..=80e-9).contains(&inductance),
            "island {i} loop inductance {inductance:.3e} H is not physical"
        );
        assert!(truncated[(i, i)].re > 0.0);
    }
}
