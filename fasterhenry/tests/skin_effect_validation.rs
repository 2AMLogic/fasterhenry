//! Validation of the skin-depth-graded filament grid against the analytic
//! skin effect, and of graded grids against fine uniform ones.
//!
//! No tabulated values are hard-coded. The references are:
//!
//! * [`slab_internal_impedance`] — the textbook one-dimensional solution for
//!   a conductor of thickness `t` whose two faces see equal and opposite
//!   magnetic fields, `Z_int = l·k/(2σw)·coth(k·t/2)` with `k = (1+j)/δ`.
//!   Its limits are the two classical results this file asserts: a DC
//!   resistance `l/(σwt)` with an internal inductance `μ0·l·t/(12w)`, and a
//!   high-frequency `R = l/(2σwδ)` with an internal inductance `μ0·l·δ/(4w)`
//!   — the `∝ δ` law of Rosa's internal-inductance analysis, here for a
//!   slab rather than a round wire.
//! * [`fasterhenry::inductance::closed_form::rectangular_bar_self`] — the
//!   exact uniform-current partial self-inductance, which the solver must
//!   reproduce at DC whatever grid it is given, because a bundle of
//!   filaments tiling a bar at uniform current density *is* the bar.
//!
//! The comparison against the one-dimensional reference constrains the
//! model to depth redistribution: a wide trace (`w = 200·t`) cut with
//! `nw = 1`, so every filament spans the full width and the only
//! redistribution available to the current is across the thickness. The
//! remaining discrepancies are the finite length and width of the trace,
//! which remain measurable at these aspect ratios.
//!
//! Run with `cargo test -p fasterhenry --test skin_effect_validation --
//! --nocapture` to see the measured numbers.

use num_complex::Complex;

use fasterhenry::inductance::closed_form;
use fasterhenry::{
    skin_depth, solve, Discretization, Geometry, Node, Port, SegmentDef, SkinDepthGrading, MU0,
};

const COPPER: f64 = 5.8e7;

/// A straight rectangular trace along `+x`: width along `+y`, thickness
/// along `+z`.
#[derive(Clone, Copy, Debug)]
struct Trace {
    length: f64,
    width: f64,
    thickness: f64,
    sigma: f64,
}

impl Trace {
    /// `Z(f)` at every frequency, in ohms, on the given grid.
    fn impedance(&self, discretization: &Discretization, frequencies: &[f64]) -> Vec<Complex<f64>> {
        let (geometry, ports) = self.build();
        let result = solve(&geometry, &ports, discretization, frequencies).expect("solvable");
        result
            .impedance_ohm
            .iter()
            .map(|z| z[(0, 0)])
            .collect::<Vec<_>>()
    }

    /// Filament count of the assembled system on the given grid.
    fn filament_count(&self, discretization: &Discretization) -> usize {
        let (geometry, ports) = self.build();
        fasterhenry::MeshSystem::assemble(&geometry, &ports, discretization)
            .expect("assembles")
            .counts()
            .filaments
    }

    fn build(&self) -> (Geometry, Vec<Port>) {
        let mut geometry = Geometry::new();
        let a = geometry.add_node(Node::new(0.0, 0.0, 0.0)).expect("node");
        let b = geometry
            .add_node(Node::new(self.length, 0.0, 0.0))
            .expect("node");
        geometry
            .add_segment(SegmentDef::new(
                a,
                b,
                self.width,
                self.thickness,
                self.sigma,
            ))
            .expect("segment");
        (geometry, vec![Port::new(a, b)])
    }

    /// Skin depth at `frequency`, in metres.
    fn delta(&self, frequency: f64) -> f64 {
        skin_depth(frequency, self.sigma)
    }

    /// The frequency at which the skin depth is `delta`.
    fn frequency_for_delta(&self, delta: f64) -> f64 {
        1.0 / (std::f64::consts::PI * MU0 * self.sigma * delta * delta)
    }

    /// The analytic internal impedance of the equivalent slab.
    fn analytic(&self, frequency: f64) -> Complex<f64> {
        slab_internal_impedance(
            frequency,
            self.length,
            self.width,
            self.thickness,
            self.sigma,
        )
    }

    /// The analytic internal inductance of the equivalent slab, in henries.
    fn analytic_internal_inductance(&self, frequency: f64) -> f64 {
        if frequency == 0.0 {
            return MU0 * self.length * self.thickness / (12.0 * self.width);
        }
        self.analytic(frequency).im / (std::f64::consts::TAU * frequency)
    }
}

// ---------------------------------------------------------------------------
// The independent reference
// ---------------------------------------------------------------------------

/// Internal impedance of an isolated conductor of thickness `t`, width `w`
/// and length `l`, in the one-dimensional limit `w ≫ t`.
///
/// With the current along `x` and the thickness along `z`, the diffusion
/// equation `J'' = k²J`, `k = (1+j)/δ`, and the symmetry of an isolated
/// conductor (`H = ∓I/2w` on the two faces) give `J(z) = J₀·cosh(kz)`. The
/// surface electric field over the total current is then
///
/// ```text
/// Z_int = l · k/(2σw) · coth(k·t/2) .
/// ```
///
/// Its series at small `k·t` is `l/(σwt) + jω·μ0·l·t/(12w)` — the DC
/// resistance and the classical thin-slab internal inductance — and its
/// limit at large `k·t` is `(1+j)·l/(2σwδ)`, the two-skin-layer resistance
/// with an internal inductance `μ0·l·δ/(4w)` that vanishes `∝ δ`.
fn slab_internal_impedance(frequency: f64, l: f64, w: f64, t: f64, sigma: f64) -> Complex<f64> {
    if frequency == 0.0 {
        return Complex::new(l / (sigma * w * t), 0.0);
    }
    let delta = skin_depth(frequency, sigma);
    let k = Complex::new(1.0, 1.0) / delta;
    let coth = (k * (0.5 * t)).tanh().inv();
    k * coth * (l / (2.0 * sigma * w))
}

/// Relative difference of two impedances, `|a − b| / |b|`.
fn relative(a: Complex<f64>, b: Complex<f64>) -> f64 {
    (a - b).norm() / b.norm()
}

// ---------------------------------------------------------------------------
// The fixtures
// ---------------------------------------------------------------------------

/// A wide, thin copper trace: 400 mm long, 40 mm wide, 200 µm thick. `w/t` is
/// 200 and `l/w` is 10, so the one-dimensional slab solution applies to a few
/// per cent, and the skin depth passes through the thickness in the low
/// megahertz.
fn wide_trace() -> Trace {
    Trace {
        length: 400e-3,
        width: 40e-3,
        thickness: 200e-6,
        sigma: COPPER,
    }
}

/// A square-section copper trace, 2 mm long and 200 µm on a side — the
/// round-trip fixture, where both cross-section axes are subdivided.
fn square_trace() -> Trace {
    Trace {
        length: 2e-3,
        width: 200e-6,
        thickness: 200e-6,
        sigma: COPPER,
    }
}

// ---------------------------------------------------------------------------
// Skin-effect impedance
// ---------------------------------------------------------------------------

/// A graded grid must reproduce the analytic skin-effect impedance of a wide
/// trace where the asymptotics apply, `t/δ = 5 … 20`, to 5 %.
#[test]
fn graded_grid_matches_the_analytic_skin_impedance() {
    let trace = wide_trace();
    // 14 filaments at 2:1 across the thickness: weights 1,2,…,64,64,…,2,1
    // sum to 254, so the surface filaments are t/254 = 0.79 µm thick — a
    // twelfth of the smallest skin depth below, where a uniform grid of the
    // same resolution would need 254 filaments.
    let grid = Discretization::graded(1, 14, 2.0);
    let ratios = [5.0, 10.0, 20.0];
    let frequencies: Vec<f64> = ratios
        .iter()
        .map(|per_delta| trace.frequency_for_delta(trace.thickness / per_delta))
        .collect();
    let modelled = trace.impedance(&grid, &frequencies);

    println!(
        "\n--- wide trace, graded 1 × 14 @ 2:1 ({} filaments) ---",
        14
    );
    for ((&frequency, z), per_delta) in frequencies.iter().zip(&modelled).zip(ratios) {
        let reference = trace.analytic(frequency);
        let (dr, di) = (
            (z.re - reference.re) / reference.re,
            (z.im - reference.im) / reference.im,
        );
        println!(
            "t/δ = {per_delta:4}  f = {:9.3e} Hz  R = {:.6e} Ω (analytic {:.6e}, {:+.2} %)  \
             X_int = {:.6e} Ω (analytic {:.6e}, {:+.2} %)",
            frequency,
            z.re,
            reference.re,
            100.0 * dr,
            z.im,
            reference.im,
            100.0 * di
        );
        assert!(
            dr.abs() < 0.05,
            "R at t/δ = {per_delta} is {dr:+.4} off the analytic skin-effect value"
        );
    }

    // …and the resistance must actually be following the √f law by then:
    // R(t/δ = 20) / R(t/δ = 5) is 4 for the two-skin-layer asymptote.
    let growth = modelled[2].re / modelled[0].re;
    println!("R(t/δ=20)/R(t/δ=5) = {growth:.4} (asymptotically 4)");
    assert!((growth - 4.0).abs() < 0.05 * 4.0);
}

/// The internal inductance must fall from the DC value `μ0·l·t/(12w)` toward
/// the `μ0·l·δ/(4w)` of a current confined to two skin layers — Rosa's
/// `∝ δ` internal-inductance transition, for a slab.
///
/// Only *differences* of the port inductance are meaningful here: the trace's
/// external inductance is two orders of magnitude larger and is what the
/// closed-form anchor below checks instead.
#[test]
fn internal_inductance_follows_the_skin_depth() {
    let trace = wide_trace();
    let grid = Discretization::graded(1, 14, 2.0);

    // 1 Hz: δ = 66 mm, 330 thicknesses — DC as far as the current
    // distribution is concerned, but with an imaginary part to read.
    let reference_frequency = 1.0;
    let per_delta = [2.0, 5.0, 10.0, 20.0];
    let mut frequencies = vec![reference_frequency];
    frequencies.extend(
        per_delta
            .iter()
            .map(|p| trace.frequency_for_delta(trace.thickness / p)),
    );
    let modelled = trace.impedance(&grid, &frequencies);
    let inductance = |z: Complex<f64>, f: f64| z.im / (std::f64::consts::TAU * f);

    let l_reference = inductance(modelled[0], reference_frequency);
    let l_int_reference = trace.analytic_internal_inductance(reference_frequency);
    println!("\n--- internal inductance, wide trace ---");
    println!(
        "L({reference_frequency} Hz) = {:.6e} H, of which {:.4e} H is internal (analytic)",
        l_reference, l_int_reference
    );

    for (&frequency, z) in frequencies[1..].iter().zip(&modelled[1..]) {
        let delta = trace.delta(frequency);
        // Anchoring the fall of the port inductance at the low-frequency end
        // turns it into the internal inductance itself.
        let modelled_internal =
            inductance(*z, frequency) - l_reference + trace.analytic_internal_inductance(0.0);
        let analytic_internal = trace.analytic_internal_inductance(frequency);
        let thin_skin = MU0 * trace.length * delta / (4.0 * trace.width);
        println!(
            "t/δ = {:5.1}  L_int = {:.5e} H  (analytic {:.5e}, {:+.2} %;  μ0·l·δ/4w = {:.5e})",
            trace.thickness / delta,
            modelled_internal,
            analytic_internal,
            100.0 * (modelled_internal - analytic_internal) / analytic_internal,
            thin_skin
        );
        assert!(
            (modelled_internal - analytic_internal).abs() < 0.15 * analytic_internal,
            "internal inductance at t/δ = {:.1} is off the analytic value",
            trace.thickness / delta
        );
    }

    // The two deepest points must sit on the ∝ δ asymptote itself.
    for (&frequency, z) in frequencies[3..].iter().zip(&modelled[3..]) {
        let modelled_internal =
            inductance(*z, frequency) - l_reference + trace.analytic_internal_inductance(0.0);
        let thin_skin = MU0 * trace.length * trace.delta(frequency) / (4.0 * trace.width);
        assert!(
            (modelled_internal - thin_skin).abs() < 0.15 * thin_skin,
            "internal inductance at {frequency} Hz is not μ0·l·δ/(4w)"
        );
    }
}

/// Anchor for the absolute level of the inductance: at DC the current
/// density is uniform whatever the grid, and a bundle of filaments tiling a
/// bar at uniform current density *is* the bar — so the port inductance must
/// equal the exact closed-form partial self-inductance of the whole trace.
#[test]
fn low_frequency_inductance_matches_the_closed_form_bar() {
    let trace = wide_trace();
    let reference = closed_form::rectangular_bar_self(trace.length, trace.width, trace.thickness);
    let frequency = 1.0;

    println!("\n--- closed-form anchor, wide trace ---");
    for grid in [
        Discretization::uniform(1, 1),
        Discretization::uniform(2, 8),
        Discretization::graded(1, 14, 2.0),
        Discretization::graded(3, 10, 3.0),
    ] {
        let z = trace.impedance(&grid, &[frequency])[0];
        let modelled = z.im / (std::f64::consts::TAU * frequency);
        let error = (modelled - reference.value) / reference.value;
        println!(
            "{:?}: L = {:.9e} H vs closed form {:.9e} H ({:+.3e})",
            grid, modelled, reference.value, error
        );
        assert!(
            error.abs() < 1e-4,
            "{grid:?}: {modelled:e} vs {:e}",
            reference.value
        );
    }
}

// ---------------------------------------------------------------------------
// Graded vs. fine uniform
// ---------------------------------------------------------------------------

/// A coarse graded grid must agree with a much finer uniform one on `Z`.
///
/// 4 × 4 at 10:1 is 16 filaments against the 256 of a uniform 16 × 16 — a
/// sixteenth of the count — and the two must agree to 1 % at skin depths
/// from one to ten thicknesses.
#[test]
fn graded_and_fine_uniform_grids_agree() {
    let trace = square_trace();
    let uniform = Discretization::uniform(16, 16);
    let graded = Discretization::graded(4, 4, 10.0);

    let fine_count = trace.filament_count(&uniform);
    let graded_count = trace.filament_count(&graded);
    assert_eq!((fine_count, graded_count), (256, 16));
    assert!(graded_count * 4 <= fine_count);

    let frequencies: Vec<f64> = [1.0, 3.0, 10.0]
        .iter()
        .map(|thicknesses| trace.frequency_for_delta(thicknesses * trace.thickness))
        .collect();
    let fine = trace.impedance(&uniform, &frequencies);
    let coarse = trace.impedance(&graded, &frequencies);

    println!("\n--- round trip: uniform 16 × 16 ({fine_count}) vs graded 4 × 4 @ 10:1 ({graded_count}) ---");
    for ((&frequency, a), b) in frequencies.iter().zip(&coarse).zip(&fine) {
        let error = relative(*a, *b);
        println!(
            "δ/t = {:5.1}  f = {:9.3e} Hz  graded {:.6e} vs uniform {:.6e}  ({:.3} %)",
            trace.delta(frequency) / trace.thickness,
            frequency,
            a,
            b,
            100.0 * error
        );
        assert!(
            error < 0.01,
            "graded and uniform differ by {:.3} % at {frequency} Hz",
            100.0 * error
        );
    }
}

/// The skin-depth-adaptive grid must pick its own counts: one filament per
/// axis when the skin depth dwarfs the conductor, more as the frequency
/// rises, and always enough to put the surface filaments inside a skin
/// depth — at a cost that grows logarithmically, not as `√f`.
#[test]
fn skin_depth_grading_sizes_itself_to_the_frequency() {
    let trace = wide_trace();
    println!("\n--- skin-depth-adaptive counts, wide trace ---");

    // DC: nothing to resolve.
    let dc = Discretization::SkinDepth(SkinDepthGrading::new(0.0, 0.5, 2.0));
    assert_eq!(trace.filament_count(&dc), 1);

    let mut previous = 0;
    for per_delta in [0.5, 2.0, 10.0, 100.0] {
        let frequency = trace.frequency_for_delta(trace.thickness / per_delta);
        let delta = trace.delta(frequency);
        let grading = SkinDepthGrading::new(frequency, 0.5, 2.0);
        let count = trace.filament_count(&Discretization::SkinDepth(grading));
        // The uniform grid that would resolve the same surface layer.
        let uniform =
            (trace.width / (0.5 * delta)).ceil() * (trace.thickness / (0.5 * delta)).ceil();
        println!(
            "t/δ = {per_delta:6}  δ = {delta:.3e} m  graded {count:4} filaments  \
             (uniform would need {uniform:.0})"
        );
        assert!(count >= previous, "the count must not fall with frequency");
        assert!(
            (count as f64) <= uniform,
            "the graded grid must never cost more than the uniform one"
        );
        previous = count;
    }

    // Two-dimensional width grading needs a 2D cross-section reference;
    // the one-dimensional slab formula cannot validate its edge currents.
}

/// Grading is an accuracy *lever*: at a fixed filament count it must beat a
/// uniform grid on the skin-effect resistance, and by a wide margin once the
/// skin depth is a small fraction of the thickness.
#[test]
fn grading_beats_a_uniform_grid_at_equal_cost() {
    let trace = wide_trace();
    let frequency = trace.frequency_for_delta(trace.thickness / 10.0);
    let reference = trace.analytic(frequency);

    println!("\n--- accuracy at equal cost, t/δ = 10 ---");
    let mut best_uniform = f64::MAX;
    let mut best_graded = f64::MAX;
    for count in [4, 8, 12, 16] {
        let uniform = trace.impedance(&Discretization::uniform(1, count), &[frequency])[0];
        let graded = trace.impedance(&Discretization::graded(1, count, 2.0), &[frequency])[0];
        let (eu, eg) = (
            ((uniform.re - reference.re) / reference.re).abs(),
            ((graded.re - reference.re) / reference.re).abs(),
        );
        println!(
            "{count:3} filaments: uniform {:+.2} %, graded {:+.2} %",
            100.0 * eu,
            100.0 * eg
        );
        assert!(
            eg < eu,
            "{count} filaments: graded ({eg:.4}) is no better than uniform ({eu:.4})"
        );
        best_uniform = best_uniform.min(eu);
        best_graded = best_graded.min(eg);
    }
    assert!(
        best_graded * 4.0 < best_uniform,
        "grading must be at least 4× more accurate at these counts"
    );
}
