//! The `fasterhenry` command-line front end: FastHenry-format `.inp` decks
//! (the public file format) or JSON problem documents in, JSON out.
//!
//! The binary logic lives in this library so the tests can drive it without
//! spawning a process; `src/main.rs` is a thin wrapper.

#![forbid(unsafe_code)]

pub mod cli;
pub mod inp;
pub mod mat;
pub mod outputs;
pub mod problem;
pub mod spice;

pub use problem::Problem;

use std::path::Path;

use fasterhenry::{
    CurrentSolution, IterativeParams, IterativeSystem, MeshSystem, Solver, SolverChoice,
    SweepResult,
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
    let (frequencies, system, warnings) = prepared(problem, frequency_override, choice)?;
    let result = system.sweep(&frequencies)?;
    Ok((result, warnings))
}

/// The assembled system of whichever solve path was chosen.
enum System {
    Dense(Box<MeshSystem>),
    Iterative(Box<IterativeSystem>),
}

impl System {
    /// Coupling-truncation warnings; always empty for the iterative path.
    fn warnings(&self) -> Vec<String> {
        match self {
            Self::Dense(system) => system
                .truncation_warnings()
                .iter()
                .map(ToString::to_string)
                .collect(),
            Self::Iterative(_) => Vec::new(),
        }
    }

    fn sweep(&self, frequencies: &[f64]) -> Result<SweepResult, String> {
        match self {
            Self::Dense(system) => system.sweep(frequencies),
            Self::Iterative(system) => system.sweep(frequencies),
        }
        .map_err(|error| error.to_string())
    }

    /// Currents for 1 A into port 1, the other ports open.
    fn unit_currents(&self, frequency: f64, ports: usize) -> Result<CurrentSolution, String> {
        let excitation = fasterhenry::unit_excitation(ports);
        match self {
            Self::Dense(system) => system.currents(frequency, &excitation),
            Self::Iterative(system) => system.currents(frequency, &excitation),
        }
        .map_err(|error| error.to_string())
    }
}

fn assemble_system(problem: &Problem, choice: SolverChoice) -> Result<System, String> {
    if solver_for(problem, choice)? == Solver::Iterative {
        IterativeSystem::assemble(
            &problem.geometry,
            &problem.ports,
            &problem.discretization,
            &IterativeParams::default(),
        )
        .map(|system| System::Iterative(Box::new(system)))
    } else {
        MeshSystem::assemble_with_coupling(
            &problem.geometry,
            &problem.ports,
            &problem.discretization,
            &problem.coupling,
        )
        .map(|system| System::Dense(Box::new(system)))
    }
    .map_err(|error| error.to_string())
}

/// The frequency list, assembled system and truncation warnings that every
/// solve entry point needs, behind the two input guards every one of them
/// owes the user.
///
/// `frequency_override`, when given, replaces the problem's frequencies (the
/// CLI's `--freq fmin fmax ndec`). An empty frequency list or an empty port
/// list is reported here, by name, instead of surfacing as an obscure
/// failure out of the assembly or an empty sweep.
///
/// [`run_reporting_with`] and [`run_with_currents`] both start here, so
/// `fasterhenry run` and `fasterhenry run --currents` cannot disagree about
/// which problems they accept or what they say when they refuse one — the
/// same reason [`mod@fasterhenry::solve`] holds the sweep itself in one place
/// for both solver paths (issue #368).
fn prepared(
    problem: &Problem,
    frequency_override: Option<Vec<f64>>,
    choice: SolverChoice,
) -> Result<(Vec<f64>, System, Vec<String>), String> {
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
    let system = assemble_system(problem, choice)?;
    let warnings = system.warnings();
    Ok((frequencies, system, warnings))
}

/// The header row of the `--currents` CSV.
pub const CURRENTS_CSV_HEADER: &str = "frequency_hz,filament,segment,conductor,x_m,y_m,z_m,\
current_re_a,current_im_a,current_abs_a,j_abs_a_per_m2,power_w";

/// [`run_reporting_with`] that also computes the filament currents at every
/// frequency (excitation: 1 A into port 1, the other ports open) and streams
/// them to `csv`, one row per filament per frequency, holding only one
/// frequency's rows in memory at a time. Returns the sweep, the truncation
/// warnings and the `currents` JSON block (per-frequency power balance,
/// per-segment sums and per-conductor peak `|J|`; no per-filament table).
/// Columns, units and conventions: `docs/output-formats.md`.
pub fn run_with_currents(
    problem: &Problem,
    frequency_override: Option<Vec<f64>>,
    choice: SolverChoice,
    csv: &mut dyn std::io::Write,
) -> Result<(SweepResult, Vec<String>, serde_json::Value), String> {
    let (frequencies, system, warnings) = prepared(problem, frequency_override, choice)?;
    let result = system.sweep(&frequencies)?;
    let io = |error: std::io::Error| format!("cannot write currents CSV: {error}");
    writeln!(csv, "{CURRENTS_CSV_HEADER}").map_err(io)?;
    let mut blocks = Vec::with_capacity(frequencies.len());
    for &frequency in &frequencies {
        let solution = system.unit_currents(frequency, problem.ports.len())?;
        for f in &solution.filaments {
            writeln!(
                csv,
                "{:e},{},{},{},{:e},{:e},{:e},{:e},{:e},{:e},{:e},{:e}",
                frequency,
                f.filament,
                f.segment,
                f.conductor,
                f.location[0],
                f.location[1],
                f.location[2],
                f.current.re,
                f.current.im,
                f.current.norm(),
                f.current_density,
                f.power_w
            )
            .map_err(io)?;
        }
        blocks.push(serde_json::json!({
            "frequency_hz": frequency,
            "power_balance": solution.power_balance,
            "conductors": solution.conductors,
            "segments": solution.segments,
        }));
    }
    csv.flush().map_err(io)?;
    let block = serde_json::json!({
        "excitation": "1 A (peak phasor) into port 1, other ports open",
        "power_convention": "time-average, peak phasors: P = 0.5*|I|^2*R",
        "by_frequency": blocks,
    });
    Ok((result, warnings, block))
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
