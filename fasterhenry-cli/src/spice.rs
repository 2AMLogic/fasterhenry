//! SPICE subcircuit export of an impedance matrix — the lumped stamp a
//! tool expects from an extraction run.
//!
//! # Method
//!
//! A port-pair impedance matrix at one frequency, `Z = R + jωL`, is
//! stamped as one branch per port carrying
//!
//! * a series inductor `L_i = L_ii` for the inductive part, coupled to the
//!   other port inductors by `K_ij = L_ij / sqrt(L_ii L_jj)` mutuals
//!   (SPICE `K` elements), and
//! * one current-controlled voltage source per matrix entry, `H_i_j`,
//!   transimpedance `R_ij`, sensing port `j`'s 0 V current meter — the
//!   resistive part, which coupled inductors cannot represent.
//!
//! The partial-inductance matrix is positive semidefinite, so
//! `|K_ij| ≤ 1` holds mathematically; floating-point may land a hair
//! above 1, which is clamped to `±(1 − 1e-12)`. At DC (`f = 0`) the
//! inductors are undefined and the netlist is purely resistive (the
//! inductors become wires, the `K` lines are omitted).
//!
//! Port `i` occupies external nodes `p<i>` (positive) and `n<i>`
//! (negative); current into `p<i>` is the positive port current. The
//! controlling meters are named `vsense<i>`.

use std::fmt::Write;
use std::path::Path;

use fasterhenry::SweepResult;

/// The subcircuit name used when an input path has no usable file stem.
pub const FALLBACK_SUBCKT_NAME: &str = "fasterhenry";

/// The subcircuit name the CLI derives from an input path: the file stem,
/// with every character outside ASCII `[A-Za-z0-9_]` (whitespace, control
/// characters, punctuation including `.`, any non-ASCII character)
/// replaced by one `_`, so the result is always a single netlist token.
/// A stem that is missing or empty yields [`FALLBACK_SUBCKT_NAME`]. A stem
/// that is not valid UTF-8 is read lossily first, so the mapping stays
/// deterministic. Simple stems such as `spiral` pass through unchanged.
pub fn subckt_name_from_path(path: &Path) -> String {
    let name: String = path
        .file_stem()
        .map(|stem| stem.to_string_lossy())
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if name.is_empty() {
        FALLBACK_SUBCKT_NAME.to_owned()
    } else {
        name
    }
}

/// Writes the subcircuit for `result` at frequency index `frequency`:
/// `.subckt <name>` … `.ends`. Returns the text.
pub fn write_spice_subckt(result: &SweepResult, frequency: usize, name: &str) -> String {
    let n = result.ports.len();
    let f = result.frequencies_hz[frequency];
    let z = &result.impedance_ohm[frequency];
    let omega = std::f64::consts::TAU * f;

    let resistance = |i: usize, j: usize| z[(i, j)].re;
    let inductance = |i: usize, j: usize| z[(i, j)].im / omega;

    let mut out = String::new();
    let ports: Vec<String> = (0..n).map(|i| format!("p{} n{}", i + 1, i + 1)).collect();
    let _ = writeln!(out, "* fasterhenry SPICE export: {name} at {f:.17e} Hz");
    let _ = writeln!(
        out,
        "* port current is positive into p<i>; Z from the R (H sources) and L (coupled inductors) stamps"
    );
    let _ = writeln!(out, ".subckt {name} {}", ports.join(" "));

    // One branch per port, all elements in series:
    //   p<i> -- L<i> -- H_i_1 -- H_i_2 -- … -- vsense<i> -- n<i>
    // The intermediate nodes exist only to chain the elements.
    for i in 0..n {
        let l = if f > 0.0 { inductance(i, i) } else { 0.0 };
        let _ = writeln!(out, "L{} p{} a{}_0 {:.17e}", i + 1, i + 1, i, l);
        let mut node = format!("a{}_0", i);
        for j in 0..n {
            let r = resistance(i, j);
            if r != 0.0 {
                let next = format!("a{}_{}", i, j + 1);
                let _ = writeln!(
                    out,
                    "H{}_{} {node} {next} vsense{} {:.17e}",
                    i + 1,
                    j + 1,
                    j + 1,
                    r
                );
                node = next;
            }
        }
        let _ = writeln!(out, "vsense{} {node} n{} 0", i + 1, i + 1);
    }

    // Mutual couplings between the port inductors.
    if f > 0.0 {
        for i in 0..n {
            for j in (i + 1)..n {
                let denominator = (inductance(i, i) * inductance(j, j)).sqrt();
                let coupling = (inductance(i, j) / denominator).clamp(-(1.0 - 1e-12), 1.0 - 1e-12);
                let _ = writeln!(
                    out,
                    "K{}_{} L{} L{} {:.17e}",
                    i + 1,
                    j + 1,
                    i + 1,
                    j + 1,
                    coupling
                );
            }
        }
    }

    let _ = writeln!(out, ".ends {name}");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(path: &str) -> String {
        subckt_name_from_path(Path::new(path))
    }

    #[test]
    fn simple_stems_are_unchanged() {
        assert_eq!(name("spiral.inp"), "spiral");
        assert_eq!(name("dir/two_turn_2.json"), "two_turn_2");
    }

    #[test]
    fn unsafe_characters_become_underscores() {
        assert_eq!(name("my coil.inp"), "my_coil");
        assert_eq!(name("a\tb\nc.inp"), "a_b_c");
        assert_eq!(name("v1.2-final(x).inp"), "v1_2_final_x_");
        assert_eq!(name("bobine\u{e9}.inp"), "bobine_");
        assert_eq!(name("\u{7dda}\u{5708}.inp"), "__");
        assert_eq!(name("x\n.ends\n.subckt evil.inp"), "x__ends__subckt_evil");
    }

    #[test]
    fn missing_stem_falls_back() {
        assert_eq!(name(""), FALLBACK_SUBCKT_NAME);
        assert_eq!(name("/"), FALLBACK_SUBCKT_NAME);
        assert_eq!(name(".."), FALLBACK_SUBCKT_NAME);
    }
}
