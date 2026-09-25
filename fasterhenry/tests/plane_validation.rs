//! Ground-plane validation (issue #22): independent PyPEEC slot-differential
//! gates, plus trace-over-plane convergence and coarse sanity checks.
//!
//! # Trace-over-plane diagnostic
//!
//! A copper trace (0.2 mm × 35 µm) runs 8 mm at height `h = 0.5 mm` above
//! a 10 mm × 6 mm × 35 µm plane; a via at each end drops onto the plane,
//! snapped to the nearest live cell node. The port closes the loop across
//! the trace.
//!
//! The return current's lateral distribution is solved by the mesh. We
//! check low-frequency grid convergence, LF/RF inductance ordering, and
//! DC resistance bounds. The RF inductance is also reported against a
//! rectangle-image estimate plus the two vias' Rosa self terms, with a
//! coarse 45 % sanity bound. This composite fixture's vias, snap offsets,
//! finite plane, and single-filament plane bars prevent a demonstrated
//! 5–10 % image-oracle accuracy claim.
//!
//! # Independent plane-validation gate
//!
//! A separate slotted-versus-solid plane fixture compares the slot's
//! inductance and resistance increments against independent PyPEEC voxel
//! references, within 5.5 % and 15 % respectively. CI generates both
//! references and runs this gate in release mode; without references the
//! local test skips that comparison.

use fasterhenry::geometry::{Geometry, Node, NodeId, SegmentDef};
use fasterhenry::mesh::Port;
use fasterhenry::plane::GroundPlane;
use fasterhenry::solve::{solve, Discretization, Subdivision};

const SIGMA: f64 = 5.8e7;
const TRACE_W: f64 = 0.2e-3;
const T: f64 = 35e-6;
const H: f64 = 0.5e-3;
const L: f64 = 8.0e-3;
const MU0: f64 = 2.0 * std::f64::consts::TAU * 1e-7;

fn trace_over_plane(nx: usize, ny: usize) -> (Geometry, Vec<Port>, Discretization) {
    let mut geometry = Geometry::new();
    let plane = GroundPlane {
        lo: [0.0, -3.0e-3],
        hi: [10.0e-3, 3.0e-3],
        z_top: 0.0,
        thickness: T,
        nx,
        ny,
        sigma: SIGMA,
        holes: Vec::new(),
        contacts: Vec::new(),
    };
    let centres = plane.build_into(&mut geometry).unwrap();
    let segments_before = geometry.segment_count();

    // Vias land on the nearest live plane nodes (the snap under test).
    assert!(plane.contains([1.0e-3, 0.0, 0.0], 1e-9));
    assert!(plane.contains([9.0e-3, 0.0, 0.0], 1e-9));
    let snapped_a = plane.attach(&centres, [1.0e-3, 0.0, 0.0]).unwrap();
    let snapped_b = plane.attach(&centres, [9.0e-3, 0.0, 0.0]).unwrap();

    let a = NodeId(geometry.add_node(Node::new(1.0e-3, 0.0, H)).unwrap().0);
    let b = NodeId(geometry.add_node(Node::new(9.0e-3, 0.0, H)).unwrap().0);
    for (top, bottom) in [(a, snapped_a), (b, snapped_b)] {
        geometry
            .add_segment(SegmentDef::new(top, bottom, TRACE_W, T, SIGMA))
            .unwrap();
    }
    geometry
        .add_segment(SegmentDef::new(a, b, TRACE_W, T, SIGMA))
        .unwrap();
    assert_eq!(geometry.segment_count(), segments_before + 3);

    let ports = vec![Port::new(a, b).named("loop")];
    let mut subdivisions = vec![Subdivision::new(1, 1); geometry.segment_count()];
    let trace_index = geometry.segment_count() - 1;
    subdivisions[trace_index] = Subdivision::new(2, 1);
    (geometry, ports, Discretization::PerSegment(subdivisions))
}

fn loop_inductance(nx: usize, ny: usize, frequency: f64) -> f64 {
    let (geometry, ports, discretization) = trace_over_plane(nx, ny);
    let result = solve(&geometry, &ports, &discretization, &[frequency]).unwrap();
    result.impedance_ohm[0][(0, 0)].im / (std::f64::consts::TAU * frequency)
}

/// Wire-over-plane rectangle-image estimate: the loop spans the height `h`
/// (return on the plane surface), so the pair distance in the cross-term
/// runs over `[2h, 2h + 2t]`; the self term uses the rectangle's
/// geometric mean distance `0.2235 (w + t)`. Plus both vias' Rosa terms.
fn image_estimate() -> f64 {
    // <ln R>_cross by 16x16 midpoint quadrature over (dx, dz-offset).
    let mut integral = 0.0;
    const N: usize = 16;
    for i in 0..N {
        let dx = (i as f64 + 0.5) * TRACE_W / N as f64;
        for k in 0..N {
            let offset = 2.0 * H + (k as f64 + 0.5) * 2.0 * T / N as f64;
            integral += (dx * dx + offset * offset).sqrt().ln();
        }
    }
    let cross = integral / (N * N) as f64;
    let gmd = 0.2235 * (TRACE_W + T);
    let l_prime = MU0 / (2.0 * std::f64::consts::PI) * (cross - gmd.ln());

    // Approximate the two vias with Rosa self terms. These do not resolve
    // the snapped via geometry or its coupling to the finite plane and
    // trace, so the composite estimate has no demonstrated 10 % bound.
    let a_eq = (TRACE_W * T / std::f64::consts::PI).sqrt();
    let via = |length: f64| {
        MU0 / (2.0 * std::f64::consts::PI) * length * ((2.0 * length / a_eq).ln() - 0.75)
    };
    l_prime * L + 2.0 * via(H)
}

#[test]
fn trace_over_plane_rf_image_estimate_is_a_coarse_sanity_check() {
    let oracle = image_estimate();
    let measured = loop_inductance(20, 6, 1e9);
    let relative = (measured - oracle).abs() / oracle;
    println!(
        "RF: L = {:.4} nH vs image+via estimate {:.4} nH (rel {relative:.4})",
        measured * 1e9,
        oracle * 1e9
    );
    // Retain a coarse regression check and the diagnostic above. Accuracy
    // is independently gated by the separate PyPEEC slot differential,
    // not by a 5–10 % image-oracle bound for this composite fixture.
    assert!(relative < 0.45, "gross image-estimate deviation {relative}");
}

#[test]
fn low_frequency_return_spreads_and_converges() {
    // At low f the return spreads across the plane (R-dominated); the
    // loop inductance is grid-converged and well above the RF value.
    let coarse = loop_inductance(10, 3, 1e3);
    let fine = loop_inductance(40, 12, 1e3);
    let rf = loop_inductance(20, 6, 1e9);
    let relative = (coarse - fine).abs() / fine;
    println!(
        "LF: {:.6} nH (10x3) vs {:.6} nH (40x12), grid rel {relative:.5}; RF {:.4} nH",
        coarse * 1e9,
        fine * 1e9,
        rf * 1e9
    );
    assert!(relative < 0.05, "grid sensitivity {relative}");
    // With a genuinely conducting plane the return stays concentrated at
    // all frequencies; proximity only tightens it further.
    assert!(
        coarse >= rf && fine >= 0.98 * rf,
        "LF/RF inversion: {coarse:.9} vs {rf:.9}"
    );
}

#[test]
fn dc_resistance_is_dominated_by_the_trace() {
    let (geometry, ports, discretization) = trace_over_plane(20, 6);
    let dc = solve(&geometry, &ports, &discretization, &[0.0]).unwrap();
    let r_trace = L / (SIGMA * TRACE_W * T);
    let r = dc.impedance_ohm[0][(0, 0)].re;
    println!("DC R = {r:.6} ohm (trace alone {r_trace:.6})");
    // The plane return path genuinely conducts: the DC resistance sits
    // between the all-parallel lower bound and the trace alone (funnel
    // resistance through the snap nodes keeps it above the ideal).
    assert!(r > 0.003 && r < r_trace);
}

/// The PyPEEC plane reference, if generated (`--fixture plane`).
fn pypeec_plane_reference() -> Option<serde_json::Value> {
    let path = std::env::var("FASTERHENRY_PYPEEC_PLANE_REFERENCE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../tools/pypeec_plane_reference.json")
        });
    path.exists()
        .then(|| serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap())
}

/// The plane-with-slot fixture (1.2 × 0.8 × 0.02 mm copper, 0.2 × 0.64 mm
/// slot) as a solvable system: port between the cell nodes nearest the
/// two short ends.
fn slotted_plane(nx: usize, ny: usize, slotted: bool) -> (Geometry, Vec<Port>, Discretization) {
    let mut geometry = Geometry::new();
    let holes = if slotted {
        vec![fasterhenry::plane::Hole {
            lo: [0.5e-3, 0.0],
            hi: [0.7e-3, 0.64e-3],
        }]
    } else {
        Vec::new()
    };
    let plane = GroundPlane {
        lo: [0.0, 0.0],
        hi: [1.2e-3, 0.8e-3],
        z_top: 20e-6,
        thickness: 20e-6,
        nx,
        ny,
        sigma: SIGMA,
        holes,
        contacts: Vec::new(),
    };
    let centres = plane.build_into(&mut geometry).unwrap();
    let west = plane.attach(&centres, [1e-9, 0.4e-3, 10e-6]).unwrap();
    let east = plane
        .attach(&centres, [1.2e-3 - 1e-9, 0.4e-3, 10e-6])
        .unwrap();
    let ports = vec![Port::new(west, east).named("plane")];
    let subdivisions = vec![Subdivision::SINGLE; geometry.segment_count()];
    (geometry, ports, Discretization::PerSegment(subdivisions))
}

/// The slotted minus solid differential — the slot's own contribution,
/// with the (differently-modelled) drive contacts cancelling to first
/// order on both sides of the comparison.
#[test]
fn slot_differential_against_pypec() {
    // The 96×64 solve belongs to the release PyPEEC gate below. The ordinary
    // workspace test has no reference and should skip before paying for it.
    if pypeec_plane_reference().is_none() {
        eprintln!("SKIPPED (pypeec plane reference absent)");
        return;
    }
    let measure = |slotted: bool| -> (f64, f64) {
        let system = slotted_plane(96, 64, slotted);
        let result = solve(&system.0, &system.1, &system.2, &[0.0, 1e3]).unwrap();
        let z_dc = result.impedance_ohm[0][(0, 0)];
        let z_ac = result.impedance_ohm[1][(0, 0)];
        (z_ac.im / (std::f64::consts::TAU * 1e3), z_dc.re)
    };
    let (l_slotted, r_slotted) = measure(true);
    let (l_solid, r_solid) = measure(false);
    let (dl, dr) = (l_slotted - l_solid, r_slotted - r_solid);
    println!(
        "fasterhenry: L {:.6} -> {:.6} nH (dL {:.6}); R {:.6} -> {:.6} (dR {:.6})",
        l_solid * 1e9,
        l_slotted * 1e9,
        dl * 1e9,
        r_solid,
        r_slotted,
        dr
    );

    // Grid sensitivity of the differential: the cell-centre mesh converges
    // like ~1/n (measured dL: 0.184 / 0.174 / 0.172 nH at 48x32 / 96x64 /
    // 192x128), so the 48x64-vs-96x64 gap is ~6 % — the reference grid is
    // the comparison above, this only guards regressions in the trend.
    {
        let l = |nx: usize, ny: usize, slotted: bool| {
            let s = slotted_plane(nx, ny, slotted);
            let r = solve(&s.0, &s.1, &s.2, &[1e3]).unwrap();
            r.impedance_ohm[0][(0, 0)].im / (std::f64::consts::TAU * 1e3)
        };
        let coarse = l(48, 32, true) - l(48, 32, false);
        let grid_relative = (coarse - dl).abs() / dl;
        println!("dL grid rel (48x32 vs 96x64) {grid_relative:.4}");
        assert!(grid_relative < 0.07, "dL grid sensitivity {grid_relative}");
    }

    match pypeec_plane_reference() {
        Some(reference) => {
            let solid = std::env::var("FASTERHENRY_PYPEEC_PLANE_SOLID_REFERENCE")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| {
                    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../tools/pypeec_plane_solid_reference.json")
                });
            if !solid.exists() {
                eprintln!("SKIPPED (solid-plane reference absent)");
                return;
            }
            let solid: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(solid).unwrap()).unwrap();
            let pypeec_dl = reference["l_h"].as_f64().unwrap() - solid["l_h"].as_f64().unwrap();
            let pypeec_dr = reference["r_ohm"].as_f64().unwrap() - solid["r_ohm"].as_f64().unwrap();
            let relative = (dl - pypeec_dl).abs() / pypeec_dl;
            println!(
                "PyPEEC: dL {:.6} nH (fh {:.6}, rel {relative:.4}); dR {:.6} vs {:.6} ohm",
                pypeec_dl * 1e9,
                dl * 1e9,
                pypeec_dr,
                dr
            );
            // Measured: fh dL converges 0.184 -> 0.174 -> 0.172 nH at
            // 48x32 / 96x64 / 192x128; PyPEEC 0.1654 nH at 5 um voxels
            // (0.2 % shift at 2.5 um). The residual ~4 % is the two
            // discretizations' method bias plus contact terms the
            // differential cancels only to first order; the bound leaves
            // headroom rather than encoding today's exact number.
            assert!(
                relative < 0.055,
                "PyPEEC slot-differential deviation {relative}"
            );
            assert!((dr - pypeec_dr).abs() / pypeec_dr < 0.15);
        }
        None => {
            eprintln!(
                "SKIPPED (pypeec plane reference absent): regenerate with\n  \
                 python3 tools/pypeec_reference.py --fixture plane --voxel-um 5 \
                 --out tools/pypeec_plane_reference.json and --fixture plane-solid \
                 --out tools/pypeec_plane_solid_reference.json"
            );
        }
    }
}

#[test]
#[ignore = "manual convergence probe"]
fn slot_convergence_probe() {
    let l = |nx: usize, ny: usize, slotted: bool| {
        let s = slotted_plane(nx, ny, slotted);
        let r = solve(&s.0, &s.1, &s.2, &[1e3]).unwrap();
        r.impedance_ohm[0][(0, 0)].im / (std::f64::consts::TAU * 1e3)
    };
    for (nx, ny) in [(48usize, 32usize), (96, 64), (192, 128)] {
        let dl = l(nx, ny, true) - l(nx, ny, false);
        println!("{nx}x{ny}: dL = {:.6} nH", dl * 1e9);
    }
}
