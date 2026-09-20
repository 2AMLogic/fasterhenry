//! Round-trip validation: the self-authored two-turn spiral as an `.inp`
//! deck (`tests/data/spiral.inp`) and as the equivalent JSON problem
//! document (`tests/data/spiral.json`) must produce identical sweeps.

use fasterhenry_cli::{read_inputs, run, write_zc_text};
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
fn zc_text_has_one_block_per_frequency() {
    let problem = read_inputs(&fixture("spiral.inp")).unwrap();
    let result = run(&problem, Some(vec![0.0, 1e6])).unwrap();
    let text = write_zc_text(&result);
    assert_eq!(
        text.matches("frequency ").count(),
        2,
        "one block per frequency:\n{text}"
    );
    assert!(text.contains("not FastHenry's binary Zc.mat"));
    assert!(text.contains("spiral"));
    assert!(text.contains("+ j"));
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
