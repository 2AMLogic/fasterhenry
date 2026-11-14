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
//! # Why the references' geometry is checked (issue #369)
//!
//! `tools/pypeec_reference.py` must stay an independent discretization and
//! solve, so it cannot share these fixtures: every geometry gated here is
//! stated twice, once by [`SHEET_HI`] and friends below and once in Python.
//! The agreement used to be asserted in prose only, and nothing read it —
//! so a pitch, landing, thickness or conductivity edit on either side left
//! a 2–5.5 %-tolerance gate comparing two *different* problems, passing
//! until the drift happened to exceed the tolerance.
//!
//! The script therefore writes the geometry it solved into each reference's
//! `geometry` object, and every gate below runs
//! `common::assert_reference_geometry` against its own constants *before*
//! comparing an impedance. A drift then fails naming the field that moved.
//!
//! # Graded contact regions (issue #36)
//!
//! The same sheet — [`SHEET_HI`] / [`SHEET_T`], one definition shared with
//! the slot fixture above — driven between two 25 µm via
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
/// Where the PyPEEC-gated fixtures below extract L. The references solve at
/// this frequency too (`FREQ_AC` in `tools/pypeec_reference.py`), which
/// `assert_reference_geometry` checks against the `freq_ac_hz` they declare.
const FREQ_HZ: f64 = 1.0e3;

// The sheet both PyPEEC-gated plane fixtures are built on: 1.2 × 0.8 ×
// 0.02 mm copper. Declared once, so the slot gate and the contact gates
// cannot drift onto different sheets while both keep passing — the rule
// `tests/common/mod.rs` and `benches/support.rs` already apply to fixtures
// shared across crates, applied here to one shared within a file.
/// Minimum corner of the gated sheet, metres.
const SHEET_LO: [f64; 2] = [0.0, 0.0];
/// Maximum corner of the gated sheet, metres.
const SHEET_HI: [f64; 2] = [1.2e-3, 0.8e-3];
/// Thickness of every copper sheet in this file, metres.
const SHEET_T: f64 = 20e-6;
/// The slot the slotted variant cuts into that sheet: 0.2 × 0.64 mm,
/// reaching in from the `y = SHEET_LO[1]` edge.
const SLOT_LO: [f64; 2] = [0.5e-3, 0.0];
const SLOT_HI: [f64; 2] = [0.7e-3, 0.64e-3];

/// The PyPEEC references state their own copy of a fixture's geometry; this
/// is the one check that it matches the constants above. Shared with
/// `tests/spiral_validation.rs`, which runs the same check on the spiral.
#[allow(dead_code)]
mod common;
use common::assert_reference_geometry;

/// The sheet itself, as `assert_reference_geometry` field/value pairs.
fn sheet_fields() -> Vec<(&'static str, f64)> {
    vec![
        ("lo_m[0]", SHEET_LO[0]),
        ("lo_m[1]", SHEET_LO[1]),
        ("hi_m[0]", SHEET_HI[0]),
        ("hi_m[1]", SHEET_HI[1]),
        ("thickness_m", SHEET_T),
        ("sigma_s_per_m", SIGMA),
    ]
}

/// What a `plane` reference must declare: the sheet, with the slot cut.
fn slotted_plane_fields() -> Vec<(&'static str, f64)> {
    let mut fields = sheet_fields();
    fields.extend([
        ("slot_count", 1.0),
        ("slot_lo_m[0]", SLOT_LO[0]),
        ("slot_lo_m[1]", SLOT_LO[1]),
        ("slot_hi_m[0]", SLOT_HI[0]),
        ("slot_hi_m[1]", SLOT_HI[1]),
    ]);
    fields
}

/// What a `plane-solid` reference must declare: the same sheet, uncut.
/// `slot_count` is what stops either half of the pair standing in for the
/// other — the differential is meaningless if both solved the same sheet.
fn solid_plane_fields() -> Vec<(&'static str, f64)> {
    let mut fields = sheet_fields();
    fields.push(("slot_count", 0.0));
    fields
}

/// What a `contact` / `contact-near` reference must declare: the uncut
/// sheet, the landing pad, and the two landing centres this separation
/// drives.
fn contact_fields(landings: [[f64; 2]; 2]) -> Vec<(&'static str, f64)> {
    let mut fields = sheet_fields();
    fields.extend([
        ("slot_count", 0.0),
        ("pad_m", CONTACT_FINE),
        ("landings_m[0][0]", landings[0][0]),
        ("landings_m[0][1]", landings[0][1]),
        ("landings_m[1][0]", landings[1][0]),
        ("landings_m[1][1]", landings[1][1]),
    ]);
    fields
}

/// A contact reference describes this fixture: the declared geometry above,
/// plus the claim the landing constants make in prose — that every centre is
/// a voxel centre of the reference's own grid, so the reference drives
/// exactly the points fasterhenry drives.
fn check_contact_reference(reference: &serde_json::Value, fixture: &str, landings: [[f64; 2]; 2]) {
    assert_reference_geometry(reference, fixture, FREQ_HZ, &contact_fields(landings));
    let voxel = reference["voxel_size_um"]
        .as_f64()
        .unwrap_or_else(|| panic!("the {fixture:?} reference declares no `voxel_size_um`"))
        * 1e-6;
    for landing in landings {
        for (axis, coordinate) in landing.iter().enumerate() {
            let index = (coordinate - SHEET_LO[axis]) / voxel - 0.5;
            assert!(
                (index - index.round()).abs() < 1e-6,
                "landing {landing:?} is not a voxel centre of the {fixture:?} reference's \
                 {:.3} um grid on axis {axis}: the reference drives a different point \
                 than fasterhenry does",
                voxel * 1e6
            );
        }
    }
}

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
        equipotentials: Vec::new(),
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

/// The PyPEEC solid-plane reference, if generated (`--fixture plane-solid`).
fn pypeec_plane_solid_reference() -> Option<serde_json::Value> {
    let path = std::env::var("FASTERHENRY_PYPEEC_PLANE_SOLID_REFERENCE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../tools/pypeec_plane_solid_reference.json")
        });
    path.exists()
        .then(|| serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap())
}

/// Every reference present describes the fixture this file would compare it
/// against — checked without paying for a single solve.
///
/// The three reference-backed gates below each run this check before
/// comparing impedances, but two of them are release-only, so in a debug
/// build a drift would otherwise go unreported until CI. This runs in every
/// profile and is the cheapest place a drift can surface.
#[test]
fn the_references_in_place_describe_these_fixtures() {
    let mut checked = 0;
    if let Some(reference) = pypeec_plane_reference() {
        assert_reference_geometry(&reference, "plane", FREQ_HZ, &slotted_plane_fields());
        checked += 1;
    }
    if let Some(reference) = pypeec_plane_solid_reference() {
        assert_reference_geometry(&reference, "plane-solid", FREQ_HZ, &solid_plane_fields());
        checked += 1;
    }
    for (near, fixture, landings) in [
        (false, "contact", CONTACT_FAR),
        (true, "contact-near", CONTACT_NEAR),
    ] {
        if let Some(reference) = pypeec_contact_reference(near) {
            check_contact_reference(&reference, fixture, landings);
            checked += 1;
        }
    }
    if checked == 0 {
        eprintln!("SKIPPED (no pypeec plane or contact reference present)");
        return;
    }
    println!("{checked} pypeec reference(s) describe this file's fixtures");
}

/// The plane-with-slot fixture (1.2 × 0.8 × 0.02 mm copper, 0.2 × 0.64 mm
/// slot) as a solvable system: port between the cell nodes nearest the
/// two short ends.
fn slotted_plane(nx: usize, ny: usize, slotted: bool) -> (Geometry, Vec<Port>, Discretization) {
    let mut geometry = Geometry::new();
    let holes = if slotted {
        vec![fasterhenry::plane::Hole::Rect {
            lo: SLOT_LO,
            hi: SLOT_HI,
        }]
    } else {
        Vec::new()
    };
    let plane = GroundPlane {
        lo: SHEET_LO,
        hi: SHEET_HI,
        z_top: SHEET_T,
        thickness: SHEET_T,
        nx,
        ny,
        sigma: SIGMA,
        holes,
        contacts: Vec::new(),
        equipotentials: Vec::new(),
    };
    let centres = plane.build_into(&mut geometry).unwrap();
    // The two short ends, at mid-height and mid-thickness of the sheet.
    let (mid_y, mid_z) = (SHEET_HI[1] / 2.0, SHEET_T / 2.0);
    let west = plane
        .attach(&centres, [SHEET_LO[0] + 1e-9, mid_y, mid_z])
        .unwrap();
    let east = plane
        .attach(&centres, [SHEET_HI[0] - 1e-9, mid_y, mid_z])
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
        let result = solve(&system.0, &system.1, &system.2, &[0.0, FREQ_HZ]).unwrap();
        let z_dc = result.impedance_ohm[0][(0, 0)];
        let z_ac = result.impedance_ohm[1][(0, 0)];
        (z_ac.im / (std::f64::consts::TAU * FREQ_HZ), z_dc.re)
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
            let r = solve(&s.0, &s.1, &s.2, &[FREQ_HZ]).unwrap();
            r.impedance_ohm[0][(0, 0)].im / (std::f64::consts::TAU * FREQ_HZ)
        };
        let coarse = l(48, 32, true) - l(48, 32, false);
        let grid_relative = (coarse - dl).abs() / dl;
        println!("dL grid rel (48x32 vs 96x64) {grid_relative:.4}");
        assert!(grid_relative < 0.07, "dL grid sensitivity {grid_relative}");
    }

    match pypeec_plane_reference() {
        Some(reference) => {
            let Some(solid) = pypeec_plane_solid_reference() else {
                eprintln!("SKIPPED (solid-plane reference absent)");
                return;
            };
            // Both references must have solved this sheet, and the pair must
            // differ by exactly the slot (issue #369).
            assert_reference_geometry(&reference, "plane", FREQ_HZ, &slotted_plane_fields());
            assert_reference_geometry(&solid, "plane-solid", FREQ_HZ, &solid_plane_fields());
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

// The slot fixture's sheet — [`SHEET_HI`] × [`SHEET_T`] — driven between
// two via landings inside it instead of across its short ends: the port
// impedance is then dominated by the current crowding under the contacts,
// which is exactly what a graded contact region has to resolve. It is the
// *same* sheet, so it is the same constants: restating its extent here is
// what let the two fixtures drift onto different sheets.
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
/// drives exactly the same two points — and a voxel centre of the PyPEEC
/// reference's grid, so the reference drives them too. Both halves of that
/// are asserted: the cell-centre half in `contact_fixture`, the voxel-centre
/// half against the grid the reference declares, in the gate below (#369).
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
            (SHEET_HI[0] / pitch).round() as usize,
            (SHEET_HI[1] / pitch).round() as usize,
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
        lo: SHEET_LO,
        hi: SHEET_HI,
        z_top: SHEET_T,
        thickness: SHEET_T,
        nx: cells[0],
        ny: cells[1],
        sigma: SIGMA,
        holes: Vec::new(),
        contacts,
        equipotentials: Vec::new(),
    };
    (plane, landings)
}

/// The contact fixture as a solvable system, plus the plane's bar count.
fn contact_fixture(mesh: ContactMesh, far: bool) -> (Geometry, Vec<Port>, Discretization, usize) {
    let (plane, landings) = contact_plane(mesh, far);
    let mut geometry = Geometry::new();
    let centres = plane.build_into(&mut geometry).unwrap();
    let bars = geometry.segment_count();
    let depth = SHEET_T / 2.0;
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
    let result = solve(&geometry, &ports, &discretization, &[0.0, FREQ_HZ]).unwrap();
    let z_dc = result.impedance_ohm[0][(0, 0)];
    let z_ac = result.impedance_ohm[1][(0, 0)];
    (z_ac.im / (std::f64::consts::TAU * FREQ_HZ), z_dc.re, bars)
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
    // Both references must have solved this sheet, with this pad, driven
    // between exactly the landings fasterhenry drives (issue #369).
    check_contact_reference(&far_reference, "contact", CONTACT_FAR);
    check_contact_reference(&near_reference, "contact-near", CONTACT_NEAR);

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

// ---------------------------------------------------------------------------
// Equipotential contacts (issue #101): a landing that is a pad, not a point
// ---------------------------------------------------------------------------

/// A small copper sheet — [`SHEET_T`] thick, like the fixtures above, but
/// its own extent — driven between two 100 µm landings, each resolved by its
/// own graded contact region. The sheet is deliberately smaller than the
/// fixture above (82 and 162 bars, four DC solves, ~15 s unoptimized)
/// so this gate runs in a debug build: what it measures is a *difference
/// between two models of the same mesh*, not an accuracy claim that would
/// need a fine one.
const PAD_HI: [f64; 2] = [0.6e-3, 0.4e-3];
const PAD: [f64; 2] = [100e-6, 100e-6];
const PAD_LANDINGS: [[f64; 2]; 2] = [[0.15e-3, 0.2e-3], [0.45e-3, 0.2e-3]];

/// `(R at DC, plane bars, cells tied per pad)` for that sheet with each
/// landing's region cut into `cells` fine cells per axis — driven either
/// between the two nearest cell-centre nodes (`pad = false`: the landing
/// this engine has always had) or between two `Equipotential` pads covering
/// the same two rectangles (`pad = true`).
fn pad_resistance(cells: usize, pad: bool) -> (f64, usize, usize) {
    let corners = |landing: [f64; 2]| {
        (
            [landing[0] - PAD[0] / 2.0, landing[1] - PAD[1] / 2.0],
            [landing[0] + PAD[0] / 2.0, landing[1] + PAD[1] / 2.0],
        )
    };
    let mut plane = GroundPlane {
        lo: [0.0, 0.0],
        hi: PAD_HI,
        z_top: SHEET_T,
        thickness: SHEET_T,
        nx: 6,
        ny: 4,
        sigma: SIGMA,
        holes: Vec::new(),
        contacts: PAD_LANDINGS
            .iter()
            .map(|&landing| {
                let (lo, hi) = corners(landing);
                fasterhenry::plane::ContactRegion::new(lo, hi, [cells; 2], CONTACT_RATIO)
            })
            .collect(),
        equipotentials: Vec::new(),
    };
    if pad {
        for landing in PAD_LANDINGS {
            let (lo, hi) = corners(landing);
            plane
                .equipotentials
                .push(fasterhenry::plane::Equipotential::new(lo, hi));
        }
    }
    let mut geometry = Geometry::new();
    let centres = plane.build_into(&mut geometry).unwrap();
    let bars = geometry.segment_count();
    let mut ends = Vec::new();
    for (index, landing) in PAD_LANDINGS.iter().enumerate() {
        ends.push(if pad {
            plane
                .equipotential_node(&centres, index)
                .unwrap()
                .expect("each pad ties at least one live cell")
        } else {
            plane
                .attach(&centres, [landing[0], landing[1], SHEET_T / 2.0])
                .unwrap()
        });
    }
    // How many cells the first landing's node speaks for, for the report.
    let mesh = plane.mesh().unwrap();
    let tied = (0..mesh.nx())
        .flat_map(|i| (0..mesh.ny()).map(move |j| (i, j)))
        .filter(|&(i, j)| centres[i][j] == Some(ends[0]))
        .count();
    let ports = vec![Port::new(ends[0], ends[1]).named("pad")];
    let result = solve(
        &geometry,
        &ports,
        &Discretization::Uniform(Subdivision::SINGLE),
        &[0.0],
    )
    .unwrap();
    (result.impedance_ohm[0][(0, 0)].re, bars, tied)
}

/// What an equipotential contact buys, measured on the same mesh: current
/// enters the pad across its whole boundary instead of through one cell, so
/// the port's DC resistance is lower than a single-node landing's — and,
/// unlike that landing's, it barely moves when the landing's own cells are
/// refined. A point contact's constriction resistance grows without bound
/// as the cell shrinks (it is the mesh, not the geometry, that sets it);
/// the pad's is set by the rectangle the deck named, which is the whole
/// reason `contact equiv_rect` exists.
#[test]
fn an_equipotential_pad_lowers_resistance_and_stops_the_mesh_setting_it() {
    let (coarse_point, coarse_bars, coarse_cells) = pad_resistance(2, false);
    let (fine_point, fine_bars, fine_cells) = pad_resistance(3, false);
    let (coarse_pad, _, coarse_tied) = pad_resistance(2, true);
    let (fine_pad, _, fine_tied) = pad_resistance(3, true);
    println!(
        "point landing: {coarse_point:.6} ohm ({coarse_bars} bars, {coarse_cells} cell) -> \
         {fine_point:.6} ohm ({fine_bars} bars, {fine_cells} cell); pad: {coarse_pad:.6} ohm \
         ({coarse_tied} cells tied) -> {fine_pad:.6} ohm ({fine_tied} cells tied)"
    );
    assert_eq!((coarse_cells, fine_cells), (1, 1), "a landing is one cell");
    assert_eq!((coarse_tied, fine_tied), (4, 9), "a pad is its rectangle");
    for (point, pad, cells) in [(coarse_point, coarse_pad, 2), (fine_point, fine_pad, 3)] {
        assert!(
            pad < point,
            "{cells} cells across the landing: the pad must beat the point \
             landing ({pad} vs {point})"
        );
    }
    // Refining the landing tightens the point contact's constriction…
    assert!(
        fine_point > coarse_point * 1.03,
        "the point landing's resistance must climb as its cell shrinks \
         ({coarse_point} -> {fine_point})"
    );
    // …while the pad, whose size the deck stated, stays where it was.
    let drift = (fine_pad - coarse_pad).abs() / coarse_pad;
    assert!(
        drift < 0.05,
        "the pad's resistance must not follow the mesh ({coarse_pad} -> {fine_pad}, {drift})"
    );
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
