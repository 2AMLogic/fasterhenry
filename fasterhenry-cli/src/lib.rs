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

use fasterhenry::{
    IterativeParams, IterativeSystem, MeshSystem, Solver, SolverChoice, SweepResult,
};

/// Reads a problem from a file: `.inp`/`.fh` decks by extension, anything
/// else as a JSON problem document. Decks are read with the default
/// [`inp::ParseOptions`]; see [`read_inputs_with`].
pub fn read_inputs(path: &Path) -> Result<Problem, String> {
    read_inputs_with(path, inp::ParseOptions::default())
}

/// [`read_inputs`], reading a deck under `options` (for example
/// [`inp::ParseOptions::fasthenry_compat`]). JSON problem documents ignore
/// `options`.
///
/// Any deck parse warnings are discarded; see [`read_inputs_reporting_with`]
/// to receive them.
pub fn read_inputs_with(path: &Path, options: inp::ParseOptions) -> Result<Problem, String> {
    read_inputs_reporting_with(path, options).map(|(problem, _)| problem)
}

/// [`read_inputs`], returning the problem together with the deck's parse
/// warnings (see [`read_inputs_reporting_with`]).
pub fn read_inputs_reporting(path: &Path) -> Result<(Problem, Vec<String>), String> {
    read_inputs_reporting_with(path, inp::ParseOptions::default())
}

/// [`read_inputs_with`], returning the problem together with the deck's
/// parse warnings — each an [`inp::ParseWarning`] rendered as `line N:
/// <message>`, in deck order — the way [`run_reporting`] returns the
/// solve's warnings. A JSON problem document has none. Nothing is printed:
/// the binary prints them on stderr.
pub fn read_inputs_reporting_with(
    path: &Path,
    options: inp::ParseOptions,
) -> Result<(Problem, Vec<String>), String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("inp") | Some("fh") => inp::parse_with_options_reporting(&text, options)
            .map(|(deck, warnings)| {
                (
                    Problem::from(deck),
                    warnings.iter().map(ToString::to_string).collect(),
                )
            })
            .map_err(|error| error.to_string()),
        _ => serde_json::from_str(&text)
            .map(|problem| (problem, Vec::new()))
            .map_err(|error| format!("invalid problem document: {error}")),
    }
}

/// Runs the sweep, discarding any coupling-truncation warnings. See
/// [`run_reporting`] to receive them.
pub fn run(problem: &Problem, frequency_override: Option<Vec<f64>>) -> Result<SweepResult, String> {
    run_reporting(problem, frequency_override).map(|(result, _)| result)
}

/// The solve path `choice` takes on `problem`: its filament count against
/// [`fasterhenry::DENSE_PATH_MAX_FILAMENTS`] when the choice is
/// [`SolverChoice::Auto`], the forced path otherwise.
///
/// A [`fasterhenry::Coupling`] that truncates something is a dense-path
/// feature — the matrix-free operator couples every pair — so `Auto` stays
/// dense for such a problem whatever its size, and an explicit
/// [`SolverChoice::Iterative`] is an error rather than a silently different
/// physical approximation.
pub fn solver_for(problem: &Problem, choice: SolverChoice) -> Result<Solver, String> {
    let filaments = problem
        .discretization
        .filament_count(&problem.geometry)
        .map_err(|error| error.to_string())?;
    let solver = choice.resolve(filaments);
    if problem.coupling.is_all_pairs() {
        return Ok(solver);
    }
    match choice {
        SolverChoice::Iterative => Err(
            "--solver iterative cannot honour a .couples truncation: the matrix-free operator \
             couples every pair of filaments. Drop the truncation or use --solver dense."
                .to_string(),
        ),
        // Truncation is the user's own approximation; keep it.
        SolverChoice::Auto | SolverChoice::Dense => Ok(Solver::Dense),
    }
}

/// Runs the sweep and returns it together with the assembly's
/// coupling-truncation warnings — the group pairs whose mutual inductance
/// `.couples` dropped even though they are closer than one extent apart, in
/// the order [`fasterhenry::MeshSystem::truncation_warnings`] reports them.
/// The list is empty unless the problem truncates something.
///
/// `frequency_override`, when given, replaces the problem's frequencies (the
/// CLI's `--freq fmin fmax ndec`).
///
/// The solve path is [`SolverChoice::Auto`]'s; see [`run_reporting_with`] to
/// force one.
pub fn run_reporting(
    problem: &Problem,
    frequency_override: Option<Vec<f64>>,
) -> Result<(SweepResult, Vec<String>), String> {
    run_reporting_with(problem, frequency_override, SolverChoice::Auto)
}

/// [`run_reporting`] on a chosen solve path (the CLI's `--solver`).
///
/// [`SolverChoice::Auto`] keeps the dense path
/// ([`MeshSystem`]) at or below [`fasterhenry::DENSE_PATH_MAX_FILAMENTS`]
/// filaments and takes the matrix-free pFFT + GMRES path
/// ([`IterativeSystem`]) above it — see [`solver_for`]. The iterative path
/// reports no coupling-truncation warnings because it does not truncate:
/// its warning list is always empty.
pub fn run_reporting_with(
    problem: &Problem,
    frequency_override: Option<Vec<f64>>,
    choice: SolverChoice,
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
    if solver_for(problem, choice)? == Solver::Iterative {
        let system = IterativeSystem::assemble(
            &problem.geometry,
            &problem.ports,
            &problem.discretization,
            &IterativeParams::default(),
        )
        .map_err(|error| error.to_string())?;
        let result = system
            .sweep(&frequencies)
            .map_err(|error| error.to_string())?;
        return Ok((result, Vec::new()));
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
        crate::cli::BareCli::command().debug_assert();
    }

    /// The reporting read path returns a deck's parse warnings as `line N:
    /// …` strings alongside the very problem the plain path returns, whose
    /// signatures are unchanged (issue #105).
    #[test]
    fn read_inputs_reporting_returns_parse_warnings() {
        use crate::{read_inputs, read_inputs_reporting, read_inputs_with, Problem};
        use std::path::Path;

        let directory = std::env::temp_dir().join(format!(
            "fasterhenry-cli-lib-{}-reporting",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("deck.inp");
        std::fs::write(
            &path,
            "\
.units mm
.default sigma=5.8e4
Gp x1=0 y1=0 z1=0 x2=10 y2=0 z2=0 x3=10 y3=6 z3=0
+ thick=0.04 seg1=5 seg2=3
+ hole point (50, 3, 0)
Nt x=5 y=3 z=0.5
Nb x=5 y=3 z=0
Ev Nt Nb w=0.2 h=0.2
.external Nt Nb
.freq fmin=1 fmax=1 ndec=1
.end
",
        )
        .unwrap();
        let plain: fn(&Path) -> Result<Problem, String> = read_inputs;
        let with: fn(&Path, crate::inp::ParseOptions) -> Result<Problem, String> = read_inputs_with;
        let (problem, warnings) = read_inputs_reporting(&path).unwrap();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0].starts_with("line 5: 'hole point'"),
            "{warnings:?}"
        );
        assert_eq!(plain(&path).unwrap(), problem);
        assert_eq!(with(&path, Default::default()).unwrap(), problem);
        let _ = std::fs::remove_dir_all(&directory);
    }
}
