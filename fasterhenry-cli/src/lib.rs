//! The `fasterhenry` command-line front end: FastHenry-format `.inp` decks
//! (the public file format) or JSON problem documents in, JSON out.
//!
//! The binary logic lives in this library so the tests can drive it without
//! spawning a process; `src/main.rs` is a thin wrapper.

pub mod cli;
pub mod inp;
pub mod mat;
pub mod problem;
pub mod spice;

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

#[cfg(test)]
mod tests {
    #[test]
    fn cli_definition_is_valid() {
        use clap::CommandFactory;
        crate::cli::Cli::command().debug_assert();
    }
}
