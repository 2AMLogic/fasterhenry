//! The precorrected-FFT operator against the dense partial-inductance
//! matrix it replaces.
//!
//! Every test builds a filament set, applies [`PfftOperator`] to a vector
//! and compares with `partial_inductance_matrix(filaments) · x`. The error
//! measure is norm-wise, `‖y_pfft − y_dense‖₂ / ‖y_dense‖₂`, and the
//! acceptance bound for the default parameters is `1e-6` (issue #42).
//!
//! Fixtures: random filament clouds at several scales (axis-aligned and in
//! general position), the 2-turn spiral of `spiral_validation.rs`, the
//! coupled structure of `solve.rs`, highly anisotropic bounding boxes, and
//! sets smaller than one grid cell. `parameter_study` (ignored) prints the
//! accuracy-versus-cost table the module documentation summarizes.

use std::time::Instant;

use fasterhenry::geometry::{Geometry, Node};
use fasterhenry::mesh::Port;
use fasterhenry::pfft::{GridSpacing, PfftError, PfftOperator, PfftParams};
use fasterhenry::solve::{Discretization, MeshSystem};
use fasterhenry::{discretize, partial_inductance_matrix, Filament, Segment};
use nalgebra::DVector;

// The spiral and the coupled structure are the fixtures of
// `spiral_validation.rs` and `solve.rs`; this file asserts the pFFT
// operator on *those* geometries, so it reads their one definition rather
// than restating them.
#[allow(dead_code)]
mod common;
use common::COPPER;

// The random filament cloud is the fixture the pFFT bench measures on, so
// this file reads the bench suite's one definition rather than keeping a
// second copy that nothing checks against it. Same form as
// `tests/perf_smoke.rs`, which asserts the bench headline on the same
// geometry. `COPPER` is identical in both modules; the import above picks
// `common`'s so the crate has exactly one in scope.
#[path = "../benches/support.rs"]
#[allow(dead_code)]
mod support;
use support::{cloud_side, lcg, random_set};

/// Acceptance bound on the norm-wise relative error with default parameters.
const TOLERANCE: f64 = 1e-6;

/// A deterministic vector with entries of both signs.
fn test_vector(n: usize, seed: u64) -> Vec<f64> {
    let mut r = lcg(seed);
    (0..n).map(|_| r() - 0.5).collect()
}

/// `(‖Δ‖₂/‖y‖₂, max|Δ|/max|y|)` of the pFFT product against the dense one.
fn errors(filaments: &[Filament], params: &PfftParams, x: &[f64]) -> (f64, f64) {
    let dense = partial_inductance_matrix(filaments).unwrap();
    let reference = &dense * DVector::from_column_slice(x);
    let op = PfftOperator::new(filaments, params).unwrap();
    let y = op.apply(x);
    let difference = DVector::from_vec(y) - &reference;
    (
        difference.norm() / reference.norm(),
        difference.amax() / reference.amax(),
    )
}

fn assert_accurate(what: &str, filaments: &[Filament], params: &PfftParams) {
    let x = test_vector(filaments.len(), 99);
    let (norm, max) = errors(filaments, params, &x);
    println!(
        "{what}: {} filaments, relative error {norm:.2e} (norm), {max:.2e} (max entry)",
        filaments.len()
    );
    assert!(
        norm < TOLERANCE,
        "{what}: norm-wise relative error {norm:e}"
    );
    assert!(max < TOLERANCE, "{what}: max-entry relative error {max:e}");
}

// ---------------------------------------------------------------------------
// Random filament sets
// ---------------------------------------------------------------------------

#[test]
fn random_sets_match_the_dense_product_at_several_scales() {
    // Constant density (400 filaments per 1000 mm³), so the larger sets have
    // proportionally more well-separated pairs on the FFT path.
    for (n, seed) in [(60, 1), (200, 2), (500, 3)] {
        let side = cloud_side(n);
        for manhattan in [true, false] {
            let filaments = random_set(n, side, manhattan, seed);
            let what = if manhattan { "manhattan" } else { "skew" };
            assert_accurate(what, &filaments, &PfftParams::default());
        }
    }
}

#[test]
fn most_pairs_of_a_large_random_set_take_the_fft_path() {
    // Guards against the accuracy tests passing only because everything is
    // near-field: at 500 filaments most pairs must be approximated.
    let n = 500;
    let filaments = random_set(n, cloud_side(n), true, 3);
    let stats = PfftOperator::new(&filaments, &PfftParams::default())
        .unwrap()
        .stats();
    let near_fraction = stats.near_entries as f64 / (n * n) as f64;
    assert!(near_fraction < 0.5, "near fraction {near_fraction}");
}

#[test]
fn the_operator_is_symmetric() {
    // Spreading and gathering are transposes and the kernel and the near
    // field are symmetric, so ⟨u, L̃v⟩ = ⟨L̃u, v⟩ to rounding.
    let filaments = random_set(300, 10e-3, false, 5);
    let op = PfftOperator::new(&filaments, &PfftParams::default()).unwrap();
    let u = test_vector(filaments.len(), 1);
    let v = test_vector(filaments.len(), 2);
    let dot = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(a, b)| a * b).sum::<f64>();
    let (left, right) = (dot(&u, &op.apply(&v)), dot(&op.apply(&u), &v));
    assert!(
        (left - right).abs() <= 1e-12 * left.abs().max(right.abs()),
        "{left} vs {right}"
    );
}

#[test]
fn application_is_linear_and_repeatable() {
    let filaments = random_set(150, 8e-3, true, 6);
    let op = PfftOperator::new(&filaments, &PfftParams::default()).unwrap();
    let u = test_vector(filaments.len(), 3);
    let v = test_vector(filaments.len(), 4);
    let combined: Vec<f64> = u.iter().zip(&v).map(|(a, b)| 2.0 * a - 3.0 * b).collect();
    let (lu, lv, lc) = (op.apply(&u), op.apply(&v), op.apply(&combined));
    let scale = lc.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    for i in 0..filaments.len() {
        assert!((lc[i] - (2.0 * lu[i] - 3.0 * lv[i])).abs() <= 1e-12 * scale);
    }
    assert_eq!(op.apply(&u), lu);
}

// ---------------------------------------------------------------------------
// The spiral and coupled-structure fixtures
// ---------------------------------------------------------------------------

/// The filaments a shared fixture discretizes into. The operator is tested
/// against the dense matrix over a filament set, which is all these two
/// fixtures are needed for here — the ports and the solve belong to the
/// tests that validate the physics on them.
fn filaments_of(system: (Geometry, Vec<Port>, Discretization)) -> Vec<Filament> {
    let (geometry, ports, discretization) = system;
    MeshSystem::assemble(&geometry, &ports, &discretization)
        .unwrap()
        .filaments()
        .to_vec()
}

/// The 2-turn square spiral of `spiral_validation.rs`: segment lengths 60,
/// 60, 75, 75, 90, 90, 105, 105 µm, 2 × 2 µm copper, 4 × 4 filaments each.
fn spiral_filaments() -> Vec<Filament> {
    filaments_of(common::spiral_system())
}

/// The coupled structure of `solve.rs`: two traces (one with a bend), a
/// closed ring and a floating bar, with mixed subdivisions.
fn coupled_structure_filaments() -> Vec<Filament> {
    filaments_of(common::coupled_structure())
}

#[test]
fn spiral_fixture_matches_the_dense_product() {
    let filaments = spiral_filaments();
    assert_eq!(filaments.len(), 128);
    assert_accurate("spiral", &filaments, &PfftParams::default());
    // A finer grid pushes most of the spiral onto the FFT path.
    let fine = PfftParams {
        grid_spacing: GridSpacing::Fixed(2e-6),
        ..PfftParams::default()
    };
    assert_accurate("spiral, 2 µm grid", &filaments, &fine);
}

#[test]
fn coupled_structure_fixture_matches_the_dense_product() {
    let filaments = coupled_structure_filaments();
    assert_accurate("coupled structure", &filaments, &PfftParams::default());
    let fine = PfftParams {
        grid_spacing: GridSpacing::Fixed(0.1e-3),
        ..PfftParams::default()
    };
    assert_accurate("coupled structure, 0.1 mm grid", &filaments, &fine);
}

// ---------------------------------------------------------------------------
// Degenerate and near-degenerate grids
// ---------------------------------------------------------------------------

#[test]
fn a_planar_set_uses_a_flat_grid() {
    // Every filament in one plane, 35 µm thick: the bounding box is about
    // 300 times thinner than it is wide.
    let mut r = lcg(8);
    let filaments: Vec<Filament> = (0..300)
        .map(|_| {
            let a = [10e-3 * r(), 10e-3 * r(), 0.0];
            let (dx, dy) = if r() < 0.5 { (1e-3, 0.0) } else { (0.0, 1e-3) };
            let b = [a[0] + dx, a[1] + dy, 0.0];
            Filament::new(&Segment::new(
                Node::from(a),
                Node::from(b),
                0.1e-3,
                35e-6,
                COPPER,
            ))
            .unwrap()
        })
        .collect();
    let op = PfftOperator::new(&filaments, &PfftParams::default()).unwrap();
    let dims = op.stats().grid_dims;
    assert!(dims[2] < dims[0] / 4 && dims[2] < dims[1] / 4, "{dims:?}");
    assert_accurate("planar", &filaments, &PfftParams::default());
}

#[test]
fn a_long_thin_bundle_uses_a_line_grid() {
    // A 30 mm bus of 4 × 2 filaments cut into 1 mm pieces along its length:
    // a bounding box 100 times longer than it is wide.
    let mut filaments = Vec::new();
    for k in 0..30 {
        let x = k as f64 * 1e-3;
        let piece = Segment::new(
            Node::new(x, 0.0, 0.0),
            Node::new(x + 1e-3, 0.0, 0.0),
            0.3e-3,
            0.1e-3,
            COPPER,
        );
        filaments.extend(discretize(&piece, 4, 2).unwrap());
    }
    let op = PfftOperator::new(&filaments, &PfftParams::default()).unwrap();
    let dims = op.stats().grid_dims;
    assert!(dims[0] > 4 * dims[1] && dims[0] > 4 * dims[2], "{dims:?}");
    assert_accurate("line", &filaments, &PfftParams::default());
}

#[test]
fn a_set_inside_one_grid_cell_is_exact() {
    // Five filaments well inside a single grid cell: every pair is near
    // field, so the product is the dense one to rounding.
    let filaments = random_set(5, 1e-3, false, 9);
    let params = PfftParams {
        grid_spacing: GridSpacing::Fixed(1.0),
        ..PfftParams::default()
    };
    let x = test_vector(filaments.len(), 10);
    let (norm, _) = errors(&filaments, &params, &x);
    assert!(norm < 1e-12, "{norm:e}");
    let stats = PfftOperator::new(&filaments, &params).unwrap().stats();
    assert_eq!(stats.near_entries, 25);
}

#[test]
fn a_single_filament_is_its_self_inductance() {
    let filaments = random_set(1, 1e-3, true, 10);
    let op = PfftOperator::new(&filaments, &PfftParams::default()).unwrap();
    let dense = partial_inductance_matrix(&filaments).unwrap();
    let y = op.apply(&[2.0]);
    assert!((y[0] - 2.0 * dense[(0, 0)]).abs() <= 1e-13 * dense[(0, 0)]);
}

#[test]
fn no_filaments_is_an_empty_operator() {
    let op = PfftOperator::new(&[], &PfftParams::default()).unwrap();
    assert!(op.is_empty());
    assert!(op.apply(&[]).is_empty());
    assert_eq!(op.stats().filaments, 0);
}

#[test]
fn orthogonal_filaments_only_couple_through_their_own_components() {
    // An x-directed and a z-directed set: the partial inductance between
    // them is zero, and the operator must not leak one into the other.
    let mut filaments = random_set(450, 12e-3, true, 11);
    filaments.retain(|f| f.direction().y.abs() < 0.5);
    let n = filaments.len();
    let stats = PfftOperator::new(&filaments, &PfftParams::default())
        .unwrap()
        .stats();
    assert!(stats.near_entries < n * n / 2, "mostly far field");
    assert_accurate("x and z only", &filaments, &PfftParams::default());
}

// ---------------------------------------------------------------------------
// Parameters and scaling
// ---------------------------------------------------------------------------

#[test]
fn invalid_parameters_are_reported() {
    let filaments = random_set(4, 1e-3, true, 12);
    let bad = [
        PfftParams {
            interpolation_order: 0,
            ..PfftParams::default()
        },
        PfftParams {
            near_field_radius: 2.0,
            interpolation_order: 3,
            ..PfftParams::default()
        },
        PfftParams {
            near_field_radius: f64::NAN,
            ..PfftParams::default()
        },
        PfftParams {
            grid_spacing: GridSpacing::Fixed(0.0),
            ..PfftParams::default()
        },
        PfftParams {
            grid_spacing: GridSpacing::Fixed(f64::INFINITY),
            ..PfftParams::default()
        },
        PfftParams {
            grid_spacing: GridSpacing::Auto {
                cells_per_filament: -1.0,
            },
            ..PfftParams::default()
        },
    ];
    for params in bad {
        assert!(
            matches!(
                PfftOperator::new(&filaments, &params),
                Err(PfftError::InvalidParameter(_))
            ),
            "{params:?}"
        );
    }
    let too_fine = PfftParams {
        grid_spacing: GridSpacing::Fixed(1e-7),
        ..PfftParams::default()
    };
    assert!(matches!(
        PfftOperator::new(&filaments, &too_fine),
        Err(PfftError::GridTooLarge { .. })
    ));
}

#[test]
fn memory_grows_linearly_with_the_filament_count() {
    // At constant density, quadrupling the filaments must not come close to
    // the 16× of a dense matrix; the measured ratio is about 4.
    let memory = |n: usize| {
        let side = cloud_side(n);
        let filaments = random_set(n, side, true, 13);
        PfftOperator::new(&filaments, &PfftParams::default())
            .unwrap()
            .stats()
            .memory_bytes as f64
    };
    let ratio = memory(1600) / memory(400);
    println!("memory ratio for 4× the filaments: {ratio:.2}");
    assert!(ratio < 6.0, "memory ratio {ratio}");
}

/// Accuracy and cost over a grid of parameters, for the trade-off table in
/// the module documentation. Run with
/// `cargo test --release -p fasterhenry --test pfft parameter_study -- --ignored --nocapture`.
#[test]
#[ignore]
fn parameter_study() {
    let n = 1000;
    let side = cloud_side(n);
    for manhattan in [true, false] {
        let filaments = random_set(n, side, manhattan, 7);
        let x = test_vector(n, 99);
        let dense = partial_inductance_matrix(&filaments).unwrap();
        let reference = &dense * DVector::from_column_slice(&x);
        println!("manhattan = {manhattan}, n = {n}");
        for cells in [4.0, 8.0, 16.0] {
            for radius in [4.0, 6.0, 8.0] {
                for order in [2, 3, 4, 5] {
                    if radius < (order + 1) as f64 {
                        continue;
                    }
                    let params = PfftParams {
                        grid_spacing: GridSpacing::Auto {
                            cells_per_filament: cells,
                        },
                        near_field_radius: radius,
                        interpolation_order: order,
                    };
                    let start = Instant::now();
                    let op = PfftOperator::new(&filaments, &params).unwrap();
                    let setup = start.elapsed();
                    let start = Instant::now();
                    let y = op.apply(&x);
                    let apply = start.elapsed();
                    let error = (DVector::from_vec(y) - &reference).norm() / reference.norm();
                    let stats = op.stats();
                    println!(
                        "  cells/filament {cells:>4} radius {radius} order {order}: error {error:.1e}  \
                         near/n {:>4.0}  proj/n {:>4.0}  memory {:>5.1} MB  setup {setup:>9.2?}  apply {apply:>9.2?}",
                        stats.near_entries as f64 / n as f64,
                        stats.projection_entries as f64 / n as f64,
                        stats.memory_bytes as f64 / 1e6,
                    );
                }
            }
        }
    }
}
