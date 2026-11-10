//! Validation of the filament-current output ([`fasterhenry::currents`]).
//!
//! Oracles: a straight bar at DC carries a uniform `|J| = I/A`; at high
//! frequency a wide slab follows the textbook profile
//! `J(z) = I·k·cosh(kz) / (2w·sinh(kt/2))`, `k = (1+j)/δ` (the same slab
//! model `skin_effect_validation.rs` uses for the impedance); the dissipated
//! power matches the power delivered at the ports; and the dense and
//! matrix-free paths agree.

use num_complex::Complex;

use fasterhenry::{
    skin_depth, unit_excitation, CurrentSolution, Discretization, Geometry, IterativeParams,
    IterativeSystem, MeshSystem, Node, Port, SegmentDef, SolveError,
};

type C64 = Complex<f64>;

const COPPER: f64 = 5.8e7;

fn bar(length: f64, width: f64, height: f64) -> (Geometry, Vec<Port>) {
    let mut g = Geometry::new();
    let a = g.add_node(Node::new(0.0, 0.0, 0.0)).unwrap();
    let b = g.add_node(Node::new(length, 0.0, 0.0)).unwrap();
    g.add_segment(SegmentDef::new(a, b, width, height, COPPER))
        .unwrap();
    (g, vec![Port::new(a, b)])
}

/// A closed square ring of four segments with the port across the first
/// one, so the conductor has an internal loop.
fn ring() -> (Geometry, Vec<Port>) {
    let mut g = Geometry::new();
    let s = 5e-3;
    let n: Vec<_> = [(0.0, 0.0), (s, 0.0), (s, s), (0.0, s)]
        .iter()
        .map(|&(x, y)| g.add_node(Node::new(x, y, 0.0)).unwrap())
        .collect();
    for i in 0..4 {
        g.add_segment(SegmentDef::new(n[i], n[(i + 1) % 4], 1e-3, 0.5e-3, COPPER))
            .unwrap();
    }
    (g, vec![Port::new(n[0], n[1])])
}

fn max_diff(a: &CurrentSolution, b: &CurrentSolution) -> f64 {
    let scale = a
        .filaments
        .iter()
        .map(|f| f.current.norm())
        .fold(0.0, f64::max);
    a.filaments
        .iter()
        .zip(&b.filaments)
        .map(|(x, y)| (x.current - y.current).norm())
        .fold(0.0, f64::max)
        / scale
}

#[test]
fn bar_at_dc_has_uniform_density() {
    let (g, ports) = bar(10e-3, 1e-3, 35e-6);
    let d = Discretization::uniform(5, 3);
    let area = 1e-3 * 35e-6;
    let i = C64::new(2.0, 0.0);
    let dense = MeshSystem::assemble(&g, &ports, &d).unwrap();
    let iter = IterativeSystem::assemble(&g, &ports, &d, &IterativeParams::default()).unwrap();
    for sol in [
        dense.currents(0.0, &[i]).unwrap(),
        iter.currents(0.0, &[i]).unwrap(),
    ] {
        assert_eq!(sol.filaments.len(), 15);
        for f in &sol.filaments {
            assert!((f.current_density - 2.0 / area).abs() < 1e-6 * 2.0 / area);
            assert!(f.current.im.abs() < 1e-12);
        }
        assert!((sol.segments[0].current - i).norm() < 1e-9);
        assert_eq!(sol.conductors.len(), 1);
        assert!((sol.conductors[0].peak_current_density - 2.0 / area).abs() < 1e-6 * 2.0 / area);
        // Power ½ I² R with R = l/(σA): peak-phasor convention.
        let r = 10e-3 / (COPPER * area);
        assert!((sol.power_balance.dissipated_w - 0.5 * 4.0 * r).abs() < 1e-9 * r);
        assert!(sol.power_balance.relative_residual < 1e-9);
    }
}

#[test]
fn wide_slab_follows_the_skin_effect_profile() {
    let (length, width, t) = (100e-3, 40e-3, 200e-6);
    let delta = t / 4.0;
    let f = 1.0 / (std::f64::consts::PI * fasterhenry::MU0 * COPPER * delta * delta);
    assert!((skin_depth(f, COPPER) - delta).abs() < 1e-12);
    let (g, ports) = bar(length, width, t);
    let sys = MeshSystem::assemble(&g, &ports, &Discretization::uniform(1, 40)).unwrap();
    let sol = sys.currents(f, &unit_excitation(1)).unwrap();
    let k = C64::new(1.0, 1.0) / delta;
    let surface = (k / (2.0 * width) * (k * (0.5 * t)).tanh().inv()).norm();
    let profile = |z: f64| (k / (2.0 * width) * (k * z).cosh() / (k * (0.5 * t)).sinh()).norm();
    let mut worst: f64 = 0.0;
    for fil in &sol.filaments {
        // Centre of the bar is z = 0.
        worst = worst.max((fil.current_density - profile(fil.location[2])).abs() / surface);
    }
    assert!(worst < 0.05, "worst profile error {worst:.3e} of surface J");
    // The hottest filament is an outermost one, and its density is the analytic one at its centroid.
    let peak = sol.conductors[0];
    assert!(peak.location[2].abs() > 0.45 * t);
    assert!((peak.peak_current_density - profile(peak.location[2])).abs() < 0.05 * surface);
    // Current crowds: peak well above the uniform I/A.
    assert!(peak.peak_current_density > 2.0 / (width * t));
    assert!(sol.power_balance.relative_residual < 1e-8);
}

#[test]
fn power_balance_and_path_agreement_on_a_ring() {
    let (g, ports) = ring();
    let d = Discretization::uniform(3, 2);
    let dense = MeshSystem::assemble(&g, &ports, &d).unwrap();
    let iter = IterativeSystem::assemble(&g, &ports, &d, &IterativeParams::default()).unwrap();
    let ex = unit_excitation(1);
    for f in [0.0, 1e5, 1e8] {
        let a = dense.currents(f, &ex).unwrap();
        let b = iter.currents(f, &ex).unwrap();
        assert!(a.power_balance.relative_residual < 1e-9, "{f}");
        assert!(b.power_balance.relative_residual < 1e-7, "{f}");
        assert!(max_diff(&a, &b) < 1e-5, "{f}: {}", max_diff(&a, &b));
        // KCL: the direct path n0→n1 plus the way round (n0→n3→n2→n1, which is
        // segment 1 traversed backwards) carry the port current together.
        let through_port = a.segments[0].current;
        let around = a.segments[1].current;
        assert!((through_port - around - ex[0]).norm() < 1e-9);
        assert_eq!(a.segments.len(), 4);
        assert_eq!(a.conductors.len(), 1);
    }
    // The input power equals ½ Re(Z) |I|².
    let z = dense.impedance(1e8).unwrap()[(0, 0)];
    let a = dense.currents(1e8, &ex).unwrap();
    assert!((a.power_balance.input_w - 0.5 * z.re).abs() < 1e-9 * z.re);
}

#[test]
fn multi_port_excitation_and_single_filament_segments() {
    let mut g = Geometry::new();
    let n: Vec<_> = (0..4)
        .map(|i| g.add_node(Node::new(i as f64 * 1e-3, 0.0, 0.0)).unwrap())
        .collect();
    g.add_segment(SegmentDef::new(n[0], n[1], 1e-4, 1e-4, COPPER))
        .unwrap();
    g.add_segment(SegmentDef::new(n[2], n[3], 1e-4, 1e-4, COPPER))
        .unwrap();
    let ports = vec![Port::new(n[0], n[1]), Port::new(n[2], n[3])];
    let d = Discretization::uniform(1, 1);
    let sys = MeshSystem::assemble(&g, &ports, &d).unwrap();
    let ex = [C64::new(1.0, 0.0), C64::new(0.0, 0.5)];
    // No internal loops: the filament currents are the port currents.
    let sol = sys.currents(1e6, &ex).unwrap();
    assert!((sol.filaments[0].current - ex[0]).norm() < 1e-12);
    assert!((sol.filaments[1].current - ex[1]).norm() < 1e-12);
    assert_eq!(sol.conductors.len(), 2);
    assert!(sol.power_balance.relative_residual < 1e-9);
    let iter = IterativeSystem::assemble(&g, &ports, &d, &IterativeParams::default()).unwrap();
    let sol2 = iter.currents(1e6, &ex).unwrap();
    assert!(max_diff(&sol, &sol2) < 1e-12);
}

#[test]
fn bad_inputs_are_rejected() {
    let (g, ports) = bar(1e-3, 1e-4, 1e-4);
    let sys = MeshSystem::assemble(&g, &ports, &Discretization::uniform(1, 1)).unwrap();
    assert!(matches!(
        sys.currents(0.0, &[]),
        Err(SolveError::InvalidExcitation { .. })
    ));
    assert!(matches!(
        sys.currents(-1.0, &unit_excitation(1)),
        Err(SolveError::InvalidFrequency { .. })
    ));
    assert!(matches!(
        sys.currents(0.0, &[C64::new(f64::NAN, 0.0)]),
        Err(SolveError::InvalidExcitation { .. })
    ));
}
