//! The dense path ([`MeshSystem`]) against the matrix-free one
//! ([`IterativeSystem`]: GMRES on the precorrected-FFT operator) end to end —
//! assembly plus a one-frequency sweep — from 1 000 to 100 000 filaments.
//!
//! This is the measurement behind
//! [`DENSE_PATH_MAX_FILAMENTS`](fasterhenry::DENSE_PATH_MAX_FILAMENTS), the
//! size at which `SolverChoice::Auto` (and so the CLI's `--solver auto`)
//! hands over from one path to the other. Run it with
//! `cargo bench -p fasterhenry --bench scaling`; it is not part of
//! `cargo test`, and `.github/workflows/bench.yml` does not run it — the
//! upper sizes are minutes to hours per criterion sample and gigabytes of
//! resident memory. The committed numbers live in `docs/benchmarks.md`.
//!
//! # What is compared
//!
//! Both paths are timed over `assemble` + `sweep(&[1 MHz])`, because the two
//! spend their time in different places: the dense path's cost is dominated
//! by the `O(n²)` partial-inductance assembly, the iterative path's by the
//! GMRES iterations of the solve. Timing only the solve stage would flatter
//! the dense path by exactly the term that makes it unaffordable.
//!
//! Memory is reported two ways per size:
//!
//! * **model** — what the path must hold by construction, and so is
//!   machine-independent: `8n²` bytes for the dense `L` plus `16m²` for the
//!   complex internal-loop block it factorizes, against
//!   `PfftStats::memory_bytes` plus GMRES's `restart + 1` Krylov vectors;
//! * **peak RSS** — the process high-water mark around each measurement
//!   (Linux only, read from `/proc/self/status` after resetting the mark
//!   through `/proc/self/clear_refs`; omitted elsewhere). It is a whole-
//!   process figure, so read it as a cross-check on the model rather than an
//!   independent measurement of one data structure. It can land on either
//!   side: above the model where the allocator is still holding an earlier
//!   size's arena (as at the smallest sizes), below it where the terms the
//!   model sums are not all live at the same instant (as on the dense arm at
//!   9 800 filaments).
//!
//! Both appear in the report the bench prints before its timed groups, which
//! also runs each size exactly once — the affordable way to reach the sizes
//! where criterion's ten-sample floor is not.
//!
//! # The fixture
//!
//! A square meander (`meander`): `bars` conductor runs along ±x at a fixed
//! pitch, each cut into `bars` collinear segments one pitch long and joined
//! to the next run by a jog along +y, with one port across the whole chain.
//! Three properties matter for a scaling sweep, and the serpentine of
//! `benches/support.rs` (whole-bar segments, fixed 2 mm length) has only the
//! first:
//!
//! * the filament count is `4·bars·(bars + 1)` at the 2 × 2 cross-section
//!   subdivision used throughout the bench suite, and every filament carries
//!   the same three bundle loops, so the internal-loop count stays a fixed
//!   fraction of it;
//! * the footprint stays **square** as the size grows, instead of stretching
//!   into a ribbon — the pFFT grid covers a bounding box, so an aspect ratio
//!   that grows with `n` would drive its spacing down and charge the
//!   iterative path for the fixture's shape rather than its size;
//! * segments stay **one pitch long**, a few grid cells, instead of growing
//!   to hundreds of cells: a filament's projection cost is proportional to
//!   its length in cells (see `fasterhenry::pfft`).
//!
//! # Capping the sweep
//!
//! `FASTERHENRY_BENCH_MAX_FILAMENTS` caps both arms, as for the other bench
//! files. `FASTERHENRY_BENCH_DENSE_MAX_FILAMENTS` caps the dense arm alone
//! and defaults to
//! [`DENSE_PATH_MAX_FILAMENTS`](fasterhenry::DENSE_PATH_MAX_FILAMENTS): the
//! dense path's working set is about 2.5 GB at 10 000 filaments, 23 GB at
//! 30 000 and 256 GB at 100 000, so running it above the threshold is an
//! explicit, machine-specific decision.

use std::hint::black_box;
use std::time::{Duration, Instant};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use fasterhenry::{
    Discretization, Geometry, GmresParams, IterativeParams, IterativeSystem, MeshSystem, Node,
    Port, SegmentDef, DENSE_PATH_MAX_FILAMENTS,
};

#[path = "support.rs"]
#[allow(dead_code)]
mod support;
use support::{max_filaments, COPPER, SUBDIVISION_NH, SUBDIVISION_NW};

/// Bar counts of [`meander`], whose [`filament_count`] is about 1 000,
/// 2 500, 5 000, 10 000, 30 000 and 100 000 — the three sizes issue #44 asks
/// for, with three smaller ones to place the crossover.
const BARS: [usize; 6] = [15, 24, 34, 49, 86, 157];

/// One representative frequency, as in `benches/solve.rs`: high enough that
/// `L` is not dropped, low enough that the fixture stays physical.
const FREQUENCY_HZ: f64 = 1e6;

/// Conductor pitch, and the length of one segment: the meander's footprint
/// is `bars · PITCH` square.
const PITCH: f64 = 100e-6;
/// Conductor width, in metres.
const WIDTH: f64 = 20e-6;
/// Conductor thickness, in metres.
const THICKNESS: f64 = 10e-6;

/// Filaments [`meander`] produces for a bar count, at the bench suite's
/// 2 × 2 cross-section subdivision.
const fn filament_count(bars: usize) -> usize {
    bars * (bars + 1) * SUBDIVISION_NW * SUBDIVISION_NH
}

/// Internal (source-free) loops of that meander: three bundle loops per
/// segment at a 2 × 2 subdivision, and no closed conductor rings.
const fn internal_loops(bars: usize) -> usize {
    bars * (bars + 1) * (SUBDIVISION_NW * SUBDIVISION_NH - 1)
}

/// A square meander of `bars` runs, each `bars` segments of [`PITCH`] long,
/// with one port across the whole chain. See the module documentation.
fn meander(bars: usize) -> (Geometry, Port) {
    let mut geometry = Geometry::new();
    let mut previous = geometry.add_node(Node::new(0.0, 0.0, 0.0)).unwrap();
    let first = previous;
    let add = |geometry: &mut Geometry, previous: &mut _, x: f64, y: f64| {
        let node = geometry.add_node(Node::new(x, y, 0.0)).unwrap();
        geometry
            .add_segment(SegmentDef::new(*previous, node, WIDTH, THICKNESS, COPPER))
            .unwrap();
        *previous = node;
    };
    for bar in 0..bars {
        let y = bar as f64 * PITCH;
        // Even runs go towards +x, odd runs back towards −x.
        for step in 1..=bars {
            let along = if bar % 2 == 0 { step } else { bars - step };
            add(&mut geometry, &mut previous, along as f64 * PITCH, y);
        }
        // The jog onto the next run, and a lead-out after the last one.
        let x = if bar % 2 == 0 { bars } else { 0 } as f64 * PITCH;
        add(&mut geometry, &mut previous, x, y + PITCH);
    }
    (geometry, Port::new(first, previous))
}

/// Bytes the dense path must hold at its peak: the `n × n` real
/// partial-inductance matrix, plus the complex `m × m` internal-loop block
/// and the copy `lu_solve` factorizes.
const fn dense_model_bytes(bars: usize) -> usize {
    let n = filament_count(bars);
    let m = internal_loops(bars);
    8 * n * n + 2 * 16 * m * m
}

/// Bytes the iterative path must hold at its peak: the operator itself, the
/// transient FFT work buffers one product needs, and GMRES's `restart + 1`
/// Krylov vectors of complex internal-loop currents — the same terms
/// `IterativeSystem::sweep` budgets its concurrency against.
fn iterative_model_bytes(bars: usize, system: &IterativeSystem) -> usize {
    let stats = system.operator_stats();
    let fft_points: usize = stats.fft_dims.iter().product();
    let krylov = (GmresParams::default().restart + 1) * internal_loops(bars) * 16;
    stats.memory_bytes + krylov + 4 * fft_points * 16
}

/// Filament counts to sweep, and the cap on the dense arm.
fn bars_within_budget() -> impl Iterator<Item = usize> {
    let cap = max_filaments();
    BARS.into_iter()
        .filter(move |&bars| filament_count(bars) <= cap)
}

/// Upper bound on the dense arm, from `FASTERHENRY_BENCH_DENSE_MAX_FILAMENTS`
/// (default: [`DENSE_PATH_MAX_FILAMENTS`] — see the module documentation).
fn dense_max_filaments() -> usize {
    std::env::var("FASTERHENRY_BENCH_DENSE_MAX_FILAMENTS")
        .ok()
        .map(|s| {
            s.parse()
                .expect("FASTERHENRY_BENCH_DENSE_MAX_FILAMENTS must be a number")
        })
        .unwrap_or(DENSE_PATH_MAX_FILAMENTS)
}

fn dense_bars() -> impl Iterator<Item = usize> {
    let cap = dense_max_filaments();
    bars_within_budget().filter(move |&bars| filament_count(bars) <= cap)
}

/// Resets the process's peak-RSS high-water mark, so the next
/// [`peak_rss_bytes`] reports this measurement's peak and not an earlier,
/// larger one. Linux only; a no-op (and harmless) elsewhere.
fn reset_peak_rss() {
    let _ = std::fs::write("/proc/self/clear_refs", "5\n");
}

/// The process's peak resident set size in bytes, from `/proc/self/status`
/// (`VmHWM`); `None` where that file does not exist.
fn peak_rss_bytes() -> Option<usize> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmHWM:"))?;
    let kilobytes: usize = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kilobytes * 1024)
}

fn megabytes(bytes: usize) -> f64 {
    bytes as f64 / 1e6
}

/// One dense and one iterative run per size, timed and measured once each —
/// the numbers `docs/benchmarks.md` quotes. Criterion's own groups below
/// re-measure the sizes that are affordable to sample ten times.
fn report() {
    eprintln!(
        "square meander, {}×{} filaments per segment, assemble + sweep([{FREQUENCY_HZ:e} Hz]); \
         dense arm capped at {} filaments",
        SUBDIVISION_NW,
        SUBDIVISION_NH,
        dense_max_filaments(),
    );
    eprintln!(
        "  {:>7} {:>7}  {:>9} {:>10} {:>10}  {:>9} {:>10} {:>10} {:>6}",
        "n", "loops", "dense s", "model MB", "peak MB", "iter s", "model MB", "peak MB", "GMRES"
    );
    for bars in bars_within_budget() {
        let (geometry, port) = meander(bars);
        let ports = [port];
        let discretization = Discretization::uniform(SUBDIVISION_NW, SUBDIVISION_NH);
        let n = filament_count(bars);

        // The iterative arm runs first at every size: the peak-RSS mark is
        // process-wide, so measuring the memory-hungry path first would let
        // the allocator's retained arena dominate the other's reading.
        reset_peak_rss();
        let start = Instant::now();
        let system = IterativeSystem::assemble(
            &geometry,
            &ports,
            &discretization,
            &IterativeParams::default(),
        )
        .expect("iterative");
        let solution = system.solve(FREQUENCY_HZ).expect("iterative solves");
        let iterative_seconds = start.elapsed();
        let iterative_peak = peak_rss_bytes();
        let iterations: usize = solution.gmres.iter().map(|o| o.iterations).sum();

        let (dense_seconds, dense_peak) = if filament_count(bars) <= dense_max_filaments() {
            reset_peak_rss();
            let start = Instant::now();
            let dense = MeshSystem::assemble(&geometry, &ports, &discretization).expect("dense");
            dense.sweep(&[FREQUENCY_HZ]).expect("dense solves");
            (Some(start.elapsed()), peak_rss_bytes())
        } else {
            (None, None)
        };

        let seconds = |d: Option<Duration>| {
            d.map_or_else(|| String::from("-"), |d| format!("{:.2}", d.as_secs_f64()))
        };
        let peak = |b: Option<usize>| {
            b.map_or_else(|| String::from("-"), |b| format!("{:.0}", megabytes(b)))
        };
        eprintln!(
            "  {n:>7} {:>7}  {:>9} {:>10.0} {:>10}  {:>9.2} {:>10.0} {:>10} {iterations:>6}",
            internal_loops(bars),
            seconds(dense_seconds),
            megabytes(dense_model_bytes(bars)),
            peak(dense_peak),
            iterative_seconds.as_secs_f64(),
            megabytes(iterative_model_bytes(bars, &system)),
            peak(iterative_peak),
        );
    }
}

fn bench_scaling(c: &mut Criterion) {
    report();

    let mut group = c.benchmark_group("scaling");
    group.sample_size(10);
    let discretization = Discretization::uniform(SUBDIVISION_NW, SUBDIVISION_NH);

    for bars in dense_bars() {
        let (geometry, port) = meander(bars);
        let ports = [port];
        let n = filament_count(bars);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_function(BenchmarkId::new("dense", n), |b| {
            b.iter(|| {
                let system =
                    MeshSystem::assemble(&geometry, &ports, &discretization).expect("dense");
                system
                    .sweep(black_box(&[FREQUENCY_HZ]))
                    .expect("dense solves")
            });
        });
    }

    for bars in bars_within_budget() {
        let (geometry, port) = meander(bars);
        let ports = [port];
        let n = filament_count(bars);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_function(BenchmarkId::new("iterative", n), |b| {
            b.iter(|| {
                let system = IterativeSystem::assemble(
                    &geometry,
                    &ports,
                    &discretization,
                    &IterativeParams::default(),
                )
                .expect("iterative");
                system
                    .sweep(black_box(&[FREQUENCY_HZ]))
                    .expect("iterative solves")
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_scaling);
criterion_main!(benches);
