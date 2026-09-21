//! Throughput of the partial-inductance kernels: scalar vs SIMD inner
//! quadrature vs SIMD + `rayon`, in filament pairs per second.
//!
//! Run with `cargo bench -p fasterhenry`. Not part of `cargo test`.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use fasterhenry::inductance::{mutual_batch_with, Execution};
use fasterhenry::{discretize, self_inductance, Filament, Node, Segment};

const COPPER: f64 = 5.8e7;

/// `count` single-filament segments in general position inside a cube of
/// side `box_size` centred at `centre`, from a fixed linear congruential
/// sequence so that every run sees the same geometry.
fn scattered(count: usize, centre: [f64; 3], box_size: f64, seed: u64) -> Vec<Filament> {
    let mut state = seed;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 11) as f64 / (1u64 << 53) as f64 - 0.5
    };
    (0..count)
        .map(|_| {
            let a = centre.map(|c| c + box_size * next());
            let b = [a[0] + 1e-3, a[1] + 2e-3 * next(), a[2] + 2e-3 * next()];
            let segment = Segment::new(Node::from(a), Node::from(b), 0.2e-3, 35e-6, COPPER);
            Filament::new(&segment).expect("valid segment")
        })
        .collect()
}

/// One 10 mm trace cut into an 8 × 4 bundle: every pair is near-field.
fn bundle() -> Vec<Filament> {
    let trace = Segment::new(
        Node::new(0.0, 0.0, 0.0),
        Node::new(10e-3, 0.0, 0.0),
        1e-3,
        35e-6,
        COPPER,
    );
    discretize(&trace, 8, 4).expect("valid segment")
}

fn bench_batches(c: &mut Criterion) {
    let modes = [
        ("scalar", Execution::Scalar),
        ("simd", Execution::Simd),
        ("simd+rayon", Execution::Parallel),
    ];
    // Two clouds of skew filaments many lengths apart: point quadrature of
    // the lowest orders, dominated by per-pair set-up.
    let far_rows = scattered(64, [0.0, 0.0, 0.0], 5e-3, 1);
    let far_cols = scattered(64, [30e-3, 10e-3, 5e-3], 5e-3, 2);
    // The same a length or two apart: higher orders, where the SIMD lanes of
    // the inner quadrature do the work.
    let mid_rows = scattered(64, [0.0, 0.0, 0.0], 1e-3, 3);
    let mid_cols = scattered(64, [2.5e-3, 1e-3, 0.5e-3], 1e-3, 4);
    // A bundle against itself: closed forms and singular quadrature, where
    // only the thread pool helps.
    let near = bundle();

    for (name, r, c_) in [
        ("point_quadrature_far_64x64", &far_rows, &far_cols),
        ("point_quadrature_mid_64x64", &mid_rows, &mid_cols),
        ("bundle_32x32", &near, &near),
    ] {
        let mut group = c.benchmark_group(name);
        group.throughput(Throughput::Elements((r.len() * c_.len()) as u64));
        group.sample_size(20);
        for (label, execution) in modes {
            group.bench_with_input(BenchmarkId::from_parameter(label), &execution, |b, &e| {
                b.iter(|| mutual_batch_with(black_box(r), black_box(c_), e).expect("finite"));
            });
        }
        group.finish();
    }
}

fn bench_self(c: &mut Criterion) {
    let mut group = c.benchmark_group("self_inductance");
    for (label, width) in [("closed_form_l/w=10", 1e-3), ("quadrature_l/w=1e4", 1e-6)] {
        let segment = Segment::new(
            Node::new(0.0, 0.0, 0.0),
            Node::new(10e-3, 0.0, 0.0),
            width,
            width,
            COPPER,
        );
        let filament = Filament::new(&segment).expect("valid segment");
        group.bench_function(label, |b| b.iter(|| self_inductance(black_box(&filament))));
    }
    group.finish();
}

criterion_group!(benches, bench_batches, bench_self, bench_skin_accuracy);
criterion_main!(benches);

/// Cost/accuracy curve for the skin-effect resistance at t/δ = 10.
/// The reference is a converged 128-layer uniform grid; each benchmark
/// assembles and solves a fresh system, so the time includes kernel work.
fn bench_skin_accuracy(c: &mut Criterion) {
    use fasterhenry::{solve, Discretization, Geometry, Port, SegmentDef, MU0};
    let mut geometry = Geometry::new();
    let a = geometry.add_node(Node::new(0.0, 0.0, 0.0)).unwrap();
    let b = geometry.add_node(Node::new(0.4, 0.0, 0.0)).unwrap();
    geometry
        .add_segment(SegmentDef::new(a, b, 40e-3, 200e-6, COPPER))
        .unwrap();
    let ports = [Port::new(a, b)];
    let delta: f64 = 20e-6;
    let frequency = 1.0 / (std::f64::consts::PI * MU0 * COPPER * delta * delta);
    let reference = solve(
        &geometry,
        &ports,
        &Discretization::uniform(1, 128),
        &[frequency],
    )
    .unwrap()
    .impedance_ohm[0][(0, 0)]
        .re;
    let mut group = c.benchmark_group("skin_resistance_accuracy");
    group.sample_size(10);
    for count in [4, 8, 12, 16] {
        for (kind, grid) in [
            ("uniform", Discretization::uniform(1, count)),
            ("graded_2to1", Discretization::graded(1, count, 2.0)),
        ] {
            let resistance = solve(&geometry, &ports, &grid, &[frequency])
                .unwrap()
                .impedance_ohm[0][(0, 0)]
                .re;
            let error_pct = 100.0 * (resistance / reference - 1.0).abs();
            println!("skin accuracy: {kind} {count} filaments, R error {error_pct:.3}%");
            group.bench_with_input(BenchmarkId::new(kind, count), &grid, |b, grid| {
                b.iter(|| {
                    solve(
                        black_box(&geometry),
                        black_box(&ports),
                        black_box(grid),
                        black_box(&[frequency]),
                    )
                    .unwrap()
                })
            });
        }
    }
    group.finish();
}
