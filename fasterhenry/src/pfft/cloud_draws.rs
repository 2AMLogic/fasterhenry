//! The random-filament draw sequence every pFFT measurement runs on: the
//! LCG, the constant-density cube rule and the bar geometry they draw.
//!
//! This file is deliberately self-contained — no `crate::` or
//! `fasterhenry::` paths, no `cfg` attributes — so the same bytes compile
//! unchanged in every crate that needs the sequence:
//!
//! * the library's test build, where `fasterhenry/src/pfft/near.rs`
//!   includes it as `#[cfg(test)] #[path = "cloud_draws.rs"] mod
//!   cloud_draws;` for the `#[ignore]`d `setup_phase_breakdown` profile,
//!   and
//! * the bench and integration-test crates, where
//!   `fasterhenry/benches/support.rs` includes it inward with
//!   `#[path = "../src/pfft/cloud_draws.rs"]` and builds `random_set`
//!   from [`bars`].
//!
//! The include direction is what makes this publishable
//! (`fasterhenry/Cargo.toml` packages `src/**/*.rs` only): this file ships
//! in the crate tarball while `benches/` — the includer — does not, so
//! `cargo test` inside an extracted tarball still builds. A `#[path]`
//! reaching the other way, out of published source into `benches/`, would
//! compile locally but leave that tarball unbuildable (issue #364, step 4).
//!
//! The set-up-cost table in `fasterhenry/src/pfft/mod.rs`, the `1e-6`
//! accuracy bounds asserted in `fasterhenry/tests/pfft.rs` and the
//! `pfft_setup` numbers of `fasterhenry/benches/pfft.rs` are comparable to
//! each other only while they all run on the same geometry; with one draw
//! sequence there is no second copy that could drift (issue #372).

/// Copper conductivity, siemens per metre (matches `benches/kernels.rs`
/// and `tests/perf_smoke.rs`, which keep their own constants).
pub const COPPER: f64 = 5.8e7;

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

/// Side of the cube the pFFT filament cloud scatters `n` filaments
/// through, from the constant density of 400 filaments per 1 000 mm³
/// every pFFT measurement is taken at: `n^(1/3)` growth, so the physics
/// per filament does not change as a sweep gets larger.
///
/// Separate from the cloud itself because the accuracy tests in
/// `fasterhenry/tests/pfft.rs` need this density at their own seeds.
pub fn cloud_side(n: usize) -> f64 {
    10e-3 * (n as f64 / 400.0).cbrt()
}

/// One drawn bar as plain numbers: endpoints and cross-section, with the
/// material left to whichever crate turns it into a filament.
pub struct Bar {
    pub a: [f64; 3],
    pub b: [f64; 3],
    pub width: f64,
    pub height: f64,
}

/// `n` bars of length 0.5–1.5 mm scattered in a cube of side `side`:
/// axis-aligned (random axis and sense) when `manhattan`, else in general
/// position. Cross-sections 0.1–0.3 × 0.05–0.15 mm.
pub fn bars(n: usize, side: f64, manhattan: bool, seed: u64) -> Vec<Bar> {
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
            Bar {
                a,
                b,
                width,
                height,
            }
        })
        .collect()
}
