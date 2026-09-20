//! The `fasterhenry` command-line front end: FastHenry-format `.inp` decks
//! (the public file format) or JSON problem documents in, JSON out.
//!
//! The binary logic lives in this library so the tests can drive it without
//! spawning a process; `src/main.rs` is a thin wrapper.

pub mod cli;
pub mod inp;
pub mod problem;

pub use problem::Problem;

use std::path::Path;

use fasterhenry::{solve, SweepResult};

/// Reads a problem from a file: `.inp`/`.fh` decks by extension, anything
/// else as a JSON problem document.
pub fn read_inputs(path: &Path) -> Result<Problem, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("inp") | Some("fh") => inp::parse(&text)
            .map(Problem::from)
            .map_err(|error| error.to_string()),
        _ => serde_json::from_str(&text)
            .map_err(|error| format!("invalid problem document: {error}")),
    }
}

/// Runs the sweep. `frequency_override`, when given, replaces the problem's
/// frequencies (the CLI's `--freq fmin fmax ndec`).
pub fn run(problem: &Problem, frequency_override: Option<Vec<f64>>) -> Result<SweepResult, String> {
    let frequencies = frequency_override.unwrap_or_else(|| problem.frequencies_hz.clone());
    if frequencies.is_empty() {
        return Err(
            "no frequencies: give .freq in the deck, frequencies_hz in the document, or --freq"
                .to_string(),
        );
    }
    if problem.ports.is_empty() {
        return Err("no ports: give .external in the deck or ports in the document".to_string());
    }
    solve(
        &problem.geometry,
        &problem.ports,
        &problem.discretization,
        &frequencies,
    )
    .map_err(|error| error.to_string())
}

/// Writes `result` as a human-readable text matrix — one block per
/// frequency — for tools that expect a `Zc.mat`-style file.
///
/// This is **not** FastHenry's binary MATLAB `Zc.mat`; it is a documented
/// plain-text format of our own (the JSON output is the primary,
/// tool-readable form). Port `i` is row `i`; entries are `re + j im` ohms.
pub fn write_zc_text(result: &SweepResult) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let ports: Vec<&str> = result
        .ports
        .iter()
        .map(|port| port.name.as_deref().unwrap_or("?"))
        .collect();
    let _ = writeln!(
        out,
        "# fasterhenry Zc text matrix (not FastHenry's binary Zc.mat)"
    );
    let _ = writeln!(out, "# ports: {}", ports.join(", "));
    for (index, &frequency) in result.frequencies_hz.iter().enumerate() {
        let z = &result.impedance_ohm[index];
        let _ = writeln!(out, "frequency {:.17e} Hz", frequency);
        for row in z.row_iter() {
            let entries: Vec<String> = row
                .iter()
                .map(|z| {
                    if z.im < 0.0 {
                        format!("{:.17e} - j {:.17e}", z.re, -z.im)
                    } else {
                        format!("{:.17e} + j {:.17e}", z.re, z.im)
                    }
                })
                .collect();
            let _ = writeln!(out, "  {}", entries.join("  "));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn cli_definition_is_valid() {
        use clap::CommandFactory;
        crate::cli::Cli::command().debug_assert();
    }
}
