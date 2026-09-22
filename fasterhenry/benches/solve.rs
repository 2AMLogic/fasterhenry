//! Throughput of [`MeshSystem::sweep`] (the frequency solve: forming
//! `R + jωL` on the internal-loop system and factorizing it) in filaments
//! per second, vs. filament count. Assembly is excluded from the timed
//! section — [`MeshSystem::assemble`] runs once per size before the
//! benchmark loop, since its own cost is what `benches/assembly.rs`
//! measures.
//!
//! Run with `cargo bench -p fasterhenry --bench solve`. Not part of
//! `cargo test`. The filament-count sweep mirrors the head-to-head table in
//! `docs/benchmarks.md` (48, ~250, 1 000, 5 000, 19 600); set
//! `FASTERHENRY_BENCH_MAX_FILAMENTS` to cap it (the CI workflow does, to
//! keep the job inside its time budget — see
//! `.github/workflows/bench.yml`).
//!
//! The solve stage's own cost scales with the internal-mesh count (the
//! order of the dense system factorized), not the filament count directly;
//! [`support::SUBDIVISION_NW`] × [`support::SUBDIVISION_NH`] fixes a
//! constant ratio between the two across the sweep so filament count stays
//! a fair x-axis to compare against `assembly.rs`.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use fasterhenry::{Discretization, MeshSystem};

#[path = "support.rs"]
mod support;
use support::{bars_within_budget, filament_count, serpentine, SUBDIVISION_NH, SUBDIVISION_NW};

/// One representative frequency: low enough that the skin effect barely
/// perturbs the resistive part, high enough that `L` is not trivially
/// dropped (`ω = 0` would solve a real, not complex, system).
const FREQUENCY_HZ: f64 = 1e6;

fn bench_solve(c: &mut Criterion) {
    let mut group = c.benchmark_group("solve");
    for bars in bars_within_budget() {
        let (geometry, port) = serpentine(bars);
        let discretization = Discretization::uniform(SUBDIVISION_NW, SUBDIVISION_NH);
        let system = MeshSystem::assemble(&geometry, &[port], &discretization).expect("assembles");
        let filaments = filament_count(bars);

        group.throughput(Throughput::Elements(filaments as u64));
        group.sample_size(if filaments >= 1_000 { 10 } else { 20 });

        group.bench_with_input(
            BenchmarkId::from_parameter(filaments),
            &system,
            |b, system| {
                b.iter(|| system.sweep(black_box(&[FREQUENCY_HZ])).expect("solves"));
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_solve);
criterion_main!(benches);
