//! Geometry fixtures shared by more than one integration test.
//!
//! `cargo test` compiles every `tests/*.rs` as its own crate, so a fixture
//! two of them need used to exist twice — once in the test that validates
//! the physics on it, once in the test that reuses the same geometry for a
//! different question. Nothing checked that the copies agreed, so an edit
//! to one left the other guarding a geometry it no longer named. A file in
//! a subdirectory of `tests/` is not itself a test target, so declaring
//! `mod common;` pulls this one definition into each crate that wants it.
//!
//! Each builder returns the `(Geometry, Vec<Port>, Discretization)` triple
//! `MeshSystem::assemble` and `solve` take, which is also all
//! `tests/pfft.rs` needs: it assembles and takes `.filaments()`. Port names
//! do not reach the filaments, so the named ports serve every caller.
//!
//! Current users:
//!
//! * [`coupled_structure`] — `tests/solve.rs`, `tests/pfft.rs`;
//! * [`spiral_system`] — `tests/spiral_validation.rs`, `tests/pfft.rs`;
//! * [`assert_reference_geometry`] — `tests/spiral_validation.rs`,
//!   `tests/plane_validation.rs`. Not a fixture: the one check that a
//!   PyPEEC reference solved the geometry the gate reading it is built on
//!   (issue #369). It lives here for the same reason the fixtures do —
//!   two test crates need it, and a copy could drift.
//!
//! Declare it with `#[allow(dead_code)]`: no single test crate uses all of
//! it, and an unused item in a private module is a warning the workspace
//! denies.

use fasterhenry::{Discretization, Geometry, Node, NodeId, Port, SegmentDef, Subdivision};

/// Copper conductivity, siemens per metre.
pub const COPPER: f64 = 5.8e7;

// ---------------------------------------------------------------------------
// The coupled structure
// ---------------------------------------------------------------------------

/// Two coupled traces of different sizes with bundles, a bend, a closed ring
/// (so the loop basis has a conductor loop) and a floating bar.
///
/// 11 nodes, 8 segments and 41 filaments at the subdivisions below, of which
/// the ring contributes the one conductor loop.
pub fn coupled_structure() -> (Geometry, Vec<Port>, Discretization) {
    let points = [
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
    ];
    let segments = [
        (0, 1, 0.4e-3, 0.1e-3),
        (1, 2, 0.4e-3, 0.1e-3),
        (3, 4, 0.2e-3, 0.05e-3),
        (5, 6, 0.2e-3, 0.1e-3),
        (6, 7, 0.2e-3, 0.1e-3),
        (7, 8, 0.2e-3, 0.1e-3),
        (8, 5, 0.2e-3, 0.1e-3),
        (9, 10, 0.2e-3, 0.1e-3),
    ];
    let mut geometry = Geometry::new();
    for point in points {
        geometry.add_node(Node::from(point)).unwrap();
    }
    for (a, b, width, height) in segments {
        geometry
            .add_segment(SegmentDef::new(NodeId(a), NodeId(b), width, height, COPPER))
            .unwrap();
    }
    let ports = vec![
        Port::new(NodeId(0), NodeId(2)).named("A"),
        Port::new(NodeId(4), NodeId(3)).named("B"),
        Port::new(NodeId(5), NodeId(7)).named("ring diagonal"),
    ];
    let mut subdivisions = vec![Subdivision::new(2, 2); segments.len()];
    subdivisions[0] = Subdivision::new(4, 3);
    subdivisions[1] = Subdivision::new(3, 3);
    subdivisions[2] = Subdivision::new(3, 1);
    subdivisions[5] = Subdivision::SINGLE;
    (geometry, ports, Discretization::PerSegment(subdivisions))
}

// ---------------------------------------------------------------------------
// The 2-turn square spiral
// ---------------------------------------------------------------------------

/// Metres per micrometre: the spiral is laid out in µm and built in metres.
pub const UM: f64 = 1e-6;
/// Conductor width of [`spiral_system`], in micrometres.
pub const W_UM: f64 = 2.0;
/// Conductor thickness of [`spiral_system`], in micrometres.
pub const T_UM: f64 = 2.0;
/// Turns of [`spiral_system`].
pub const SPIRAL_TURNS: usize = 2;
/// Starting side of [`spiral_system`], in micrometres.
pub const SPIRAL_START_UM: f64 = 60.0;
/// Pitch growth per turn of [`spiral_system`], in micrometres.
pub const SPIRAL_PITCH_UM: f64 = 15.0;

/// A centreline, as its two endpoints in the µm plane the spiral is laid
/// out in.
pub type Segment = ((f64, f64), (f64, f64));

/// The standard non-self-intersecting square spiral (winding order).
pub fn square_spiral_segments(turns: usize, start_len_um: f64, pitch_um: f64) -> Vec<Segment> {
    let directions = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)];
    let mut position = (0.0, 0.0);
    let mut length = start_len_um;
    let mut segments = Vec::new();
    for k in 0..turns * 4 {
        let direction = directions[k % 4];
        let end = (
            position.0 + direction.0 * length,
            position.1 + direction.1 * length,
        );
        segments.push((position, end));
        position = end;
        if k % 2 == 1 {
            length += pitch_um;
        }
    }
    segments
}

/// The 2-turn square spiral as a solvable geometry: starting side 60 µm,
/// 15 µm pitch growth per turn (segment lengths 60, 60, 75, 75, 90, 90,
/// 105, 105 µm), [`W_UM`] × [`T_UM`] copper, 8 segments head-to-tail around
/// 9 shared vertices at 4 × 4 filaments each (128 in all), the port closing
/// the spiral from its end back to its start.
///
/// `tests/spiral_validation.rs` documents where the fixture comes from and
/// validates it against independent references.
pub fn spiral_system() -> (Geometry, Vec<Port>, Discretization) {
    let segments = square_spiral_segments(SPIRAL_TURNS, SPIRAL_START_UM, SPIRAL_PITCH_UM);
    let mut nodes = vec![Node::new(0.0, 0.0, T_UM / 2.0 * UM)];
    for &(.., end) in &segments {
        nodes.push(Node::new(end.0 * UM, end.1 * UM, T_UM / 2.0 * UM));
    }
    let defs: Vec<SegmentDef> = (0..segments.len())
        .map(|i| SegmentDef::new(NodeId(i), NodeId(i + 1), W_UM * UM, T_UM * UM, COPPER))
        .collect();
    let geometry = Geometry::from_parts(nodes, defs).unwrap();
    let ports = vec![Port::new(NodeId(0), NodeId(segments.len())).named("spiral")];
    let discretization = Discretization::PerSegment(vec![Subdivision::new(4, 4); segments.len()]);
    (geometry, ports, discretization)
}

// ---------------------------------------------------------------------------
// PyPEEC reference agreement (issue #369)
// ---------------------------------------------------------------------------

/// Assert a PyPEEC reference JSON solved the fixture its reader is about to
/// compare against, at the same frequency, on the same geometry.
///
/// `tools/pypeec_reference.py` cannot share the Rust fixtures: the reference
/// must stay an independent discretization and solve. So each fixture's
/// geometry is stated twice, in two languages — and before #369 the two
/// statements were asserted to agree in prose only. An edit to either copy
/// of a pitch, a landing, a thickness or a conductivity left a 2 %-tolerance
/// gate comparing two *different* problems, and it kept passing until the
/// drift happened to exceed the tolerance.
///
/// What is shared is therefore the geometry *statement*, as data: the script
/// writes what it solved into a `geometry` object (`fixture_geometry`), and
/// this checks it against the constants the caller built its own mesh from.
///
/// `expected` names each field as `name` for a scalar and `name[i]` /
/// `name[i][j]` for an array element. The panic names the first field that
/// disagrees, so a drift reports *which* parameter moved rather than
/// surfacing as an impedance deviation.
pub fn assert_reference_geometry(
    reference: &serde_json::Value,
    fixture: &str,
    freq_ac_hz: f64,
    expected: &[(&str, f64)],
) {
    let declared = reference["fixture"].as_str().unwrap_or("<absent>");
    assert_eq!(
        declared, fixture,
        "the reference file in place declares fixture {declared:?}, but this gate \
         compares against {fixture:?}"
    );
    let declared_freq = reference["freq_ac_hz"]
        .as_f64()
        .unwrap_or_else(|| panic!("the {fixture:?} reference declares no `freq_ac_hz`"));
    assert!(
        (declared_freq - freq_ac_hz).abs() <= 1e-9 * freq_ac_hz,
        "the {fixture:?} reference solved at {declared_freq:e} Hz, this gate extracts \
         L at {freq_ac_hz:e} Hz"
    );
    let geometry = reference.get("geometry").unwrap_or_else(|| {
        panic!(
            "the {fixture:?} reference carries no `geometry` object: regenerate it with \
             tools/pypeec_reference.py, which declares the geometry it solved so this \
             gate can check it against the Rust fixture"
        )
    });
    for &(field, want) in expected {
        let got = reference_field(geometry, field).unwrap_or_else(|| {
            panic!(
                "the {fixture:?} reference's geometry has no numeric `{field}` \
                 (the Rust fixture uses {want:e})"
            )
        });
        let agrees = if want == 0.0 {
            got == 0.0
        } else {
            (got - want).abs() / want.abs() <= 1e-9
        };
        assert!(
            agrees,
            "the {fixture:?} reference solved {field} = {got:e}, the Rust fixture uses \
             {want:e}: the two definitions have drifted apart, so comparing their \
             impedances compares two different problems"
        );
    }
}

/// `geometry[field]` as an `f64`, where `field` is `name`, `name[i]` or
/// `name[i][j]`; `None` if any step is missing or not a number.
fn reference_field(geometry: &serde_json::Value, field: &str) -> Option<f64> {
    let (name, indices) = field.split_once('[').unwrap_or((field, ""));
    let mut node = geometry.get(name)?;
    for index in indices.trim_end_matches(']').split("][") {
        if index.is_empty() {
            continue;
        }
        node = node.get(index.parse::<usize>().ok()?)?;
    }
    node.as_f64()
}
