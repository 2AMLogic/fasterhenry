//! Validation of the mesh assembly and the port-impedance solve through the
//! public API.
//!
//! No reference numbers are hard-coded. Every expectation is either
//!
//! * a closed form of circuit theory evaluated in the test (series and
//!   parallel resistances, `R + jωL`, the 2 × 2 coupled-inductor system),
//!   with partial inductances taken straight from the kernels of
//!   [`fasterhenry::inductance`], which have their own validation;
//! * an identity (reciprocity, passivity, the low-frequency limit of a
//!   filament bundle, a split conductor equal to the whole one);
//! * a physical trend (skin effect, Lenz's law) asserted as an inequality; or
//! * an independent solution of the same mesh equations in their textbook
//!   form — the full system `M Z_b Mᵀ I_m = V_s` driven one port at a time,
//!   solved with `nalgebra`'s LU and inverted — written in this file.

use std::f64::consts::TAU;

use fasterhenry::{
    mutual_inductance, partial_inductance_matrix, self_inductance, solve, Discretization, Filament,
    Geometry, MeshError, MeshSystem, Node, NodeId, Port, SegmentDef, SolveError, Subdivision,
    SweepResult,
};
use nalgebra::DMatrix;
use num_complex::Complex;

const COPPER: f64 = 5.8e7;

type C64 = Complex<f64>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A geometry from node coordinates and `(a, b, width, height)` segments.
fn geometry(points: &[[f64; 3]], segments: &[(usize, usize, f64, f64)]) -> Geometry {
    let mut geometry = Geometry::new();
    for &point in points {
        geometry.add_node(Node::from(point)).unwrap();
    }
    for &(a, b, width, height) in segments {
        geometry
            .add_segment(SegmentDef::new(NodeId(a), NodeId(b), width, height, COPPER))
            .unwrap();
    }
    geometry
}

fn port(positive: usize, negative: usize) -> Port {
    Port::new(NodeId(positive), NodeId(negative))
}

/// The whole of segment `index` as one filament.
fn whole(geometry: &Geometry, index: usize) -> Filament {
    Filament::new(&geometry.segment(index).unwrap()).unwrap()
}

/// DC resistance `l / (σ·w·h)` of segment `index`.
fn dc_resistance(geometry: &Geometry, index: usize) -> f64 {
    let segment = geometry.segment(index).unwrap();
    segment.length() / (segment.sigma * segment.area())
}

fn relative(actual: f64, expected: f64) -> f64 {
    ((actual - expected) / expected).abs()
}

fn assert_close(actual: f64, expected: f64, tolerance: f64, what: &str) {
    assert!(
        relative(actual, expected) <= tolerance,
        "{what}: {actual:e} vs {expected:e}, relative error {:e} > {tolerance:e}",
        relative(actual, expected)
    );
}

/// `Z` from the mesh equations in their textbook form, sharing only the loop
/// basis, the filament resistances and the inductance kernels with the crate:
/// assemble the dense `M (R + jωL) Mᵀ`, drive each port loop in turn with
/// 1 V, solve for all loop currents with nalgebra's LU, collect the port
/// currents into the admittance matrix, and invert it.
fn textbook_impedance(system: &MeshSystem, frequency: f64) -> DMatrix<C64> {
    let omega = TAU * frequency;
    let inductance = partial_inductance_matrix(system.filaments()).unwrap();
    let resistance = system.filament_resistances();
    let branch = DMatrix::from_fn(inductance.nrows(), inductance.ncols(), |i, j| {
        let r = if i == j { resistance[i] } else { 0.0 };
        C64::new(r, omega * inductance[(i, j)])
    });
    let mesh = system.mesh();
    let mut incidence = DMatrix::from_element(mesh.loop_count(), mesh.branch_count(), C64::ZERO);
    for i in 0..mesh.loop_count() {
        for &(k, sign) in mesh.row(i) {
            incidence[(i, k)] = C64::new(sign, 0.0);
        }
    }
    let mesh_impedance = &incidence * branch * incidence.transpose();

    let (internal, ports) = (mesh.internal_count(), mesh.port_count());
    let mut sources = DMatrix::from_element(mesh.loop_count(), ports, C64::ZERO);
    for p in 0..ports {
        sources[(internal + p, p)] = C64::ONE;
    }
    let currents = mesh_impedance.lu().solve(&sources).expect("non-singular");
    let admittance = currents.rows(internal, ports).into_owned();
    admittance.try_inverse().expect("invertible admittance")
}

fn max_relative_difference(a: &DMatrix<C64>, b: &DMatrix<C64>) -> f64 {
    (a - b).norm() / b.norm()
}

// ---------------------------------------------------------------------------
// Acceptance: single conductor, two parallel conductors, DC limit
// ---------------------------------------------------------------------------

#[test]
fn single_filament_conductor_is_exactly_r_plus_jwl() {
    let geometry = geometry(
        &[[0.0, 0.0, 0.0], [10e-3, 0.0, 0.0]],
        &[(0, 1, 1e-3, 35e-6)],
    );
    let r = dc_resistance(&geometry, 0);
    let l = self_inductance(&whole(&geometry, 0));
    let frequencies = [0.0, 50.0, 1e3, 1e6, 2.4e9];
    let result = solve(
        &geometry,
        &[port(0, 1)],
        &Discretization::uniform(1, 1),
        &frequencies,
    )
    .unwrap();

    assert_eq!(result.provenance.counts.internal_meshes, 0);
    for (z, f) in result.impedance_ohm.iter().zip(frequencies) {
        assert_eq!(z.shape(), (1, 1));
        // No loop to solve: not a single rounding error is allowed.
        assert_eq!(z[(0, 0)], C64::new(r, TAU * f * l), "at {f} Hz");
    }
}

#[test]
fn two_parallel_conductors_match_the_coupled_two_by_two_system() {
    // Two different traces side by side, the second one running backwards.
    let geometry = geometry(
        &[
            [0.0, 0.0, 0.0],
            [8e-3, 0.0, 0.0],
            [9e-3, 0.4e-3, 0.0],
            [1e-3, 0.4e-3, 0.0],
        ],
        &[(0, 1, 0.2e-3, 35e-6), (2, 3, 0.1e-3, 35e-6)],
    );
    let (f0, f1) = (whole(&geometry, 0), whole(&geometry, 1));
    let (r0, r1) = (dc_resistance(&geometry, 0), dc_resistance(&geometry, 1));
    let (l0, l1) = (self_inductance(&f0), self_inductance(&f1));
    let m = mutual_inductance(&f0, &f1).unwrap();
    assert!(m < 0.0, "antiparallel traces couple negatively, got {m:e}");

    let frequencies = [0.0, 1e4, 1e8];
    let ports = [port(0, 1), port(2, 3)];
    let single = Discretization::uniform(1, 1);
    let result = solve(&geometry, &ports, &single, &frequencies).unwrap();
    for (z, f) in result.impedance_ohm.iter().zip(frequencies) {
        let w = TAU * f;
        assert_eq!(z[(0, 0)], C64::new(r0, w * l0));
        assert_eq!(z[(1, 1)], C64::new(r1, w * l1));
        assert_eq!(z[(0, 1)], C64::new(0.0, w * m));
        assert_eq!(z[(1, 0)], C64::new(0.0, w * m));
    }

    // Reversing a port reverses its current and its voltage: the transfer
    // impedances change sign, the self-impedances do not.
    let reversed = solve(&geometry, &[port(0, 1), port(3, 2)], &single, &[1e8]).unwrap();
    let (z, zr) = (&result.impedance_ohm[2], &reversed.impedance_ohm[0]);
    assert_eq!(zr[(0, 0)], z[(0, 0)]);
    assert_eq!(zr[(1, 1)], z[(1, 1)]);
    assert_eq!(zr[(0, 1)], -z[(0, 1)]);
    assert_eq!(zr[(1, 0)], -z[(1, 0)]);
}

#[test]
fn dc_limit_is_the_resistive_network() {
    // A square ring of four equal bars, each 3 × 2 filaments, plus a lead.
    let side = 2e-3;
    let geometry = geometry(
        &[
            [0.0, 0.0, 0.0],
            [side, 0.0, 0.0],
            [side, side, 0.0],
            [0.0, side, 0.0],
            [-side, 0.0, 0.0],
        ],
        &[
            (0, 1, 1e-4, 5e-5),
            (1, 2, 1e-4, 5e-5),
            (3, 2, 1e-4, 5e-5),
            (0, 3, 1e-4, 5e-5),
            (4, 0, 2e-4, 5e-5),
        ],
    );
    let r = dc_resistance(&geometry, 0);
    let lead = dc_resistance(&geometry, 4);
    assert_close(lead, r / 2.0, 1e-15, "lead resistance");

    let ports = [port(0, 2), port(0, 1), port(4, 2)];
    let system = MeshSystem::assemble(&geometry, &ports, &Discretization::uniform(3, 2)).unwrap();
    // Four bundles of 6 (5 loops each), one lead bundle, one ring loop.
    assert_eq!(system.counts().internal_meshes, 5 * 5 + 1);
    let z = system.impedance(0.0).unwrap();
    assert!(
        z.iter().all(|entry| entry.im == 0.0),
        "DC impedance is real"
    );

    // Opposite corners: 2r ∥ 2r. Adjacent corners: r ∥ 3r. Through the lead.
    assert_close(z[(0, 0)].re, r, 1e-13, "opposite corners");
    assert_close(z[(1, 1)].re, 0.75 * r, 1e-13, "adjacent corners");
    assert_close(z[(2, 2)].re, lead + r, 1e-13, "lead plus ring");
    // Transfer resistances, by superposition on the ring: unit current from
    // node 0 to node 2 splits evenly, dropping r/2 across segment 0-1 …
    assert_close(z[(1, 0)].re, 0.5 * r, 1e-13, "transfer 0→1");
    // … and the lead carries no current unless port 2 drives it.
    assert_close(z[(2, 0)].re, r, 1e-13, "transfer 0→2");
    assert_close(z[(2, 1)].re, 0.5 * r, 1e-13, "transfer 1→2");
}

#[test]
fn bundle_is_the_bar_at_dc_and_shows_the_skin_effect() {
    let geometry = geometry(
        &[[0.0, 0.0, 0.0], [10e-3, 0.0, 0.0]],
        &[(0, 1, 1e-3, 35e-6)],
    );
    let frequencies = [0.0, 1.0, 1e3, 1e5, 1e6, 1e7, 1e8, 1e9];
    let result = solve(
        &geometry,
        &[port(0, 1)],
        &Discretization::uniform(9, 3),
        &frequencies,
    )
    .unwrap();
    assert_eq!(result.provenance.counts.filaments, 27);
    assert_eq!(result.provenance.counts.internal_meshes, 26);

    // DC: 27 equal filaments in parallel are the bar.
    let dc = result.impedance_ohm[0][(0, 0)];
    assert_close(dc.re, dc_resistance(&geometry, 0), 1e-13, "R at DC");
    assert_eq!(dc.im, 0.0);

    // Low frequency: the current is still uniform, and the mean of all
    // filament partial inductances is, by linearity of the Neumann integral,
    // the partial self-inductance of the undivided bar.
    let low = result.inductance(1).unwrap()[(0, 0)];
    assert_close(
        low,
        self_inductance(&whole(&geometry, 0)),
        1e-8,
        "L at 1 Hz",
    );

    // Rising frequency crowds the current to the edges: R rises and L falls,
    // monotonically.
    let resistance: Vec<f64> = (0..frequencies.len())
        .map(|k| result.resistance(k)[(0, 0)])
        .collect();
    let inductance: Vec<f64> = (1..frequencies.len())
        .map(|k| result.inductance(k).unwrap()[(0, 0)])
        .collect();
    assert!(
        resistance.windows(2).all(|w| w[1] >= w[0]),
        "R(f) = {resistance:?}"
    );
    assert!(
        inductance.windows(2).all(|w| w[1] <= w[0]),
        "L(f) = {inductance:?}"
    );
    assert!(resistance[7] > 1.5 * resistance[0], "R(f) = {resistance:?}");
    assert!(
        inductance[6] < 0.99 * inductance[0],
        "L(f) = {inductance:?}"
    );
    assert!(inductance[6] > 0.0);
}

// ---------------------------------------------------------------------------
// Series connections and loops
// ---------------------------------------------------------------------------

#[test]
fn two_collinear_segments_in_series_equal_the_single_long_one() {
    let (width, height) = (0.3e-3, 0.1e-3);
    let long = geometry(
        &[[0.0, 0.0, 0.0], [10e-3, 0.0, 0.0]],
        &[(0, 1, width, height)],
    );
    // Split unevenly, and with the second piece defined end to start.
    let split = geometry(
        &[[0.0, 0.0, 0.0], [3.5e-3, 0.0, 0.0], [10e-3, 0.0, 0.0]],
        &[(0, 1, width, height), (2, 1, width, height)],
    );
    let single = Discretization::uniform(1, 1);
    let frequencies = [0.0, 1e6];
    let z_long = solve(&long, &[port(0, 1)], &single, &frequencies).unwrap();
    let z_split = solve(&split, &[port(0, 2)], &single, &frequencies).unwrap();

    assert_close(
        z_split.impedance_ohm[0][(0, 0)].re,
        z_long.impedance_ohm[0][(0, 0)].re,
        1e-14,
        "series resistance",
    );
    // L₁ + L₂ + 2M of the pieces is the self-inductance of the whole, to the
    // accuracy of the kernels.
    assert_close(
        z_split.inductance(1).unwrap()[(0, 0)],
        z_long.inductance(1).unwrap()[(0, 0)],
        1e-8,
        "series inductance",
    );

    // With bundles the DC resistance is still that of the whole bar.
    let bundled = solve(
        &split,
        &[port(0, 2)],
        &Discretization::PerSegment(vec![Subdivision::new(3, 2), Subdivision::new(2, 2)]),
        &[0.0],
    )
    .unwrap();
    assert_eq!(bundled.provenance.counts.filaments, 10);
    assert_close(
        bundled.impedance_ohm[0][(0, 0)].re,
        z_long.impedance_ohm[0][(0, 0)].re,
        1e-13,
        "bundled series resistance",
    );
}

#[test]
fn rectangular_loop_is_the_signed_sum_of_its_partial_inductances() {
    // A 6 mm × 4 mm loop of 0.2 mm × 0.1 mm wire, open by a 0.2 mm gap at
    // the origin corner; the port is across the gap.
    let (a, b, gap) = (6e-3, 4e-3, 0.2e-3);
    let geometry = geometry(
        &[
            [0.0, 0.0, 0.0],
            [a, 0.0, 0.0],
            [a, b, 0.0],
            [0.0, b, 0.0],
            [0.0, gap, 0.0],
        ],
        &[
            (0, 1, 0.2e-3, 0.1e-3),
            (1, 2, 0.2e-3, 0.1e-3),
            (2, 3, 0.2e-3, 0.1e-3),
            (3, 4, 0.2e-3, 0.1e-3),
        ],
    );
    let frequency = 1e7;
    let result = solve(
        &geometry,
        &[port(0, 4)],
        &Discretization::uniform(1, 1),
        &[frequency],
    )
    .unwrap();

    // Independently: every side is traversed forwards, so the loop inductance
    // is ΣLᵢ + 2Σ_{i<j} Mᵢⱼ — the orientation is inside the kernel's sign.
    let sides: Vec<Filament> = (0..4).map(|i| whole(&geometry, i)).collect();
    let self_sum: f64 = sides.iter().map(self_inductance).sum();
    let mut mutual_sum = 0.0;
    for i in 0..4 {
        for j in i + 1..4 {
            let m = mutual_inductance(&sides[i], &sides[j]).unwrap();
            // Adjacent sides are perpendicular; opposite sides antiparallel.
            if (j - i) % 2 == 1 {
                assert_eq!(m, 0.0);
            } else {
                assert!(m < 0.0);
            }
            mutual_sum += 2.0 * m;
        }
    }
    let resistance: f64 = (0..4).map(|i| dc_resistance(&geometry, i)).sum();

    let z = result.impedance_ohm[0][(0, 0)];
    assert_close(z.re, resistance, 1e-14, "loop resistance");
    assert_close(
        z.im / (TAU * frequency),
        self_sum + mutual_sum,
        1e-13,
        "loop L",
    );
    // The return path cancels part of the flux: 0 < L_loop < ΣLᵢ.
    assert!(self_sum + mutual_sum > 0.0 && mutual_sum < 0.0);
}

// ---------------------------------------------------------------------------
// Identities of the general solve
// ---------------------------------------------------------------------------

/// Two coupled traces of different sizes with bundles, a bend, a closed ring
/// (so the loop basis has a conductor loop) and a floating bar.
fn coupled_structure() -> (Geometry, Vec<Port>, Discretization) {
    let geometry = geometry(
        &[
            // Trace A with a bend.
            [0.0, 0.0, 0.0],
            [4e-3, 0.0, 0.0],
            [4e-3, 2e-3, 0.0],
            // Trace B, above A.
            [0.5e-3, 0.0, 0.3e-3],
            [3.5e-3, 0.0, 0.3e-3],
            // Closed ring beside them.
            [0.0, -1e-3, 0.0],
            [3e-3, -1e-3, 0.0],
            [3e-3, -3e-3, 0.0],
            [0.0, -3e-3, 0.0],
            // Floating bar.
            [1e-3, 1e-3, 0.0],
            [3e-3, 1e-3, 0.0],
        ],
        &[
            (0, 1, 0.4e-3, 0.1e-3),
            (1, 2, 0.4e-3, 0.1e-3),
            (3, 4, 0.2e-3, 0.05e-3),
            (5, 6, 0.2e-3, 0.1e-3),
            (6, 7, 0.2e-3, 0.1e-3),
            (7, 8, 0.2e-3, 0.1e-3),
            (8, 5, 0.2e-3, 0.1e-3),
            (9, 10, 0.2e-3, 0.1e-3),
        ],
    );
    let ports = vec![
        port(0, 2).named("A"),
        port(4, 3).named("B"),
        port(5, 7).named("ring diagonal"),
    ];
    let mut subdivisions = vec![Subdivision::new(2, 2); 8];
    subdivisions[0] = Subdivision::new(4, 3);
    subdivisions[1] = Subdivision::new(3, 3);
    subdivisions[2] = Subdivision::new(3, 1);
    subdivisions[5] = Subdivision::SINGLE;
    (geometry, ports, Discretization::PerSegment(subdivisions))
}

#[test]
fn schur_reduction_matches_the_textbook_mesh_solve() {
    let (geometry, ports, discretization) = coupled_structure();
    let system = MeshSystem::assemble(&geometry, &ports, &discretization).unwrap();
    // 41 filaments in 8 segments, plus the ring: more internal loops than
    // one panel of the blocked LU.
    assert_eq!(system.counts().internal_meshes, 41 - 8 + 1);
    for frequency in [1e3, 1e6, 1e9] {
        let z = system.impedance(frequency).unwrap();
        let reference = textbook_impedance(&system, frequency);
        let difference = max_relative_difference(&z, &reference);
        assert!(
            difference < 1e-10,
            "at {frequency} Hz the two solutions differ by {difference:e}"
        );
    }
}

#[test]
fn impedance_is_reciprocal_and_passive() {
    let (geometry, ports, discretization) = coupled_structure();
    let frequencies = [0.0, 1e4, 1e6, 1e8, 1e10];
    let result = solve(&geometry, &ports, &discretization, &frequencies).unwrap();
    for (k, z) in result.impedance_ohm.iter().enumerate() {
        let asymmetry = (z - z.transpose()).norm() / z.norm();
        assert!(asymmetry < 1e-12, "Z − Zᵀ = {asymmetry:e} at index {k}");

        // Passive: Re Z is positive definite. Inductive: so is Im Z for ω > 0.
        let resistance = result.resistance(k);
        let symmetric = (&resistance + resistance.transpose()) * 0.5;
        let eigenvalues = symmetric.symmetric_eigenvalues();
        assert!(eigenvalues.min() > 0.0, "Re Z eigenvalues {eigenvalues}");
        if let Some(inductance) = result.inductance(k) {
            let symmetric = (&inductance + inductance.transpose()) * 0.5;
            let eigenvalues = symmetric.symmetric_eigenvalues();
            assert!(eigenvalues.min() > 0.0, "Im Z/ω eigenvalues {eigenvalues}");
        }
    }
    // Ports on different conductors are coupled only magnetically.
    let dc = &result.impedance_ohm[0];
    assert_eq!(dc[(0, 1)], C64::ZERO);
    assert_eq!(dc[(0, 2)], C64::ZERO);
    assert!(result.impedance_ohm[3][(0, 1)].im.abs() > 0.0);
}

#[test]
fn floating_conductors_only_act_through_induced_currents() {
    let trace = [[0.0, 0.0, 0.0], [5e-3, 0.0, 0.0]];
    let alone = geometry(&trace, &[(0, 1, 0.2e-3, 0.1e-3)]);
    let single = Discretization::uniform(1, 1);
    let frequencies = [0.0, 1e9];
    let z_alone = solve(&alone, &[port(0, 1)], &single, &frequencies).unwrap();

    // An open single-filament bar has no loop to carry current: no effect
    // whatsoever, and no singular system either.
    let mut points = trace.to_vec();
    points.extend([[0.0, 0.5e-3, 0.0], [5e-3, 0.5e-3, 0.0]]);
    let with_bar = geometry(&points, &[(0, 1, 0.2e-3, 0.1e-3), (2, 3, 0.2e-3, 0.1e-3)]);
    let z_bar = solve(&with_bar, &[port(0, 1)], &single, &frequencies).unwrap();
    assert_eq!(z_bar.impedance_ohm, z_alone.impedance_ohm);

    // A closed ring next to the trace is a shorted secondary: invisible at DC,
    // and at high frequency its induced current opposes the flux (Lenz), so
    // the inductance seen at the port drops and the losses rise.
    let mut points = trace.to_vec();
    points.extend([
        [0.0, 0.4e-3, 0.0],
        [5e-3, 0.4e-3, 0.0],
        [5e-3, 2e-3, 0.0],
        [0.0, 2e-3, 0.0],
    ]);
    let with_ring = geometry(
        &points,
        &[
            (0, 1, 0.2e-3, 0.1e-3),
            (2, 3, 0.2e-3, 0.1e-3),
            (3, 4, 0.2e-3, 0.1e-3),
            (4, 5, 0.2e-3, 0.1e-3),
            (5, 2, 0.2e-3, 0.1e-3),
        ],
    );
    let z_ring = solve(&with_ring, &[port(0, 1)], &single, &frequencies).unwrap();
    assert_eq!(z_ring.provenance.counts.internal_meshes, 1);
    assert_eq!(z_ring.impedance_ohm[0], z_alone.impedance_ohm[0]);
    let (alone, ring) = (
        z_alone.impedance_ohm[1][(0, 0)],
        z_ring.impedance_ohm[1][(0, 0)],
    );
    assert!(ring.im < 0.95 * alone.im, "L: {} vs {}", ring.im, alone.im);
    assert!(ring.re > alone.re, "R: {} vs {}", ring.re, alone.re);

    // One shorted turn is a closed form too: Z = Z₁ − (jωM)² / Z₂.
    let sides: Vec<Filament> = (1..5).map(|i| whole(&with_ring, i)).collect();
    let primary = whole(&with_ring, 0);
    let w = TAU * frequencies[1];
    let mut ring_l = 0.0;
    for a in &sides {
        for b in &sides {
            ring_l += mutual_inductance(a, b).unwrap();
        }
    }
    let ring_r: f64 = (1..5).map(|i| dc_resistance(&with_ring, i)).sum();
    let coupling: f64 = sides
        .iter()
        .map(|s| mutual_inductance(&primary, s).unwrap())
        .sum();
    let jwm = C64::new(0.0, w * coupling);
    let expected = alone - jwm * jwm / C64::new(ring_r, w * ring_l);
    assert!(
        (ring - expected).norm() / expected.norm() < 1e-12,
        "{ring} vs {expected}"
    );
}

// ---------------------------------------------------------------------------
// Sweep API, errors, JSON
// ---------------------------------------------------------------------------

#[test]
fn sweep_preserves_order_and_equals_pointwise_evaluation() {
    let (geometry, ports, discretization) = coupled_structure();
    let system = MeshSystem::assemble(&geometry, &ports, &discretization).unwrap();
    let frequencies = [1e9, 0.0, 3e5, 1e9, 42.0];
    let result = system.sweep(&frequencies).unwrap();
    assert_eq!(result.frequencies_hz, frequencies);
    assert_eq!(result.ports, ports);
    for (z, &f) in result.impedance_ohm.iter().zip(&frequencies) {
        assert_eq!(z, &system.impedance(f).unwrap());
    }
    assert_eq!(result.impedance_ohm[0], result.impedance_ohm[3]);

    let empty = system.sweep(&[]).unwrap();
    assert!(empty.impedance_ohm.is_empty() && empty.frequencies_hz.is_empty());
}

#[test]
fn invalid_input_is_reported() {
    let geometry = geometry(
        &[[0.0, 0.0, 0.0], [1e-3, 0.0, 0.0], [2e-3, 0.0, 0.0]],
        &[(0, 1, 1e-4, 1e-4), (1, 2, 1e-4, 1e-4)],
    );
    let single = Discretization::uniform(1, 1);
    let ports = [port(0, 2)];

    for (index, bad) in [(1, -1.0), (1, f64::NAN), (1, f64::INFINITY)] {
        match solve(&geometry, &ports, &single, &[1.0, bad]) {
            Err(SolveError::InvalidFrequency { index: i, value }) => {
                assert_eq!(i, index);
                assert!(value.is_nan() || value == bad);
            }
            other => panic!("expected InvalidFrequency, got {other:?}"),
        }
    }
    assert!(matches!(
        solve(&geometry, &ports, &Discretization::uniform(2, 0), &[1.0]),
        Err(SolveError::Discretize { segment: 0, .. })
    ));
    assert_eq!(
        solve(
            &geometry,
            &ports,
            &Discretization::PerSegment(vec![Subdivision::SINGLE]),
            &[1.0]
        ),
        Err(SolveError::Mesh(MeshError::SegmentCountMismatch {
            expected: 2,
            got: 1
        }))
    );
    assert_eq!(
        solve(&geometry, &[], &single, &[1.0]),
        Err(SolveError::Mesh(MeshError::NoPorts))
    );
    assert_eq!(
        solve(&geometry, &[port(0, 3)], &single, &[1.0]),
        Err(SolveError::Mesh(MeshError::UnknownPortNode {
            port: 0,
            node: 3,
            node_count: 3
        }))
    );
    let message = solve(&geometry, &[port(1, 1)], &single, &[1.0])
        .unwrap_err()
        .to_string();
    assert!(message.contains("port 0"), "{message}");
}

#[test]
fn result_serializes_to_json_with_provenance() {
    let (geometry, ports, discretization) = coupled_structure();
    let frequencies = [0.0, 1e6, 1e9];
    let result = solve(&geometry, &ports, &discretization, &frequencies).unwrap();

    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["schema_version"], 1);
    assert_eq!(
        json["provenance"]["fasterhenry_version"],
        fasterhenry::VERSION
    );
    let counts = &json["provenance"]["counts"];
    assert_eq!(counts["segments"], 8);
    assert_eq!(counts["nodes"], 11);
    assert_eq!(counts["ports"], 3);
    assert_eq!(counts["filaments"], 4 * 3 + 3 * 3 + 3 + 1 + 4 * 4);
    assert_eq!(
        counts["meshes"].as_u64().unwrap(),
        counts["internal_meshes"].as_u64().unwrap() + 3
    );
    for key in [
        "threads",
        "inductance_s",
        "assembly_s",
        "solve_s",
        "total_s",
    ] {
        assert!(json["provenance"]["timing"][key].is_number(), "{key}");
    }
    assert_eq!(json["frequencies_hz"], serde_json::json!([0.0, 1e6, 1e9]));
    assert_eq!(json["ports"][0]["name"], "A");
    assert_eq!(json["ports"][1]["positive"], 4);

    // impedance_ohm[k][i][j] = { re, im }, rows then columns.
    let matrices = json["impedance_ohm"].as_array().unwrap();
    assert_eq!(matrices.len(), 3);
    for (k, matrix) in matrices.iter().enumerate() {
        assert_eq!(matrix.as_array().unwrap().len(), 3);
        for i in 0..3 {
            assert_eq!(matrix[i].as_array().unwrap().len(), 3);
            for j in 0..3 {
                let z = result.impedance_ohm[k][(i, j)];
                assert_eq!(matrix[i][j]["re"].as_f64().unwrap(), z.re);
                assert_eq!(matrix[i][j]["im"].as_f64().unwrap(), z.im);
            }
        }
    }

    // Exact round trip through text.
    let text = serde_json::to_string_pretty(&result).unwrap();
    let back: SweepResult = serde_json::from_str(&text).unwrap();
    assert_eq!(back, result);

    // Without the timing block, a second run is identical bit for bit.
    let again = solve(&geometry, &ports, &discretization, &frequencies).unwrap();
    let (a, b) = (result.without_timing(), again.without_timing());
    assert_eq!(a, b);
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
    assert!(serde_json::to_value(&a).unwrap()["provenance"]
        .get("timing")
        .is_none());

    // A matrix that is not square is rejected on the way in.
    let mut broken = json.clone();
    broken["impedance_ohm"][0][0].as_array_mut().unwrap().pop();
    assert!(serde_json::from_value::<SweepResult>(broken).is_err());
}

#[test]
fn filament_resistance_is_length_over_sigma_area() {
    let geometry = geometry(
        &[[0.0, 0.0, 0.0], [0.0, 0.0, 7e-3]],
        &[(0, 1, 0.3e-3, 0.2e-3)],
    );
    let system =
        MeshSystem::assemble(&geometry, &[port(0, 1)], &Discretization::uniform(3, 4)).unwrap();
    assert_eq!(system.filaments().len(), 12);
    for (filament, &r) in system.filaments().iter().zip(system.filament_resistances()) {
        assert_eq!(r, fasterhenry::filament_resistance(filament));
        assert_close(r, 7e-3 / (COPPER * 0.1e-3 * 0.05e-3), 1e-14, "R filament");
    }
}
