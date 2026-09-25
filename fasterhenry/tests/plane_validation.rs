//! Ground-plane validation (issues #22, #36): independent PyPEEC
//! slot-differential and contact-separation gates, plus trace-over-plane
//! convergence and coarse sanity checks.
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
//! local test skips that comparison. Success is announced on stdout with
//! `SLOT GATE PASSED`, which CI counts — see the note on positive markers
//! below.
//!
//! # Graded contact regions (issue #36)
//!
//! The same 1.2 × 0.8 × 0.02 mm sheet, driven between two 25 µm via
//! landings *inside* it, is meshed three ways: a 100 µm background graded
//! down to 25 µm under each landing, a fully uniform 25 µm plane, and the
//! bare 100 µm background. Two gates follow.
//!
//! * `graded_contacts_match_a_fully_fine_uniform_plane` — the graded mesh
//!   must reproduce the fully-fine plane to 1 % in both L and R at a ≥ 4×
//!   filament saving, and beat its own background mesh by a wide margin.
//! * `contact_separation_differential_against_pypec` — `L(far) − L(near)`
//!   for the two landing separations, against independent PyPEEC voxel
//!   references (the pad terms cancel, as the slot gate's do).
//!
//! Both are **release-mode** gates: the fully-fine reference is 2 992
//! filaments, seconds optimized and minutes unoptimized, so in a debug
//! build the first one skips itself. Each announces success on stdout with
//! `CONTACT GATE PASSED`, and CI asserts it counted **two** of them.
//!
//! # Why every gate here prints a positive marker (issue #59)
//!
//! Every skip path in this file reports itself with `eprintln!`, i.e. on
//! **stderr**. A CI step that pipes only stdout into its log (`cargo test
//! … | tee log`) and then greps that log for `SKIPPED` is therefore
//! grepping text the message never entered: the check cannot fail, so a
//! silently-skipped gate looks exactly like a passing one. That is what
//! happened to the slot gate for the whole of its life before #59.
//!
//! The fix is a marker printed on **stdout** only after a comparison has
//! actually completed — `SLOT GATE PASSED` here, `CONTACT GATE PASSED`
//! for the two contact gates — whose occurrences CI counts against an
//! exact expected number. A skip then shows up as a missing marker
//! regardless of how the step routes stderr.

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
/// Printed only once the slot differential has been compared against its
/// PyPEEC references and passed; CI counts it (issue #59).
const SLOT_PASSED: &str = "SLOT GATE PASSED";

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
            println!("{SLOT_PASSED} (slotted-vs-solid differential vs PyPEEC)");
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

// ---------------------------------------------------------------------------
// Contact-dominated fixture (issue #36): graded contact regions
// ---------------------------------------------------------------------------

/// The slot fixture's 1.2 × 0.8 × 0.02 mm copper sheet, driven between two
/// via landings inside it instead of across its short ends: the port
/// impedance is then dominated by the current crowding under the contacts,
/// which is exactly what a graded contact region has to resolve.
const CONTACT_HI: [f64; 2] = [1.2e-3, 0.8e-3];
const CONTACT_T: f64 = 20e-6;
/// The fine cell under a landing, and the background cell away from it.
const CONTACT_FINE: f64 = 25e-6;
const CONTACT_COARSE: f64 = 100e-6;
/// Geometric decay ratio outward from a landing, and the odd number of
/// fine cells across its refined region (odd, so the landing is the centre
/// cell's centre rather than an edge).
const CONTACT_RATIO: f64 = 2.0;
const CONTACT_CELLS: usize = 5;
/// The landing pairs: 0.825 mm apart, and 0.225 mm apart. Every centre is
/// a cell centre of the graded *and* the uniform 25 µm mesh (an odd
/// multiple of 12.5 µm on the 25 µm lattice), so every mesh under test
/// drives exactly the same two points — and a voxel centre of the 5 µm
/// PyPEEC grid, so the reference drives them too.
const CONTACT_FAR: [[f64; 2]; 2] = [[0.1875e-3, 0.3875e-3], [1.0125e-3, 0.3875e-3]];
const CONTACT_NEAR: [[f64; 2]; 2] = [[0.4875e-3, 0.3875e-3], [0.7125e-3, 0.3875e-3]];

/// How the contact fixture's plane is meshed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ContactMesh {
    /// A coarse background with a graded refinement under each landing.
    Graded,
    /// Uniform everywhere at the graded mesh's fine cell — the reference.
    FineUniform,
    /// Uniform at the background cell: the unrefined baseline.
    CoarseUniform,
}

/// The contact fixture's plane, and the landings it is driven between.
fn contact_plane(mesh: ContactMesh, far: bool) -> (GroundPlane, [[f64; 2]; 2]) {
    let landings = if far { CONTACT_FAR } else { CONTACT_NEAR };
    let cells = |pitch: f64| {
        [
            (CONTACT_HI[0] / pitch).round() as usize,
            (CONTACT_HI[1] / pitch).round() as usize,
        ]
    };
    let half = CONTACT_CELLS as f64 * CONTACT_FINE / 2.0;
    let (cells, contacts) = match mesh {
        ContactMesh::Graded => (
            cells(CONTACT_COARSE),
            landings
                .iter()
                .map(|landing| {
                    fasterhenry::plane::ContactRegion::new(
                        [landing[0] - half, landing[1] - half],
                        [landing[0] + half, landing[1] + half],
                        [CONTACT_CELLS, CONTACT_CELLS],
                        CONTACT_RATIO,
                    )
                })
                .collect(),
        ),
        ContactMesh::FineUniform => (cells(CONTACT_FINE), Vec::new()),
        ContactMesh::CoarseUniform => (cells(CONTACT_COARSE), Vec::new()),
    };
    let plane = GroundPlane {
        lo: [0.0, 0.0],
        hi: CONTACT_HI,
        z_top: CONTACT_T,
        thickness: CONTACT_T,
        nx: cells[0],
        ny: cells[1],
        sigma: SIGMA,
        holes: Vec::new(),
        contacts,
    };
    (plane, landings)
}

/// The contact fixture as a solvable system, plus the plane's bar count.
fn contact_fixture(mesh: ContactMesh, far: bool) -> (Geometry, Vec<Port>, Discretization, usize) {
    let (plane, landings) = contact_plane(mesh, far);
    let mut geometry = Geometry::new();
    let centres = plane.build_into(&mut geometry).unwrap();
    let bars = geometry.segment_count();
    let depth = CONTACT_T / 2.0;
    let mut ends = Vec::new();
    for landing in landings {
        let node = plane
            .attach(&centres, [landing[0], landing[1], depth])
            .unwrap();
        let position = geometry.nodes()[node.0];
        // Every mesh that resolves the landing drives exactly its centre;
        // the coarse baseline cannot, and snaps up to half a background
        // cell away — which is the very error grading exists to remove.
        if mesh != ContactMesh::CoarseUniform {
            assert!(
                (position.x - landing[0]).abs() < 1e-12 && (position.y - landing[1]).abs() < 1e-12,
                "landing {landing:?} must be a cell centre of the {mesh:?} mesh (got {}, {})",
                position.x,
                position.y
            );
        }
        ends.push(node);
    }
    let ports = vec![Port::new(ends[0], ends[1]).named("contact")];
    (
        geometry,
        ports,
        Discretization::Uniform(Subdivision::SINGLE),
        bars,
    )
}

/// `(L at 1 kHz, R at DC, plane bars)` of the contact fixture.
fn contact_impedance(mesh: ContactMesh, far: bool) -> (f64, f64, usize) {
    let (geometry, ports, discretization, bars) = contact_fixture(mesh, far);
    let result = solve(&geometry, &ports, &discretization, &[0.0, 1e3]).unwrap();
    let z_dc = result.impedance_ohm[0][(0, 0)];
    let z_ac = result.impedance_ohm[1][(0, 0)];
    (z_ac.im / (std::f64::consts::TAU * 1e3), z_dc.re, bars)
}

/// Bars the plane mesh carries under each meshing strategy (no solve).
fn contact_bars(mesh: ContactMesh, far: bool) -> usize {
    contact_plane(mesh, far).0.mesh().unwrap().bars()
}

/// The cheap half of the contact claim: grading resolves both landings at
/// the fine cell while spending a small fraction of the fully-fine mesh's
/// filaments. No solve, so this runs in every profile; the accuracy half
/// is the release gate below.
#[test]
fn graded_contacts_resolve_the_landings_and_save_filaments() {
    for far in [true, false] {
        let (graded, fine, coarse) = (
            contact_bars(ContactMesh::Graded, far),
            contact_bars(ContactMesh::FineUniform, far),
            contact_bars(ContactMesh::CoarseUniform, far),
        );
        println!(
            "{} landings: graded {graded} bars, fine-uniform {fine}, coarse-uniform {coarse}",
            if far { "far" } else { "near" }
        );
        assert!(
            fine >= 4 * graded,
            "filament saving {fine} / {graded} is below 4x"
        );
        assert!(graded > coarse, "grading must refine the background mesh");
        // Both landings sit exactly on a cell centre of the graded mesh —
        // asserted inside `contact_fixture`, which also snaps the port.
        let (_, _, _, bars) = contact_fixture(ContactMesh::Graded, far);
        assert_eq!(bars, graded);
        // The graded mesh's finest cell is exactly the declared fine cell;
        // its coarsest relaxes back to the background cell — up to it, but
        // not past it (the gap-filling scale-to-fit only ever shrinks the
        // target extents).
        let mesh = contact_plane(ContactMesh::Graded, far).0.mesh().unwrap();
        let extents: Vec<f64> = (0..mesh.nx()).map(|i| mesh.dx(i)).collect();
        let finest = extents.iter().copied().fold(f64::MAX, f64::min);
        let coarsest = extents.iter().copied().fold(0.0, f64::max);
        assert!(
            (finest - CONTACT_FINE).abs() < 1e-12,
            "finest cell {finest}"
        );
        assert!(
            coarsest <= CONTACT_COARSE + 1e-12 && coarsest > 0.9 * CONTACT_COARSE,
            "coarsest cell {coarsest} does not relax to the background cell"
        );
    }
}

/// The engine-internal claim: graded contact regions reproduce a fully
/// fine uniform plane at a large filament saving, and do it far better
/// than the background mesh they are built on.
///
/// The fully-fine reference is a 48 × 32 plane — 2 992 filaments, ~12 s
/// optimized but ~10 min unoptimized — so, like the slot gate above, this
/// is a **release-mode** gate; the debug skip below keeps
/// `cargo test --workspace` usable. CI runs `cargo test --release ...
/// --test plane_validation` and requires the `CONTACT GATE PASSED` line
/// at the end of this test, so the skip cannot pass for a run.
#[test]
fn graded_contacts_match_a_fully_fine_uniform_plane() {
    if cfg!(debug_assertions) {
        eprintln!(
            "SKIPPED (debug build): the 48x32 fully-fine reference solve costs ~10 min \
             unoptimized. Run `cargo test --release -p fasterhenry --test plane_validation`; \
             CI gates it there."
        );
        return;
    }
    let (l_graded, r_graded, bars_graded) = contact_impedance(ContactMesh::Graded, true);
    let (l_fine, r_fine, bars_fine) = contact_impedance(ContactMesh::FineUniform, true);
    let (l_coarse, r_coarse, bars_coarse) = contact_impedance(ContactMesh::CoarseUniform, true);
    let relative = |value: f64, reference: f64| (value - reference).abs() / reference;
    println!(
        "graded  {bars_graded:5} bars: L {:.6} nH, R {:.6} ohm (L rel {:.4}, R rel {:.4})",
        l_graded * 1e9,
        r_graded,
        relative(l_graded, l_fine),
        relative(r_graded, r_fine)
    );
    println!(
        "coarse  {bars_coarse:5} bars: L {:.6} nH, R {:.6} ohm (L rel {:.4}, R rel {:.4})",
        l_coarse * 1e9,
        r_coarse,
        relative(l_coarse, l_fine),
        relative(r_coarse, r_fine)
    );
    println!(
        "fine    {bars_fine:5} bars: L {:.6} nH, R {:.6} ohm (reference)",
        l_fine * 1e9,
        r_fine
    );
    assert!(
        relative(l_graded, l_fine) < 0.01,
        "graded vs fine-uniform L deviation {}",
        relative(l_graded, l_fine)
    );
    assert!(
        relative(r_graded, r_fine) < 0.01,
        "graded vs fine-uniform R deviation {}",
        relative(r_graded, r_fine)
    );
    assert!(
        bars_fine >= 4 * bars_graded,
        "filament saving {bars_fine} / {bars_graded} is below 4x"
    );
    // Grading is what buys the agreement: its own background mesh, without
    // the refinement, is far off.
    assert!(
        relative(l_graded, l_fine) * 4.0 < relative(l_coarse, l_fine),
        "the graded mesh must beat its background mesh by a wide margin"
    );
    println!("CONTACT GATE PASSED (graded vs fully-fine uniform)");
}

/// The PyPEEC contact references, if generated (`--fixture contact` /
/// `--fixture contact-near`).
fn pypeec_contact_reference(near: bool) -> Option<serde_json::Value> {
    let variable = if near {
        "FASTERHENRY_PYPEEC_CONTACT_NEAR_REFERENCE"
    } else {
        "FASTERHENRY_PYPEEC_CONTACT_REFERENCE"
    };
    let default = if near {
        "../tools/pypeec_contact_near_reference.json"
    } else {
        "../tools/pypeec_contact_reference.json"
    };
    let path = std::env::var(variable)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(default));
    path.exists()
        .then(|| serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap())
}

/// The separation differential on the contact fixture — `L(far) − L(near)`
/// — against independent PyPEEC voxel references. Both configurations
/// carry the same two landings, so the contact terms the two solvers model
/// differently (a cell-centre node here, a 25 µm voxel pad there) cancel
/// to first order, leaving the field between the contacts.
///
/// Release-mode, for the same reason as the gate above: the grading
/// cross-check inside it solves the fully-fine plane twice.
#[test]
fn contact_separation_differential_against_pypec() {
    if cfg!(debug_assertions) {
        eprintln!(
            "SKIPPED (debug build): the fully-fine cross-check inside this gate costs \
             ~20 min unoptimized. Run `cargo test --release -p fasterhenry --test \
             plane_validation`; CI gates it there."
        );
        return;
    }
    let (Some(far_reference), Some(near_reference)) = (
        pypeec_contact_reference(false),
        pypeec_contact_reference(true),
    ) else {
        eprintln!(
            "SKIPPED (pypeec contact references absent): regenerate with\n  \
             python3 tools/pypeec_reference.py --fixture contact --voxel-um 5 \
             --out tools/pypeec_contact_reference.json and --fixture contact-near \
             --out tools/pypeec_contact_near_reference.json"
        );
        return;
    };
    let graded = |far: bool| {
        let (l, r, _) = contact_impedance(ContactMesh::Graded, far);
        (l, r)
    };
    let (l_far, r_far) = graded(true);
    let (l_near, r_near) = graded(false);
    let (dl, dr) = (l_far - l_near, r_far - r_near);
    println!(
        "fasterhenry (graded): L {:.6} -> {:.6} nH (dL {:.6}); R {:.6} -> {:.6} (dR {:.6})",
        l_near * 1e9,
        l_far * 1e9,
        dl * 1e9,
        r_near,
        r_far,
        dr
    );

    // The differential on the fully fine uniform mesh: the graded mesh
    // must not move it, or the PyPEEC comparison below is measuring the
    // grading rather than the physics.
    {
        let fine = |far: bool| contact_impedance(ContactMesh::FineUniform, far).0;
        let dl_fine = fine(true) - fine(false);
        let relative = (dl - dl_fine).abs() / dl_fine;
        println!(
            "dL graded {:.6} nH vs fine-uniform {:.6} nH (rel {relative:.4})",
            dl * 1e9,
            dl_fine * 1e9
        );
        assert!(relative < 0.02, "dL grading sensitivity {relative}");
    }

    let pypeec_dl =
        far_reference["l_h"].as_f64().unwrap() - near_reference["l_h"].as_f64().unwrap();
    let pypeec_dr =
        far_reference["r_ohm"].as_f64().unwrap() - near_reference["r_ohm"].as_f64().unwrap();
    let relative = (dl - pypeec_dl).abs() / pypeec_dl;
    println!(
        "PyPEEC: dL {:.6} nH (fh {:.6}, rel {relative:.4}); dR {:.6} vs {:.6} ohm",
        pypeec_dl * 1e9,
        dl * 1e9,
        pypeec_dr,
        dr
    );
    // Measured: fh dL 0.224802 nH (graded, 580 bars) against PyPEEC
    // 0.224530 nH at 5 µm voxels — 0.12 %. The bound leaves headroom for
    // the two discretizations' method bias rather than encoding today's
    // exact number.
    assert!(
        relative < 0.03,
        "PyPEEC contact-differential deviation {relative}"
    );
    assert!((dr - pypeec_dr).abs() / pypeec_dr < 0.15);
    println!("CONTACT GATE PASSED (graded vs PyPEEC separation differential)");
}

#[test]
#[ignore = "manual convergence probe"]
fn contact_grading_probe() {
    for (label, mesh) in [
        ("graded", ContactMesh::Graded),
        ("coarse", ContactMesh::CoarseUniform),
        ("fine", ContactMesh::FineUniform),
    ] {
        let (l_far, r_far, bars) = contact_impedance(mesh, true);
        let (l_near, r_near, _) = contact_impedance(mesh, false);
        println!(
            "{label:7} {bars:5} bars: L(far) {:.6} nH, L(near) {:.6} nH, dL {:.6} nH, \
             R(far) {r_far:.6} ohm, dR {:.6} ohm",
            l_far * 1e9,
            l_near * 1e9,
            (l_far - l_near) * 1e9,
            r_far - r_near
        );
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
