//! Shared synthetic geometry for the throughput benches (`assembly.rs`,
//! `solve.rs`, `pfft.rs`, `scaling.rs`), for the performance smoke test that
//! asserts their headline, `fasterhenry/tests/perf_smoke.rs`, and for the
//! accuracy tests that guard the pFFT operator on the same filament cloud
//! its bench measures, `fasterhenry/tests/pfft.rs`.
//!
//! `cargo bench` compiles each `[[bench]]` target as its own binary crate,
//! and `cargo test` does the same for each `tests/*.rs`, so there is no way
//! to share a helper module through the library the way ordinary code would.
//! Every one of those crates pulls this file in instead of copying it: the
//! benches with `#[path = "support.rs"] mod support;`, the smoke test with
//! `#[path = "../benches/support.rs"] mod support;`. The fixtures therefore
//! have exactly one definition, so a bench and the test that guards it can
//! never drift onto different geometry while both keep passing.
//!
//! Nothing here is part of the published crate: `fasterhenry/Cargo.toml`
//! packages `src/**/*.rs` only.

use fasterhenry::{Filament, Geometry, Node, Port, Segment, SegmentDef};

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
/// lead-out. `2·bars` segments in one chain; the port spans the whole
/// chain. Parameterized by the bar count so that alone controls the
/// filament total once combined with a fixed subdivision, and every run
/// sees the same geometry (nothing random).
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

/// Conductor pitch of [`meander`], and the length of one of its segments:
/// the fixture's footprint is `bars · PITCH` square.
pub const PITCH: f64 = 100e-6;
/// Conductor width of [`meander`], in metres.
pub const WIDTH: f64 = 20e-6;
/// Conductor thickness of [`meander`], in metres.
pub const THICKNESS: f64 = 10e-6;

/// A square meander: `bars` conductor runs along ±x at a fixed [`PITCH`],
/// each cut into `bars` collinear segments one pitch long and joined to the
/// next by a jog along +y, with a final jog as lead-out; one port spans the
/// whole chain. That is `bars·(bars + 1)` segments, and so
/// `4·bars·(bars + 1)` filaments at the 2 × 2 cross-section subdivision the
/// bench suite uses.
///
/// Unlike [`serpentine`], the footprint stays square and the segments stay
/// one pitch long as `bars` grows; both properties are what make it a fair
/// scaling fixture for the pFFT operator, and the module documentation of
/// `fasterhenry/benches/scaling.rs` sets out why. That bench measures the
/// scaling headline, and `fasterhenry/tests/perf_smoke.rs` asserts it.
pub fn meander(bars: usize) -> (Geometry, Port) {
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

// ---------------------------------------------------------------------------
// The random filament cloud
// ---------------------------------------------------------------------------

/// Uniform deviates in `[0, 1)` from a fixed linear congruential sequence,
/// so every run sees the same geometry.
pub fn lcg(seed: u64) -> impl FnMut() -> f64 {
    let mut state = seed;
    move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// `n` filaments of length 0.5–1.5 mm scattered in a cube of side `side`:
/// axis-aligned (random axis and sense) when `manhattan`, else in general
/// position. Cross-sections 0.1–0.3 × 0.05–0.15 mm.
pub fn random_set(n: usize, side: f64, manhattan: bool, seed: u64) -> Vec<Filament> {
    let mut r = lcg(seed);
    (0..n)
        .map(|_| {
            let a = [side * r(), side * r(), side * r()];
            let length = 1e-3 * (0.5 + r());
            let d = if manhattan {
                let mut d = [0.0; 3];
                d[((r() * 3.0) as usize).min(2)] = if r() < 0.5 { -1.0 } else { 1.0 };
                d
            } else {
                let v = [r() - 0.5, r() - 0.5, r() - 0.5];
                let norm = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                v.map(|c| c / norm)
            };
            let b = [0, 1, 2].map(|k| a[k] + length * d[k]);
            let width = 0.1e-3 + 0.2e-3 * r();
            let height = 0.05e-3 + 0.1e-3 * r();
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

/// The fixture every pFFT measurement is taken on: [`random_set`] at the
/// constant density of 400 filaments per 1 000 mm³, axis-aligned, from one
/// fixed seed. The side therefore grows as `n^(1/3)`, so the physics per
/// filament stays the same across a sweep and a sub-quadratic method must
/// scale about linearly.
///
/// The density rule lives here rather than at each call site because the
/// pFFT set-up-cost table (`fasterhenry/src/pfft/mod.rs`), the `1e-6`
/// accuracy bound asserted in `fasterhenry/tests/pfft.rs` and the
/// `pfft_setup` numbers of `fasterhenry/benches/pfft.rs` are only
/// comparable to each other while all three run on the same geometry.
/// `profile::cloud` in `fasterhenry/src/pfft/near.rs` is the one copy that
/// cannot read this definition — see the note there.
pub fn cloud(n: usize) -> Vec<Filament> {
    random_set(n, cloud_side(n), true, 11)
}

/// Side of the cube [`cloud`] scatters `n` filaments through, from the
/// constant density of 400 filaments per 1 000 mm³ every pFFT measurement
/// is taken at: `n^(1/3)` growth, so the physics per filament does not
/// change as a sweep gets larger.
///
/// Separate from [`cloud`] because the accuracy tests in
/// `fasterhenry/tests/pfft.rs` need this density at their own seeds.
pub fn cloud_side(n: usize) -> f64 {
    10e-3 * (n as f64 / 400.0).cbrt()
}
