//! Scaling of the precorrected-FFT operator ([`PfftOperator`]) from 1 000 to
//! 10 000 filaments: time per [`apply`](PfftOperator::apply), set-up time,
//! and held memory, against the dense matrix-vector product it replaces.
//!
//! Run with `cargo bench -p fasterhenry --bench pfft`. Not part of
//! `cargo test`. Set `FASTERHENRY_BENCH_MAX_FILAMENTS` to cap the sweep, as
//! for the other bench files.
//!
//! The filaments are random, axis-aligned and at a constant density (400
//! per 1 000 mm³), so the cloud's side grows as `n^(1/3)` and the physics
//! per filament stays the same across the sweep: a sub-quadratic method
//! must then scale about linearly. Before the timed groups run, the bench
//! prints the operator's memory (`PfftStats::memory_bytes`) at each size
//! next to the `8n²` bytes of the dense matrix, with the growth exponent
//! between consecutive sizes — memory is deterministic, so it is reported
//! once rather than sampled.
//!
//! The dense reference (`dense_matvec`) needs the assembled `n × n` matrix,
//! whose assembly dominates at the upper sizes; it runs only up to
//! [`DENSE_MAX_FILAMENTS`].

use std::hint::black_box;
use std::time::Instant;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use fasterhenry::pfft::{PfftOperator, PfftParams};
use fasterhenry::{partial_inductance_matrix, Filament, Node, Segment};
use nalgebra::DVector;

#[path = "support.rs"]
#[allow(dead_code)]
mod support;
use support::{max_filaments, COPPER};

/// Filament counts swept: 1k to 10k, as issue #42 asks.
const SIZES: [usize; 4] = [1_000, 2_000, 5_000, 10_000];
/// Largest size at which the dense matrix is assembled for comparison
/// (5 000 filaments is a 200 MB matrix).
const DENSE_MAX_FILAMENTS: usize = 5_000;

fn sizes() -> impl Iterator<Item = usize> {
    let cap = max_filaments();
    SIZES.into_iter().filter(move |&n| n <= cap)
}

/// `n` random axis-aligned filaments at constant density, from a fixed
/// linear congruential sequence so that every run sees the same geometry.
fn cloud(n: usize) -> Vec<Filament> {
    let side = 10e-3 * (n as f64 / 400.0).cbrt();
    let mut state = 11u64;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    (0..n)
        .map(|_| {
            let a = [side * next(), side * next(), side * next()];
            let length = 1e-3 * (0.5 + next());
            let mut d = [0.0; 3];
            d[((next() * 3.0) as usize).min(2)] = if next() < 0.5 { -1.0 } else { 1.0 };
            let b = [0, 1, 2].map(|k| a[k] + length * d[k]);
            let width = 0.1e-3 + 0.2e-3 * next();
            let height = 0.05e-3 + 0.1e-3 * next();
            Filament::new(&Segment::new(
                Node::from(a),
                Node::from(b),
                width,
                height,
                COPPER,
            ))
            .expect("valid segment")
        })
        .collect()
}

fn input(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| ((i * 37) % 101) as f64 / 50.0 - 1.0)
        .collect()
}

/// Prints held memory and set-up time per size, with the growth exponent
/// `log(m₂/m₁) / log(n₂/n₁)` between consecutive sizes (2 is quadratic).
fn report_memory() {
    eprintln!("pFFT operator memory (default parameters):");
    eprintln!(
        "  {:>7}  {:>11}  {:>8}  {:>13}  {:>9}",
        "n", "pFFT", "exponent", "dense 8n²", "set-up"
    );
    let mut previous: Option<(usize, f64)> = None;
    for n in sizes() {
        let filaments = cloud(n);
        let start = Instant::now();
        let op = PfftOperator::new(&filaments, &PfftParams::default()).expect("builds");
        let setup = start.elapsed();
        let memory = op.stats().memory_bytes as f64;
        let exponent = previous.map_or(String::from("-"), |(m, bytes)| {
            format!("{:.2}", (memory / bytes).ln() / (n as f64 / m as f64).ln())
        });
        eprintln!(
            "  {n:>7}  {:>8.1} MB  {exponent:>8}  {:>10.1} MB  {setup:>9.2?}",
            memory / 1e6,
            8.0 * (n * n) as f64 / 1e6,
        );
        previous = Some((n, memory));
    }
}

fn bench_apply(c: &mut Criterion) {
    report_memory();
    let mut group = c.benchmark_group("pfft_apply");
    group.sample_size(20);
    for n in sizes() {
        let filaments = cloud(n);
        let op = PfftOperator::new(&filaments, &PfftParams::default()).expect("builds");
        let x = input(n);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &x, |b, x| {
            b.iter(|| op.apply(black_box(x)));
        });
    }
    group.finish();

    let mut group = c.benchmark_group("dense_matvec");
    group.sample_size(20);
    for n in sizes().filter(|&n| n <= DENSE_MAX_FILAMENTS) {
        let dense = partial_inductance_matrix(&cloud(n)).expect("assembles");
        let x = DVector::from_vec(input(n));
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &x, |b, x| {
            b.iter(|| &dense * black_box(x));
        });
    }
    group.finish();
}

fn bench_setup(c: &mut Criterion) {
    let mut group = c.benchmark_group("pfft_setup");
    group.sample_size(10);
    for n in sizes() {
        let filaments = cloud(n);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &filaments, |b, f| {
            b.iter(|| PfftOperator::new(black_box(f), &PfftParams::default()).expect("builds"));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_apply, bench_setup);
criterion_main!(benches);
