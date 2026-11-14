//! How far does the extracted impedance move under filament refinement at
//! 1 THz? (issue #353)
//!
//! `docs/benchmarks.md` § "FastHenry example corpus" records a worst-case
//! relative Frobenius disagreement of 1.2e-2 with FastHenry, at 1 THz on a
//! 735-filament pin deck, and says it has not been attributed yet. Part of
//! that figure can be bounded without running FastHenry at all: *this*
//! engine's own discretization error at that frequency. This file measures
//! it — a self-consistency study of one engine under refinement, so it needs
//! no external oracle.
//!
//! # Fixture
//!
//! Authored from scratch for this measurement, not reconstructed from any
//! FastHenry deck (`CONTRIBUTING.md` § "The one hard rule: clean room" bans
//! that outright): three parallel copper pins (σ = 5.8e7 S/m), 5 mm long,
//! 200 µm × 200 µm square cross-section, on a 600 µm pitch, one port across
//! each pin's two ends. The port impedance matrix is therefore 3 × 3 and
//! genuinely multi-conductor — the existing graded/skin gates
//! (`skin_effect_validation.rs`, `width_graded_skin_validation.rs`) are
//! single-conductor comparisons against external references, and none of
//! them says what refinement does to the *mutual* terms.
//!
//! Three pins at 16 filaments per axis is 768 filaments, which puts the
//! coarsest level of the asserted ladder in the corpus deck's class (735).
//! That matters: the question `docs/validation.md` answers is about the
//! corpus deck's resolution, so the coarse end of the ladder has to sit
//! there.
//!
//! # What is measured
//!
//! Two ladders, each solved at 1 THz and at 1 kHz for contrast (both
//! frequencies in a single [`solve`] call per level — assembly dominates and
//! the second frequency is nearly free):
//!
//! * **graded**, `Discretization::graded(n, n, 2.0)` at `n = 16, 20, 24, 28`
//!   — the arm the assertion applies to;
//! * **uniform**, `Discretization::uniform(n, n)` at `n = 8, 12, 16` — for
//!   contrast only. At 1 THz the skin depth is 66 nm on a 200 µm conductor
//!   (`side/δ = 3026`), so a uniform grid would need thousands of filaments
//!   per axis to resolve it; asserting *convergence* on this arm would be a
//!   test that must fail. What is asserted instead is that it has visibly
//!   **not** converged, which is the fact the documentation states.
//!
//! Three relative changes per step, each between a level and the next finer
//! one, which is used as the reference:
//!
//! * `dFrob` — the full complex matrix, by the metric
//!   `tools/fasthenry_compare.py:107` (`rel_frobenius`) uses, so the result
//!   is directly comparable with the 1.2e-2 corpus figure it bounds;
//! * `dR` and `dX` — the same metric restricted to the real and imaginary
//!   parts.
//!
//! All three are needed because at 1 THz the port impedance is
//! reactance-dominated (`X₀₀/R₀₀ ≈ 1e4` here), so `dFrob ≈ dX` and the
//! combined metric is nearly blind to the resistance — which is the only
//! part refinement meaningfully moves at this frequency. A bound on `dFrob`
//! alone would pass for the wrong reason; the table and
//! `docs/validation.md` report the parts separately, and the test asserts
//! the blindness itself (`dR > 10·dFrob` on the last step) so the documented
//! sentence cannot silently stop being true.
//!
//! # Why the ladder is what it is
//!
//! * **The grading validity floor binds.** `filament/grading.rs` rejects any
//!   cell below `max(1e-7·max(l, w, h), 32·ε·coordinate_scale)`, which is
//!   5e-10 m for a 5 mm pin. At `ratio = 3` that rules out more than about
//!   20 filaments per axis on this cross-section; `ratio = 2` stays above
//!   the floor past 32. The ladder is checked against
//!   [`graded_surface_extent`] and that arithmetic *before* any solve, so a
//!   level that cannot assemble is a clear failure here rather than a
//!   `SolveError::Discretize` from inside CI.
//! * **The counts are explicit, never `Discretization::SkinDepth`.** That
//!   variant picks counts itself and caps them at
//!   `SkinDepthGrading::DEFAULT_MAX_PER_AXIS` (32), so two nominally
//!   successive levels could saturate to the same grid and report a
//!   spurious zero change. What it *would* have chosen at 1 THz is printed
//!   for the record.
//!
//! # Running
//!
//! `#[ignore]`d, for the reason `perf_smoke.rs` is: seven dense solves of up
//! to 2 352 filaments in an unoptimized build are one to two orders of
//! magnitude too slow to be worth anyone's CI minutes. CI runs it explicitly
//! in release mode, and counts the `REFINEMENT GATE PASSED` marker:
//!
//! ```bash
//! cargo test --release -p fasterhenry --test refinement_study \
//!   -- --ignored --nocapture
//! ```
//!
//! There is deliberately **no wall-clock budget** here — the assertions are
//! on convergence, not on speed, so a busy runner cannot make it flaky.
//! Times are printed because they are what the next person sizing this
//! study needs. The committed numbers are in `docs/validation.md`
//! § "High-frequency refinement (issue #353)".

use std::time::Instant;

use nalgebra::DMatrix;
use num_complex::Complex;

use fasterhenry::{
    filament::graded_surface_extent, skin_depth, solve, Discretization, Geometry, Node, NodeId,
    Port, SegmentDef, SkinDepthGrading, SweepResult,
};

/// Copper conductivity, siemens per metre.
const COPPER: f64 = 5.8e7;
/// Pins in the bundle.
const PINS: usize = 3;
/// Pin length, metres.
const LENGTH: f64 = 5e-3;
/// Pin cross-section side, metres (square).
const SIDE: f64 = 200e-6;
/// Centre-to-centre pin spacing, metres.
const PITCH: f64 = 600e-6;

/// The frequency under study.
const F_HIGH: f64 = 1e12;
/// The contrast frequency: a skin depth 10× the conductor, where every grid
/// sees the same near-uniform current and the ladder itself must contribute
/// nothing.
const F_LOW: f64 = 1e3;

/// Inward coarsening ratio of the graded ladder. `3.0` would cross the
/// validity floor before the top of the ladder; see the module docs.
const RATIO: f64 = 2.0;
/// The graded ladder. 16 per axis is 768 filaments on three pins — the
/// corpus deck's class.
const GRADED_LEVELS: [usize; 4] = [16, 20, 24, 28];
/// The uniform ladder, for contrast. Its finest level has the same filament
/// count as the graded ladder's coarsest, so the two arms can be compared at
/// equal cost.
const UNIFORM_LEVELS: [usize; 3] = [8, 12, 16];

/// Mirror of `MIN_RELATIVE_EXTENT` in `fasterhenry/src/filament/grading.rs`.
/// Duplicated rather than imported because it is private there, and this
/// study measures the engine as it is rather than widening its API to read
/// a constant. If the engine's value moves, the ladder check below stops
/// *predicting* the floor but the ladder still cannot silently break it: a
/// level the engine rejects fails the `solve` call outright.
const MIN_RELATIVE_EXTENT: f64 = 1e-7;

/// The bound asserted on the graded ladder's last refinement step at 1 THz,
/// in the combined relative Frobenius metric. Measured at 1.7e-6 (see
/// `docs/validation.md`); the bound is two orders of magnitude above that so
/// it guards the headline instead of re-deriving it, and is itself nearly
/// four orders below the 1.2e-2 corpus figure this bounds.
const GRADED_FROBENIUS_BOUND: f64 = 1e-4;
/// The same for 1 kHz, where refinement must do essentially nothing.
///
/// Measured at most 5.3e-7, on the *finest* step: at a skin depth ten times
/// the conductor every grid sees the same near-uniform current, so what is
/// left is rounding in a grid whose cells span 6 nm to 60 µm, and it grows
/// with the grading's extremity rather than shrinking. The bound keeps
/// twenty-fold headroom over that for the platforms CI does not run this on,
/// and is still three orders below the 1.2e-2 figure the study bounds.
const LOW_FREQUENCY_BOUND: f64 = 1e-5;

/// Positive marker CI counts, so a test that did not run cannot look like
/// one that passed. See `docs/validation.md` § "How CI asserts a gate
/// actually ran".
const PASSED: &str = "REFINEMENT GATE PASSED";

/// Three parallel pins and one port across each.
fn pin_bundle() -> (Geometry, Vec<Port>) {
    let mut geometry = Geometry::new();
    for pin in 0..PINS {
        let y = pin as f64 * PITCH;
        geometry
            .add_node(Node::new(0.0, y, 0.0))
            .expect("the near end of a pin is a finite node");
        geometry
            .add_node(Node::new(LENGTH, y, 0.0))
            .expect("the far end of a pin is a finite node");
    }
    for pin in 0..PINS {
        geometry
            .add_segment(SegmentDef::new(
                NodeId(2 * pin),
                NodeId(2 * pin + 1),
                SIDE,
                SIDE,
                COPPER,
            ))
            .expect("a straight copper pin is a valid segment");
    }
    let ports = (0..PINS)
        .map(|pin| Port::new(NodeId(2 * pin), NodeId(2 * pin + 1)).named(format!("pin {pin}")))
        .collect();
    (geometry, ports)
}

/// `‖ours − reference‖_F / ‖reference‖_F` over the entries selected by
/// `part`.
///
/// With the identity `part` this is exactly `rel_frobenius` from
/// `tools/fasthenry_compare.py:107` — sum of
/// `|a − b|²` over the matrix, divided by the sum of `|b|²`, square-rooted —
/// which is the metric behind the corpus table's agreement column, so the
/// numbers here are comparable with it.
fn rel_frobenius_part(
    ours: &DMatrix<Complex<f64>>,
    reference: &DMatrix<Complex<f64>>,
    part: impl Fn(Complex<f64>) -> Complex<f64>,
) -> f64 {
    assert_eq!(ours.shape(), reference.shape(), "ports must line up");
    let numerator: f64 = ours
        .iter()
        .zip(reference.iter())
        .map(|(&a, &b)| (part(a) - part(b)).norm_sqr())
        .sum();
    let denominator: f64 = reference.iter().map(|&b| part(b).norm_sqr()).sum();
    if denominator == 0.0 {
        f64::NAN
    } else {
        (numerator / denominator).sqrt()
    }
}

/// The three relative changes from `coarse` to `fine`, with the finer level
/// — the better approximation — as the reference, matching the orientation
/// `rel_frobenius(ours, ref)` is used in for the corpus comparison.
fn changes(coarse: &DMatrix<Complex<f64>>, fine: &DMatrix<Complex<f64>>) -> (f64, f64, f64) {
    (
        rel_frobenius_part(coarse, fine, |z| z),
        rel_frobenius_part(coarse, fine, |z| Complex::new(z.re, 0.0)),
        rel_frobenius_part(coarse, fine, |z| Complex::new(z.im, 0.0)),
    )
}

/// One solved refinement level.
struct Level {
    /// Filaments per axis.
    n: usize,
    /// Filaments in the whole problem.
    filaments: usize,
    /// Wall-clock of the one `solve` call, seconds.
    wall_s: f64,
    /// Port impedance at each of `[F_HIGH, F_LOW]`.
    z: Vec<DMatrix<Complex<f64>>>,
}

/// Solves `levels` of one ladder, both frequencies in a single call each.
fn ladder(
    label: &str,
    levels: &[usize],
    discretization: impl Fn(usize) -> Discretization,
) -> Vec<Level> {
    let (geometry, ports) = pin_bundle();
    levels
        .iter()
        .map(|&n| {
            let discretization = discretization(n);
            let filaments = discretization
                .filament_count(&geometry)
                .expect("the discretization sizes the fixture");
            let start = Instant::now();
            let result: SweepResult = solve(&geometry, &ports, &discretization, &[F_HIGH, F_LOW])
                .unwrap_or_else(|error| panic!("{label} level n = {n} solves: {error}"));
            let wall_s = start.elapsed().as_secs_f64();
            assert_eq!(
                result.provenance.counts.filaments, filaments,
                "filament_count must agree with what assembly cut"
            );
            Level {
                n,
                filaments,
                wall_s,
                z: result.impedance_ohm,
            }
        })
        .collect()
}

/// Prints one ladder's table at frequency index `f`, and returns the
/// per-step changes (one shorter than `levels`).
fn report(label: &str, levels: &[Level], f: usize, frequency_hz: f64) -> Vec<(f64, f64, f64)> {
    println!(
        "\n{label}, {frequency_hz:.3e} Hz (R00/X00 are port 0's diagonal entry; \
         the deltas are over the whole {PINS} x {PINS} matrix, vs. the next finer level):"
    );
    println!(
        "{:>4}  {:>9}  {:>8}  {:>14}  {:>14}  {:>9}  {:>9}  {:>9}",
        "n", "filaments", "wall (s)", "R00 (ohm)", "X00 (ohm)", "dFrob", "dR", "dX"
    );
    let mut steps = Vec::new();
    for (index, level) in levels.iter().enumerate() {
        let z = &level.z[f];
        let (frob, real, imag) = if index == 0 {
            (f64::NAN, f64::NAN, f64::NAN)
        } else {
            let step = changes(&levels[index - 1].z[f], z);
            steps.push(step);
            step
        };
        let cell = |value: f64| {
            if value.is_nan() {
                "       --".to_owned()
            } else {
                format!("{value:9.3e}")
            }
        };
        println!(
            "{:>4}  {:>9}  {:>8.1}  {:>14.6e}  {:>14.6e}  {}  {}  {}",
            level.n,
            level.filaments,
            level.wall_s,
            z[(0, 0)].re,
            z[(0, 0)].im,
            cell(frob),
            cell(real),
            cell(imag),
        );
    }
    let (first, last) = (&levels[0], levels.last().expect("a non-empty ladder"));
    let (frob, real, imag) = changes(&first.z[f], &last.z[f]);
    println!(
        "coarsest to finest (n = {} vs. n = {}): dFrob = {frob:.3e}, dR = {real:.3e}, dX = {imag:.3e}",
        first.n, last.n
    );
    steps
}

/// The whole study: both ladders, both frequencies, the table that goes into
/// `docs/validation.md`, and the assertions behind the sentence it states.
#[test]
#[ignore = "release-mode refinement study; see the module documentation"]
fn refinement_at_one_terahertz() {
    let (geometry, _) = pin_bundle();
    let delta_high = skin_depth(F_HIGH, COPPER);
    let delta_low = skin_depth(F_LOW, COPPER);
    println!(
        "{PINS} parallel copper pins, {:.3} mm long, {:.0} µm × {:.0} µm, {:.0} µm pitch; \
         skin depth {delta_high:.4e} m at {F_HIGH:.0e} Hz (side/delta = {:.0}), \
         {delta_low:.4e} m at {F_LOW:.0e} Hz (side/delta = {:.3})",
        LENGTH * 1e3,
        SIDE * 1e6,
        SIDE * 1e6,
        PITCH * 1e6,
        SIDE / delta_high,
        SIDE / delta_low,
    );

    // --- The ladder is legal before anything is solved ------------------
    //
    // `grading.rs` floors a cell at `max(1e-7·max(l, w, h), 32·ε·scale)`,
    // where the second term uses an absolute-coordinate envelope. Bounding
    // that envelope from above by the fixture's extent makes `floor` an
    // upper bound on the engine's own floor, so clearing it is sufficient.
    let shape_scale = LENGTH.max(SIDE);
    let coordinate_bound = LENGTH + PITCH * PINS as f64 + SIDE;
    let floor = (MIN_RELATIVE_EXTENT * shape_scale).max(32.0 * f64::EPSILON * coordinate_bound);
    println!(
        "grading validity floor <= {floor:.3e} m (1e-7 × {shape_scale:.3e} m shape scale, \
         32ε × {coordinate_bound:.3e} m coordinate bound)"
    );
    for &n in &GRADED_LEVELS {
        let surface = graded_surface_extent(SIDE, n, RATIO).unwrap_or_else(|error| {
            panic!("graded({n}, {n}, {RATIO}) has a surface cell: {error}")
        });
        println!("  graded n = {n:>2}, ratio {RATIO}: surface cell {surface:.3e} m");
        assert!(
            surface > floor,
            "graded n = {n} at ratio {RATIO} puts a {surface:e} m cell under the {floor:e} m \
             validity floor: pick a coarser ladder or a smaller ratio, do not change the floor"
        );
    }

    // For the record: what `Discretization::SkinDepth` would have chosen at
    // 1 THz, and that it is not saturating its per-axis cap here (a cap hit
    // would make two nominal levels identical — which is exactly why this
    // study drives explicit counts instead).
    let automatic = Discretization::skin_depth(F_HIGH, 0.5, RATIO)
        .filament_count(&geometry)
        .expect("the skin-depth discretization sizes the fixture");
    let cap = PINS * SkinDepthGrading::DEFAULT_MAX_PER_AXIS.pow(2);
    println!(
        "SkinDepthGrading(1 THz, 0.5 delta, ratio {RATIO}) would choose {automatic} filaments \
         ({:.0} per axis); the per-axis cap of {} would be {cap}",
        ((automatic / PINS) as f64).sqrt(),
        SkinDepthGrading::DEFAULT_MAX_PER_AXIS,
    );
    assert!(
        automatic < cap,
        "SkinDepthGrading saturated its {} per-axis cap on this fixture ({automatic} filaments): \
         the printed figure would no longer mean what the documentation says it does",
        SkinDepthGrading::DEFAULT_MAX_PER_AXIS,
    );

    // --- The two ladders ------------------------------------------------
    let graded = ladder("graded", &GRADED_LEVELS, |n| {
        Discretization::graded(n, n, RATIO)
    });
    let uniform = ladder("uniform", &UNIFORM_LEVELS, |n| {
        Discretization::uniform(n, n)
    });

    let graded_high = report("graded (ratio 2)", &graded, 0, F_HIGH);
    let graded_low = report("graded (ratio 2)", &graded, 1, F_LOW);
    let uniform_high = report("uniform (contrast only)", &uniform, 0, F_HIGH);
    report("uniform (contrast only)", &uniform, 1, F_LOW);

    // --- The coarsest level is the corpus deck's resolution -------------
    assert_eq!(
        graded[0].filaments,
        PINS * GRADED_LEVELS[0].pow(2),
        "the ladder's filament counts are n² per pin"
    );
    assert!(
        (600..=900).contains(&graded[0].filaments),
        "the coarsest graded level holds {} filaments, outside the 735-filament corpus deck's \
         class: the documented answer would then be about the wrong resolution",
        graded[0].filaments
    );

    // --- The graded arm has converged, in the corpus metric -------------
    let (frob, real, imag) = *graded_high.last().expect("at least two graded levels");
    println!(
        "\ngraded last step at {F_HIGH:.0e} Hz: dFrob = {frob:.3e} (bound {GRADED_FROBENIUS_BOUND:.0e}), \
         dR = {real:.3e}, dX = {imag:.3e}"
    );
    assert!(
        frob < GRADED_FROBENIUS_BOUND,
        "the graded ladder's last refinement step moves Z by {frob:e} in the relative Frobenius \
         metric at {F_HIGH:e} Hz, over the stated {GRADED_FROBENIUS_BOUND:e} bound"
    );

    // --- …and the metric is nearly blind to the resistance --------------
    //
    // The documented sentence turns on this: `dFrob` being small does NOT
    // say the resistance has converged, because `X/R ≈ 1e4` at 1 THz makes
    // `dFrob ≈ dX`. Asserted so the claim cannot quietly stop being true.
    let z = &graded[0].z[0][(0, 0)];
    let reactive = z.im.abs() / z.re.abs();
    println!(
        "reactance/resistance at the coarsest graded level, {F_HIGH:.0e} Hz: X00/R00 = {reactive:.3e}; \
         dR/dFrob on the last step = {:.3e}",
        real / frob
    );
    assert!(
        reactive > 1e2,
        "X00/R00 = {reactive:e} at {F_HIGH:e} Hz: the port is no longer reactance-dominated, so \
         the documented reason the combined metric hides the resistance no longer holds"
    );
    assert!(
        real > 10.0 * frob,
        "the real part moved by {real:e} and the combined metric by {frob:e} on the last graded \
         step: the combined metric is no longer blind to the resistance, and the sentence in \
         docs/validation.md needs rewriting rather than this bound relaxing"
    );

    // --- The low-frequency arm shows the ladder adds nothing ------------
    for (step, &(frob, real, imag)) in graded_low.iter().enumerate() {
        assert!(
            frob < LOW_FREQUENCY_BOUND && real < LOW_FREQUENCY_BOUND && imag < LOW_FREQUENCY_BOUND,
            "graded step {step} at {F_LOW:e} Hz moves Z by dFrob = {frob:e}, dR = {real:e}, \
             dX = {imag:e}: at a skin depth 10× the conductor every level must agree, so a change \
             this large is the ladder itself contributing, not physics"
        );
    }

    // --- The uniform arm has NOT converged, and must not be asserted on -
    let (_, uniform_real, _) = *uniform_high.last().expect("at least two uniform levels");
    let finest_uniform_r = uniform.last().expect("a uniform ladder").z[0][(0, 0)].re;
    let converged_r = graded.last().expect("a graded ladder").z[0][(0, 0)].re;
    println!(
        "uniform arm at {F_HIGH:.0e} Hz: R00 = {finest_uniform_r:.6e} ohm at n = {}, against the \
         graded ladder's {converged_r:.6e} ohm (low by {:.0}×), still moving by dR = {uniform_real:.3e} \
         on its last step",
        UNIFORM_LEVELS[UNIFORM_LEVELS.len() - 1],
        converged_r / finest_uniform_r,
    );
    assert!(
        uniform_real > 1e-2,
        "the uniform ladder's last step moved R by only {uniform_real:e}: it appears to be \
         converging at {F_HIGH:e} Hz, which a grid that cannot resolve a {delta_high:e} m skin \
         depth on a {SIDE:e} m conductor cannot be doing — the contrast arm is measuring \
         something other than what this file says it is"
    );
    assert!(
        finest_uniform_r * 10.0 < converged_r,
        "the uniform ladder reaches R00 = {finest_uniform_r:e} ohm against the graded \
         {converged_r:e} ohm: within the 10× this file calls 'cannot resolve the skin depth'"
    );

    println!(
        "\n{PASSED} ({} levels graded, {} uniform)",
        GRADED_LEVELS.len(),
        UNIFORM_LEVELS.len()
    );
}
