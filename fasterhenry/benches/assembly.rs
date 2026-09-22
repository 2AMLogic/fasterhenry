//! Throughput of [`MeshSystem::assemble`]: discretization, partial-inductance
//! kernel evaluation and mesh reduction, in filaments per second, vs.
//! filament count.
//!
//! Run with `cargo bench -p fasterhenry --bench assembly`. Not part of
//! `cargo test`. The filament-count sweep mirrors the head-to-head table in
//! `docs/benchmarks.md` (48, ~250, 1 000, 5 000, 19 600); set
//! `FASTERHENRY_BENCH_MAX_FILAMENTS` to cap it (the CI workflow does, to
//! keep the job inside its time budget — see
//! `.github/workflows/bench.yml`).

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use fasterhenry::{Discretization, MeshSystem, Port};

#[path = "support.rs"]
mod support;
use support::{bars_within_budget, filament_count, serpentine, SUBDIVISION_NH, SUBDIVISION_NW};

fn bench_assembly(c: &mut Criterion) {
    let mut group = c.benchmark_group("assembly");
    for bars in bars_within_budget() {
        let (geometry, port) = serpentine(bars);
        let ports: [Port; 1] = [port];
        let discretization = Discretization::uniform(SUBDIVISION_NW, SUBDIVISION_NH);
        let filaments = filament_count(bars);

        group.throughput(Throughput::Elements(filaments as u64));
        // Large sizes are seconds each; keep criterion at its sample-count
        // floor there so the sweep stays affordable (mirrors
        // `benches/kernels.rs`'s `group.sample_size(20)`).
        group.sample_size(if filaments >= 1_000 { 10 } else { 20 });

        group.bench_with_input(
            BenchmarkId::from_parameter(filaments),
            &(geometry, ports, discretization),
            |b, (geometry, ports, discretization)| {
                b.iter(|| {
                    MeshSystem::assemble(
                        black_box(geometry),
                        black_box(ports),
                        black_box(discretization),
                    )
                    .expect("assembles")
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_assembly);
criterion_main!(benches);
