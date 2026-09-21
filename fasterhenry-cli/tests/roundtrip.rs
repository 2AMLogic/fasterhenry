//! Round-trip validation: the self-authored two-turn spiral as an `.inp`
//! deck (`tests/data/spiral.inp`) and as the equivalent JSON problem
//! document (`tests/data/spiral.json`) must produce identical sweeps.

use fasterhenry_cli::{read_inputs, run};
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

#[test]
fn spiral_inp_and_json_agree_bitwise() {
    let from_inp = read_inputs(&fixture("spiral.inp")).expect("deck parses");
    let from_json = read_inputs(&fixture("spiral.json")).expect("document parses");

    // The same problem, field by field — the deck's .equiv-compacted node
    // list, aliased port, per-segment overrides and unit-converted sigma
    // all line up with the SI document.
    assert_eq!(from_inp.geometry, from_json.geometry);
    assert_eq!(from_inp.ports, from_json.ports);
    assert_eq!(from_inp.discretization, from_json.discretization);
    assert_eq!(from_inp.frequencies_hz, from_json.frequencies_hz);

    let z_from_inp = run(&from_inp, None).expect("inp solve");
    let z_from_json = run(&from_json, None).expect("json solve");
    assert_eq!(
        z_from_inp.without_timing(),
        z_from_json.without_timing(),
        "the .inp deck and the JSON document must produce identical sweeps"
    );
}

#[test]
fn spiral_result_is_physical() {
    let problem = read_inputs(&fixture("spiral.inp")).unwrap();
    let result = run(&problem, None).unwrap();
    assert_eq!(result.frequencies_hz.len(), 4);

    let dc = run(&problem, Some(vec![0.0])).unwrap();
    // DC is a positive resistance, exactly real.
    assert_eq!(dc.impedance_ohm[0][(0, 0)].im, 0.0);
    assert!(dc.impedance_ohm[0][(0, 0)].re > 0.0);

    // A 2-turn 4 mm square spiral of 0.2 mm copper: a few tens of nH at RF
    // (order-of-magnitude sanity, not a reference value).
    let rf = &result.impedance_ohm[3];
    let omega = std::f64::consts::TAU * result.frequencies_hz[3];
    let inductance = rf[(0, 0)].im / omega;
    assert!(
        (5e-9..=200e-9).contains(&inductance),
        "spiral inductance {inductance:.3e} H outside the physical range"
    );
    // The skin effect raises the real part above DC.
    assert!(rf[(0, 0)].re > dc.impedance_ohm[0][(0, 0)].re);
}

#[test]
fn frequency_override_replaces_the_sweep() {
    let problem = read_inputs(&fixture("spiral.inp")).unwrap();
    let result = run(&problem, Some(vec![1e6])).unwrap();
    assert_eq!(result.frequencies_hz, vec![1e6]);
    let doubled = run(&problem, Some(vec![1e6, 1e6])).unwrap();
    assert_eq!(doubled.frequencies_hz, vec![1e6, 1e6]);
}

#[test]
fn mat4_file_carries_the_whole_sweep() {
    let problem = read_inputs(&fixture("spiral.inp")).unwrap();
    let result = run(&problem, Some(vec![0.0, 1e6])).unwrap();
    let mut bytes = Vec::new();
    fasterhenry_cli::mat::write_zc_mat(&mut bytes, &result).unwrap();
    // freqs (1x2 real) + Zc_1, Zc_2 (1x1 complex): each record is a
    // 20-byte header + the exact name bytes; data 2*8 for freqs and
    // (8+8) per Zc_k. 41 + 40 + 40.
    assert_eq!(bytes.len(), 41 + 40 + 40);
    // mopt = 0 and imagf as declared for every record.
    assert_eq!(&bytes[0..4], &0i32.to_le_bytes());
    assert_eq!(&bytes[41..45], &0i32.to_le_bytes());
    assert_eq!(&bytes[81..85], &0i32.to_le_bytes());
    assert_eq!(&bytes[12..16], &0i32.to_le_bytes()); // imagf of freqs
    assert_eq!(&bytes[53..57], &1i32.to_le_bytes()); // imagf of Zc_1
    assert_eq!(&bytes[20..25], b"freqs");
}

#[test]
fn spice_subcircuit_stamps_r_and_l() {
    let problem = read_inputs(&fixture("spiral.inp")).unwrap();
    let result = run(&problem, Some(vec![1e6])).unwrap();
    let text = fasterhenry_cli::spice::write_spice_subckt(&result, 0, "spiral");
    assert!(text.contains(".subckt spiral p1 n1"));
    assert!(text.contains(".ends spiral"));
    assert!(text.contains("L1 p1 a0_0 "));
    assert!(text.contains("vsense1 a0_1 n1 0"));
    assert!(text.contains("H1_1 a0_0 a0_1 vsense1 "));
    // One port: no K lines.
    assert!(!text.contains("\nK"));

    // A coupled two-port fixture exercises the K stamp.
    use fasterhenry::geometry::{Geometry, Node, NodeId, SegmentDef};
    use fasterhenry::mesh::Port;
    use fasterhenry::solve::{Discretization, Subdivision};
    let um = 1e-6;
    let mut nodes = Vec::new();
    for point in [
        [0.0, 0.0, 0.0],
        [100.0 * um, 0.0, 0.0],
        [10.0 * um, 10.0 * um, 0.0],
        [10.0 * um, -100.0 * um - 10.0 * um, 0.0],
    ] {
        nodes.push(Node::new(point[0], point[1], point[2]));
    }
    let geometry = Geometry::from_parts(
        nodes,
        vec![
            SegmentDef::new(NodeId(0), NodeId(1), 5.0 * um, 2.0 * um, 5.8e7),
            SegmentDef::new(NodeId(2), NodeId(3), 5.0 * um, 2.0 * um, 5.8e7),
        ],
    )
    .unwrap();
    let coupled = fasterhenry_cli::Problem {
        geometry,
        ports: vec![
            Port::new(NodeId(0), NodeId(1)),
            Port::new(NodeId(2), NodeId(3)),
        ],
        discretization: Discretization::Uniform(Subdivision::new(2, 2)),
        frequencies_hz: vec![],
    };
    let result = run(&coupled, Some(vec![1e6])).unwrap();
    let text = fasterhenry_cli::spice::write_spice_subckt(&result, 0, "twobar");
    assert!(text.contains("K1_2 L1 L2 "));
}

#[test]
fn json_document_rejects_unknown_fields() {
    let text = std::fs::read_to_string(fixture("spiral.json")).unwrap();
    let typo = text.replace("\"ports\":", "\"freq\": [1e6], \"ports\":");
    let error = serde_json::from_str::<fasterhenry_cli::Problem>(&typo).unwrap_err();
    assert!(error.to_string().contains("freq"), "{error}");
}

#[test]
fn missing_ports_and_frequencies_are_clear_errors() {
    let mut problem = read_inputs(&fixture("spiral.inp")).unwrap();
    problem.ports.clear();
    assert!(run(&problem, None).unwrap_err().contains("no ports"));
    problem.frequencies_hz.clear();
    assert!(run(&problem, None).unwrap_err().contains("no frequencies"));
}

use fasterhenry::geometry::{Geometry, Node, NodeId, SegmentDef};
use fasterhenry::mesh::Port;
use fasterhenry::solve::{Discretization, Subdivision};

/// The deck path and the library-API path build identical trace-over-plane
/// systems: same plane mesh, same snapped vias, same Z — bit for bit.
#[test]
fn plane_deck_matches_the_api_fixture() {
    use fasterhenry::plane::GroundPlane;
    use fasterhenry::solve::solve;

    let deck = read_inputs_from_text(
        "\
.units mm
.default sigma=5.8e4
Gp 0 -3 0 10 3 0 0.035 nx=20 ny=6
n1 x=1 y=0 z=0.5
n2 x=9 y=0 z=0.5
n3 x=1 y=0 z=0
n4 x=9 y=0 z=0
e1 n1 n2 w=0.2 h=0.035 nwinc=2
e2 n1 n3 w=0.2 h=0.035
e3 n2 n4 w=0.2 h=0.035
.external n1 n2
.freq fmin=1e9 fmax=1e9 ndec=1
.end
",
    )
    .expect("deck parses");

    // The same system through the library API directly.
    // Values computed exactly as the deck computes them (mm * 1e-3,
    // sigma / unit): a 1-ulp difference anywhere breaks bit equality.
    let (w, t, sigma) = (0.2 * 1e-3, 0.035 * 1e-3, 5.8e4 / 1e-3);
    let mut api = Geometry::new();
    let plane = GroundPlane {
        lo: [0.0, -3.0e-3],
        hi: [10.0e-3, 3.0e-3],
        z_top: 0.0,
        thickness: t,
        nx: 20,
        ny: 6,
        sigma,
        holes: Vec::new(),
    };
    let centres = plane.build_into(&mut api).unwrap();
    // Positions computed exactly as the deck computes them (mm * 1e-3):
    // a 1-ulp difference flips the snap tie at x = 9 mm.
    let (xa, xb, zt) = (1.0 * 1e-3, 9.0 * 1e-3, 0.5 * 1e-3);
    let snap_a = plane.attach(&centres, [xa, 0.0, 0.0]).unwrap();
    let snap_b = plane.attach(&centres, [xb, 0.0, 0.0]).unwrap();
    let a = NodeId(api.add_node(Node::new(xa, 0.0, zt)).unwrap().0);
    let b = NodeId(api.add_node(Node::new(xb, 0.0, zt)).unwrap().0);
    // Deck order: trace (e1), then the vias (e2, e3).
    api.add_segment(SegmentDef::new(a, b, w, t, sigma)).unwrap();
    for (top, bottom) in [(a, snap_a), (b, snap_b)] {
        api.add_segment(SegmentDef::new(top, bottom, w, t, sigma))
            .unwrap();
    }
    let mut subdivisions = vec![Subdivision::new(1, 1); api.segment_count()];
    subdivisions[api.segment_count() - 3] = Subdivision::new(2, 1);
    let api_result = solve(
        &api,
        &[Port::new(a, b)],
        &Discretization::PerSegment(subdivisions),
        &[1e9],
    )
    .unwrap();

    let deck_result = run(&deck, None).unwrap();
    assert_eq!(deck_result.frequencies_hz, api_result.frequencies_hz);
    let z_deck = deck_result.impedance_ohm[0][(0, 0)];
    let z_api = api_result.impedance_ohm[0][(0, 0)];
    assert_eq!(z_deck, z_api, "deck and API paths must agree bit for bit");
    let counts = &deck_result.provenance.counts;
    assert_eq!(counts.segments, 20 * 6 * 2 - 20 - 6 + 3, "plane bars + 3");
    assert_eq!(
        counts.nodes,
        20 * 6 + 2,
        "cells + trace nodes; via nodes snapped away"
    );
}

/// Parse helper for inline deck text (the file-based one needs a path).
fn read_inputs_from_text(text: &str) -> Result<fasterhenry_cli::Problem, String> {
    fasterhenry_cli::inp::parse(text)
        .map(fasterhenry_cli::Problem::from)
        .map_err(|error| error.to_string())
}
