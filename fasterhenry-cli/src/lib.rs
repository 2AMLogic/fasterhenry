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

use fasterhenry::{MeshSystem, SweepResult};

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

/// Runs the sweep, discarding any coupling-truncation warnings. See
/// [`run_reporting`] to receive them.
pub fn run(problem: &Problem, frequency_override: Option<Vec<f64>>) -> Result<SweepResult, String> {
    run_reporting(problem, frequency_override).map(|(result, _)| result)
}

/// Runs the sweep and returns it together with the assembly's
/// coupling-truncation warnings — the group pairs whose mutual inductance
/// `.couples` dropped even though they are closer than one extent apart, in
/// the order [`fasterhenry::MeshSystem::truncation_warnings`] reports them.
/// The list is empty unless the problem truncates something.
///
/// `frequency_override`, when given, replaces the problem's frequencies (the
/// CLI's `--freq fmin fmax ndec`).
pub fn run_reporting(
    problem: &Problem,
    frequency_override: Option<Vec<f64>>,
) -> Result<(SweepResult, Vec<String>), String> {
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
    let system = MeshSystem::assemble_with_coupling(
        &problem.geometry,
        &problem.ports,
        &problem.discretization,
        &problem.coupling,
    )
    .map_err(|error| error.to_string())?;
    let warnings = system
        .truncation_warnings()
        .iter()
        .map(ToString::to_string)
        .collect();
    let result = system
        .sweep(&frequencies)
        .map_err(|error| error.to_string())?;
    Ok((result, warnings))
}

#[cfg(test)]
mod tests {
    #[test]
    fn cli_definition_is_valid() {
        use clap::CommandFactory;
        crate::cli::Cli::command().debug_assert();
    }
}
