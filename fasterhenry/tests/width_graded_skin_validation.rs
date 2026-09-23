//! Width-graded skin effect against an independent two-dimensional
//! cross-section reference (issue #32).
//!
//! The one-dimensional slab of `skin_effect_validation.rs` certifies grading
//! across the *thickness* only: it has no edges, so it cannot say whether a
//! grid graded across the *width* of a finite trace gets the edge current
//! crowding right. This file compares uniform and width-graded grids with
//! `tools/cross_section_reference.py`, an independent solve of the same
//! trace's cross-section (see `docs/validation.md` § "Width-graded
//! filaments" for its derivation, assumptions and convergence record).
//!
//! # Fixture
//!
//! A straight, isolated copper trace (`σ = 5.8e7 S/m`), 5 mm wide and
//! 200 µm thick (`w/t = 25`), driven by a port across its two ends, at four
//! frequencies with `t/δ = 0.3, 1, 3, 10` — from a skin depth three times the
//! thickness, where only the lateral redistribution across the width has
//! begun, to a current confined to skin layers and crowded into the edges.
//!
//! # What is compared
//!
//! The reference is per unit length of an infinitely long trace. The 3-D
//! solve is turned into the same quantity by differencing two lengths,
//! `z' = (Z(2l) − Z(l)) / l` at `l = 100 mm`: every filament of a straight
//! bar carries its current along its full length, so the finite length enters
//! only through the partial-inductance kernel, whose `l·ln l` part is common
//! to every filament pair (it shifts `X` without redistributing current) and
//! whose end correction is independent of `l` to first order — both cancel in
//! the difference, leaving an `O((w/l)²)` residual — measured at most 0.042 %
//! (on `ΔL'` at `t/δ = 0.3`) and asserted below 0.1 % by
//! `length_differencing_recovers_the_per_unit_length_impedance`.
//!
//! Two quantities per frequency, both independent of the 2-D log kernel's
//! arbitrary reference length:
//!
//! * `R'(f)`, the resistance per unit length;
//! * `ΔL'(f) = X'(f)/ω − L'(DC)`, the fall of the internal inductance from
//!   its uniform-current value — the frequency-dependent internal-inductance
//!   contribution. `L'(DC)` is read from the same grid at 1 Hz.
//!
//! # Running
//!
//! Without a reference the tests print `SKIPPED` and return before paying
//! for any solve. To run them:
//!
//! ```bash
//! python3 tools/cross_section_reference.py --out target/cross_section_reference.json
//! FASTERHENRY_CROSS_SECTION_REFERENCE=target/cross_section_reference.json \
//!   cargo test --release -p fasterhenry --test width_graded_skin_validation -- --nocapture
//! ```
//!
//! CI regenerates the reference and fails if any test skipped.

use std::f64::consts::TAU;

use num_complex::Complex;

use fasterhenry::{
    solve, Discretization, Geometry, MeshSystem, Node, Port, SegmentDef, SkinDepthGrading, MU0,
};

const WIDTH: f64 = 5e-3;
const THICKNESS: f64 = 200e-6;
const SIGMA: f64 = 5.8e7;
/// The shorter of the two differenced lengths; `l/w = 20`.
const LENGTH: f64 = 100e-3;
/// Where `L'(DC)` is read: δ = 66 mm, 13 widths — a uniform current.
const DC_ANCHOR_HZ: f64 = 1.0;
/// Printed by every test that completed its comparison; CI counts them.
const PASSED: &str = "CROSS-SECTION GATE PASSED";

// ---------------------------------------------------------------------------
// The reference
// ---------------------------------------------------------------------------

/// One reference quantity: the Richardson-extrapolated value and the
/// relative size of the reference's last refinement step.
#[derive(Clone, Copy, Debug)]
struct Value {
    value: f64,
    uncertainty: f64,
}

#[derive(Clone, Copy, Debug)]
struct Point {
    thickness_per_delta: f64,
    frequency: f64,
    r: Value,
    dl: Value,
}

/// The reference named by `FASTERHENRY_CROSS_SECTION_REFERENCE`, or `None`
/// (after printing `SKIPPED`) when it is not available.
fn reference() -> Option<Vec<Point>> {
    // `cargo test` runs from the crate directory, so a relative path is taken
    // from the workspace root, where the generator writes it by default.
    let path = std::env::var("FASTERHENRY_CROSS_SECTION_REFERENCE")
        .ok()
        .map(|p| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join(p)
        });
    let Some(path) = path.filter(|p| p.exists()) else {
        println!(
            "SKIPPED (cross-section reference absent): generate it with\n  \
             python3 tools/cross_section_reference.py --out target/cross_section_reference.json\n  \
             and set FASTERHENRY_CROSS_SECTION_REFERENCE to that path"
        );
        return None;
    };
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("readable")).expect("JSON");

    // The reference must describe this fixture, not a drifted copy of it.
    for (key, expected) in [
        ("width_m", WIDTH),
        ("thickness_m", THICKNESS),
        ("sigma_s_per_m", SIGMA),
    ] {
        let found = json[key].as_f64().expect(key);
        assert!(
            (found - expected).abs() <= 1e-12 * expected,
            "reference {key} = {found}, fixture has {expected}"
        );
    }
    let value = |v: &serde_json::Value| Value {
        value: v["value"].as_f64().expect("value"),
        uncertainty: v["uncertainty"].as_f64().expect("uncertainty"),
    };
    let points: Vec<Point> = json["points"]
        .as_array()
        .expect("points")
        .iter()
        .map(|p| Point {
            thickness_per_delta: p["thickness_per_delta"].as_f64().unwrap(),
            frequency: p["frequency_hz"].as_f64().unwrap(),
            r: value(&p["r_ohm_per_m"]),
            dl: value(&p["dl_h_per_m"]),
        })
        .collect();
    assert!(
        points.len() >= 3,
        "need at least three frequencies, weak to strong skin effect"
    );
    for point in &points {
        let delta = THICKNESS / point.thickness_per_delta;
        let expected = 1.0 / (std::f64::consts::PI * MU0 * SIGMA * delta * delta);
        assert!((point.frequency - expected).abs() <= 1e-9 * expected);
    }
    Some(points)
}

// ---------------------------------------------------------------------------
// The model side
// ---------------------------------------------------------------------------

fn trace(length: f64) -> (Geometry, Vec<Port>) {
    let mut geometry = Geometry::new();
    let a = geometry.add_node(Node::new(0.0, 0.0, 0.0)).expect("node");
    let b = geometry
        .add_node(Node::new(length, 0.0, 0.0))
        .expect("node");
    geometry
        .add_segment(SegmentDef::new(a, b, WIDTH, THICKNESS, SIGMA))
        .expect("segment");
    (geometry, vec![Port::new(a, b)])
}

fn filament_count(grid: &Discretization) -> usize {
    let (geometry, ports) = trace(LENGTH);
    MeshSystem::assemble(&geometry, &ports, grid)
        .expect("assembles")
        .counts()
        .filaments
}

/// `(R', ΔL')` per unit length at each frequency, from `Z(2l) − Z(l)` at
/// `l = LENGTH`.
fn per_unit_length(grid: &Discretization, frequencies: &[f64]) -> Vec<(f64, f64)> {
    per_unit_length_at(grid, frequencies, LENGTH)
}

/// `(R', ΔL')` per unit length at each frequency, from `Z(2l) − Z(l)`.
fn per_unit_length_at(grid: &Discretization, frequencies: &[f64], length: f64) -> Vec<(f64, f64)> {
    let mut all = vec![DC_ANCHOR_HZ];
    all.extend_from_slice(frequencies);
    let impedance = |length: f64| -> Vec<Complex<f64>> {
        let (geometry, ports) = trace(length);
        solve(&geometry, &ports, grid, &all)
            .expect("solvable")
            .impedance_ohm
            .iter()
            .map(|z| z[(0, 0)])
            .collect()
    };
    let (short, long) = (impedance(length), impedance(2.0 * length));
    let z: Vec<Complex<f64>> = long
        .iter()
        .zip(&short)
        .map(|(b, a)| (b - a) / length)
        .collect();
    let l_dc = z[0].im / (TAU * DC_ANCHOR_HZ);
    z[1..]
        .iter()
        .zip(frequencies)
        .map(|(z, &f)| (z.re, z.im / (TAU * f) - l_dc))
        .collect()
}

/// Signed relative errors `(R', ΔL')` of a grid against the reference.
fn errors(grid: &Discretization, points: &[Point]) -> Vec<(f64, f64)> {
    let frequencies: Vec<f64> = points.iter().map(|p| p.frequency).collect();
    per_unit_length(grid, &frequencies)
        .iter()
        .zip(points)
        .map(|(&(r, dl), p)| {
            (
                (r - p.r.value) / p.r.value.abs(),
                (dl - p.dl.value) / p.dl.value.abs(),
            )
        })
        .collect()
}

fn print_row(label: &str, errors: &[(f64, f64)]) {
    let cells: Vec<String> = errors
        .iter()
        .map(|(r, dl)| format!("R' {:+7.2} %  ΔL' {:+7.2} %", 100.0 * r, 100.0 * dl))
        .collect();
    println!("{label:>34} | {}", cells.join(" | "));
}

fn print_header(title: &str, points: &[Point]) {
    println!("\n--- {title} ---");
    let cells: Vec<String> = points
        .iter()
        .map(|p| format!("{:^26}", format!("t/δ = {}", p.thickness_per_delta)))
        .collect();
    println!("{:>34} | {}", "error vs 2-D reference", cells.join(" | "));
}

// ---------------------------------------------------------------------------
// The comparisons
// ---------------------------------------------------------------------------

/// The per-unit-length extraction itself: doubling the differenced length
/// (`l = 100 mm` → `200 mm`) must leave `R'` and `ΔL'` unchanged, otherwise
/// the finite trace length would leak into every comparison below. Needs no
/// reference, so it runs in the ordinary (debug) workspace test.
#[test]
fn length_differencing_recovers_the_per_unit_length_impedance() {
    let frequencies: Vec<f64> = [0.3, 1.0, 3.0, 10.0]
        .iter()
        .map(|p| {
            let delta = THICKNESS / p;
            1.0 / (std::f64::consts::PI * MU0 * SIGMA * delta * delta)
        })
        .collect();
    let grid = Discretization::graded(18, 5, 1.44);
    let base = per_unit_length_at(&grid, &frequencies, LENGTH);
    let doubled = per_unit_length_at(&grid, &frequencies, 2.0 * LENGTH);
    for ((a, b), f) in base.iter().zip(&doubled).zip(&frequencies) {
        let r = (b.0 - a.0).abs() / a.0.abs();
        let dl = (b.1 - a.1).abs() / a.1.abs();
        println!("f = {f:.4e} Hz: R' moves {r:.2e}, ΔL' moves {dl:.2e}");
        assert!(r < 1e-3, "R' depends on the differenced length: {r:.2e}");
        assert!(dl < 1e-3, "ΔL' depends on the differenced length: {dl:.2e}");
    }
}

/// The case issue #32 was filed on: at 36 × 10 filaments, grading the width
/// at 1.2 moves the high-frequency resistance well away from the width-uniform
/// grid. The reference settles which one is right: the *uniform* grid, whose
/// 139 µm edge cells cannot resolve the edge crowding, under-reads `R'` by
/// ~28 % at `t/δ = 10`; the graded one is within a few per cent.
#[test]
fn width_grading_is_the_accurate_grid_at_36_by_10() {
    let Some(points) = reference() else { return };
    let uniform = errors(&Discretization::uniform(36, 10), &points);
    let graded = errors(&Discretization::graded(36, 10, 1.2), &points);

    print_header("36 × 10: uniform vs graded 1.2", &points);
    print_row("uniform(36, 10)", &uniform);
    print_row("graded(36, 10, 1.2)", &graded);

    for ((u, g), p) in uniform.iter().zip(&graded).zip(&points) {
        let t_d = p.thickness_per_delta;
        assert!(g.0.abs() < 0.05, "graded R' at t/δ = {t_d}: {:+.4}", g.0);
        assert!(g.1.abs() < 0.015, "graded ΔL' at t/δ = {t_d}: {:+.4}", g.1);
        if t_d >= 1.0 {
            assert!(
                g.0.abs() < u.0.abs(),
                "at t/δ = {t_d} width grading ({:+.4}) is not closer than uniform ({:+.4})",
                g.0,
                u.0
            );
        }
    }
    println!("{PASSED}: 36 x 10");
}

/// Refining a width-graded grid must converge on the reference, and the
/// change from the previous refinement must bound the remaining error — the
/// convergence-based accuracy estimate a user can make without a reference.
///
/// The family keeps the grading *profile* fixed while halving every cell:
/// counts double and the ratio is square-rooted, so `ratio^(n/2)` — the
/// largest-to-smallest cell ratio — stays put.
#[test]
fn width_graded_refinement_converges_with_a_self_estimated_error() {
    let Some(points) = reference() else { return };
    let family = [
        (18, 5, 1.2f64.powi(2)),
        (36, 10, 1.2),
        (72, 20, 1.2f64.sqrt()),
    ];
    print_header("width-graded refinement family (1.2 at 36 × 10)", &points);
    let levels: Vec<Vec<(f64, f64)>> = family
        .iter()
        .map(|&(nw, nh, ratio)| {
            let grid = Discretization::graded(nw, nh, ratio);
            let e = errors(&grid, &points);
            print_row(&format!("graded({nw}, {nh}, {ratio:.4})"), &e);
            e
        })
        .collect();

    /// One compared quantity: its error accessor, the reference's own
    /// uncertainty for it, and the bound asserted on the finest grid.
    struct Quantity {
        name: &'static str,
        pick: fn(&(f64, f64)) -> f64,
        uncertainty: fn(&Point) -> f64,
        bound: f64,
    }
    let quantities = [
        Quantity {
            name: "R'",
            pick: |e| e.0,
            uncertainty: |p| p.r.uncertainty,
            bound: 0.015,
        },
        Quantity {
            name: "ΔL'",
            pick: |e| e.1,
            uncertainty: |p| p.dl.uncertainty,
            bound: 0.005,
        },
    ];
    for Quantity {
        name,
        pick,
        uncertainty,
        bound,
    } in quantities
    {
        for (k, point) in points.iter().enumerate() {
            let t_d = point.thickness_per_delta;
            // Below this the comparison is at the reference's own resolution
            // (plus the O((w/l)²) length residual).
            let floor = uncertainty(point) + 1e-3;
            let e: Vec<f64> = levels.iter().map(|level| pick(&level[k])).collect();

            // Convergence: each refinement shrinks an error that is resolvable.
            for pair in e.windows(2) {
                if pair[0].abs() > floor {
                    assert!(
                        pair[1].abs() < pair[0].abs(),
                        "{name} at t/δ = {t_d} does not converge: {e:?}"
                    );
                }
            }
            // The self-estimate: |error| ≤ |change from the coarser level|.
            for level in 1..e.len() {
                let change = (e[level] - e[level - 1]).abs();
                assert!(
                    e[level].abs() <= change + floor,
                    "{name} at t/δ = {t_d}, level {level}: error {:+.4} exceeds the \
                     refinement change {change:.4}",
                    e[level]
                );
            }
            let finest = *e.last().unwrap();
            assert!(
                finest.abs() < bound,
                "{name} at t/δ = {t_d}: finest grid is {finest:+.4} off (bound {bound})"
            );
        }
    }
    println!("{PASSED}: refinement family");
}

/// Steep width grading coarsens the middle of the trace. Where the skin depth
/// exceeds the thickness, the current redistributes smoothly across the whole
/// width and those coarse middle cells under-resolve it: `graded(36, 10, 1.5)`
/// puts 0.83 mm (w/6) cells there and reads `ΔL'` a few per cent high at
/// `t/δ = 0.3`, although it is within a fraction of a per cent from
/// `t/δ = 1` up and on `R'` everywhere. This is the documented regime limit
/// (`docs/validation.md`), asserted here so a change in it is noticed.
#[test]
fn steep_width_grading_is_limited_at_weak_skin_effect() {
    let Some(points) = reference() else { return };
    let grid = Discretization::graded(36, 10, 1.5);
    let e = errors(&grid, &points);
    print_header("steep grading: graded(36, 10, 1.5)", &points);
    print_row("graded(36, 10, 1.5)", &e);
    for (&(r, dl), p) in e.iter().zip(&points) {
        let t_d = p.thickness_per_delta;
        assert!(r.abs() < 0.01, "R' at t/δ = {t_d}: {r:+.4}");
        let bound = if t_d >= 1.0 { 0.01 } else { 0.06 };
        assert!(
            dl.abs() < bound,
            "ΔL' at t/δ = {t_d}: {dl:+.4} (bound {bound})"
        );
    }
    println!("{PASSED}: steep grading");
}

/// The skin-depth-adaptive grid (`SkinDepthGrading`, half a skin depth at
/// the surface, 2:1) sizes each axis against δ alone. Measured against the
/// reference it holds `R'` to a few per cent at every frequency and `ΔL'` to
/// ~1 % once `t/δ ≥ 1`; at `t/δ = 0.3` it cuts the thickness once and the
/// width seven times, too coarse for the lateral redistribution, and `ΔL'`
/// (a ~0.1 % share of the trace's total inductance there) is ~13 % high.
#[test]
fn skin_depth_adaptive_grid_against_the_2d_reference() {
    let Some(points) = reference() else { return };
    print_header(
        "SkinDepthGrading(f, 0.5, 2.0), one grid per frequency",
        &points,
    );
    let mut row = Vec::new();
    let mut counts = Vec::new();
    for point in &points {
        let grid = Discretization::SkinDepth(SkinDepthGrading::new(point.frequency, 0.5, 2.0));
        counts.push(filament_count(&grid));
        let (r, dl) = errors(&grid, std::slice::from_ref(point))[0];
        row.push((r, dl));
    }
    print_row(&format!("filaments {counts:?}"), &row);
    for (&(r, dl), p) in row.iter().zip(&points) {
        let t_d = p.thickness_per_delta;
        assert!(r.abs() < 0.03, "R' at t/δ = {t_d}: {r:+.4}");
        let bound = if t_d >= 1.0 { 0.02 } else { 0.2 };
        assert!(
            dl.abs() < bound,
            "ΔL' at t/δ = {t_d}: {dl:+.4} (bound {bound})"
        );
    }
    println!("{PASSED}: skin-depth grid");
}
