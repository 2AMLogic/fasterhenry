//! Shared synthetic geometry for the assembly- and solve-stage throughput
//! benches (`assembly.rs`, `solve.rs`).
//!
//! `cargo bench` compiles each `[[bench]]` target as its own binary crate,
//! so there is no way to share a helper module through the library the way
//! ordinary code would; both bench files pull this one in with
//! `#[path = "support.rs"] mod support;`, mirroring the geometry generator
//! already used by `fasterhenry/tests/perf_smoke.rs`.

use fasterhenry::{Geometry, Node, Port, SegmentDef};

/// Copper conductivity, siemens per metre (matches `benches/kernels.rs` and
/// `tests/perf_smoke.rs`).
pub const COPPER: f64 = 5.8e7;

/// `nw × nh` cross-section subdivision applied to every segment: enough to
/// give the solve stage real internal-mesh work (`nw·nh − 1` loops per
/// segment) without near-field quadrature dominating assembly cost the way
/// a single densely subdivided trace would.
pub const SUBDIVISION_NW: usize = 2;
pub const SUBDIVISION_NH: usize = 2;

/// Bar counts for [`serpentine`] whose filament total (`8 · bars`, from two
/// segments per bar times `SUBDIVISION_NW * SUBDIVISION_NH`) lands on the
/// scale of the head-to-head table in `docs/benchmarks.md`: 48, ~250,
/// 1 000, 5 000, 19 600 filaments.
pub const BARS: [usize; 5] = [6, 31, 125, 625, 2450];

/// Filament count [`serpentine`] produces for a given bar count, at
/// [`SUBDIVISION_NW`] × [`SUBDIVISION_NH`].
pub const fn filament_count(bars: usize) -> usize {
    2 * bars * SUBDIVISION_NW * SUBDIVISION_NH
}

/// A planar serpentine: `bars` long traces along ±x at a fixed pitch, each
/// joined to the next by a short jog along +y, with a final jog as
/// lead-out. Identical in shape to the fixture in
/// `fasterhenry/tests/perf_smoke.rs`, parameterized here so the bar count
/// alone controls the filament total once combined with a fixed
/// subdivision, and every run sees the same geometry (nothing random).
pub fn serpentine(bars: usize) -> (Geometry, Port) {
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

/// Upper bound on the filament count to sweep, from the
/// `FASTERHENRY_BENCH_MAX_FILAMENTS` environment variable. Unset means the
/// full sweep, including the ~19 600-filament point, whose dense assembly
/// and solve are minutes of work; the CI workflow
/// (`.github/workflows/bench.yml`) sets a smaller cap to stay inside its
/// time budget on a shared runner. A local `cargo bench -p fasterhenry`
/// (no environment override) always runs the full sweep.
pub fn max_filaments() -> usize {
    std::env::var("FASTERHENRY_BENCH_MAX_FILAMENTS")
        .ok()
        .map(|s| {
            s.parse()
                .expect("FASTERHENRY_BENCH_MAX_FILAMENTS must be a number")
        })
        .unwrap_or(usize::MAX)
}

/// [`BARS`] filtered to those whose [`filament_count`] fits under
/// [`max_filaments`].
pub fn bars_within_budget() -> impl Iterator<Item = usize> {
    let cap = max_filaments();
    BARS.into_iter()
        .filter(move |&bars| filament_count(bars) <= cap)
}
