//! `fasterhenry` — clean-room PEEC inductance/resistance extraction.
//!
//! Run a sweep with `fasterhenry <deck.inp | problem.json>`, which writes a
//! `Zc.mat` in the working directory the way a FastHenry run does, or with
//! the explicit `fasterhenry run <deck.inp | problem.json>`, which writes
//! one only when `--zc-mat` asks for it. See `--help`.

#![forbid(unsafe_code)]

use fasterhenry_cli::cli::{Invocation, RunArgs};
use fasterhenry_cli::inp::{decade_sweep, ParseOptions};
use fasterhenry_cli::outputs::{validate_destinations, Destination};
use fasterhenry_cli::spice::{subckt_name_from_path, write_spice_subckt};
use fasterhenry_cli::{read_inputs_reporting_with, run_reporting_with, solver_for};

fn main() -> anyhow::Result<()> {
    let invocation = Invocation::parse();
    // The one difference between the two forms: a bare invocation always
    // writes a Zc.mat, `run` only when asked.
    let zc_mat = invocation.zc_mat().map(std::path::Path::to_path_buf);
    let RunArgs {
        input,
        freq,
        zc_mat: zc_mat_flag,
        json,
        spice,
        spice_freq,
        solver,
        fasthenry_compat,
    } = invocation.into_args();

    // Before anything is read, solved or written: no output may be the
    // input, and no two outputs may be one file (issue #175).
    let zc_mat_role = if zc_mat_flag.is_some() {
        "--zc-mat"
    } else {
        "default Zc.mat"
    };
    let destinations: Vec<Destination<'_>> = [
        ("--json", json.as_deref()),
        (zc_mat_role, zc_mat.as_deref()),
        ("--spice", spice.as_deref()),
    ]
    .into_iter()
    .filter_map(|(role, path)| path.map(|path| Destination { role, path }))
    .collect();
    validate_destinations(&input, &destinations).map_err(|m| anyhow::anyhow!("{m}"))?;

    let options = ParseOptions { fasthenry_compat };
    let (problem, parse_warnings) =
        read_inputs_reporting_with(&input, options).map_err(|m| anyhow::anyhow!("{m}"))?;
    // A clause the deck states but that cannot touch the mesh (a hole or
    // contact wholly off its plane) is accepted, and said so on stderr.
    for warning in &parse_warnings {
        eprintln!("warning: {warning}");
    }
    let override_frequencies = match &freq {
        Some(values) => Some(decade_frequencies(values[0], values[1], values[2])?),
        None => None,
    };
    let choice = solver.into();
    // Say so when the run leaves the default dense path: the two
    // paths differ in their approximations, so which one produced
    // the JSON on stdout is worth a line on stderr.
    if solver_for(&problem, choice).map_err(|m| anyhow::anyhow!("{m}"))?
        == fasterhenry::Solver::Iterative
    {
        eprintln!("solver: iterative (matrix-free precorrected-FFT + GMRES)");
    }
    let (result, warnings) = run_reporting_with(&problem, override_frequencies, choice)
        .map_err(|m| anyhow::anyhow!("{m}"))?;
    // Truncation is a physical approximation; say so on stderr, so it
    // never hides inside the JSON on stdout.
    for warning in &warnings {
        eprintln!("warning: {warning}");
    }
    let serialized = serde_json::to_string_pretty(&result)?;

    match json {
        Some(path) => std::fs::write(&path, serialized)?,
        None => println!("{serialized}"),
    }
    if let Some(path) = &zc_mat {
        let file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);
        fasterhenry_cli::mat::write_zc_mat(&mut writer, &result)
            .map_err(|error| anyhow::anyhow!("cannot write {}: {error}", path.display()))?;
        // A small result can sit entirely in the buffer, and dropping a
        // `BufWriter` discards a failed final write; drain it explicitly.
        std::io::Write::flush(&mut writer)
            .map_err(|error| anyhow::anyhow!("cannot write {}: {error}", path.display()))?;
    }
    if let Some(path) = &spice {
        let index = match spice_freq {
            Some(hz) => result
                .frequencies_hz
                .iter()
                .position(|&f| f == hz)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "--spice-freq {hz:e} Hz is not one of the sweep's frequencies ({})",
                        result
                            .frequencies_hz
                            .iter()
                            .map(|f| format!("{f:e}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })?,
            None => result.frequencies_hz.len() - 1,
        };
        let name = subckt_name_from_path(&input);
        std::fs::write(path, write_spice_subckt(&result, index, &name))?;
    }
    Ok(())
}

/// The `--freq` override; the same documented decade sampling as `.freq`
/// (`NDEC` may be fractional, but must be > 0).
fn decade_frequencies(fmin: f64, fmax: f64, ndec: f64) -> anyhow::Result<Vec<f64>> {
    decade_sweep(fmin, fmax, ndec).map_err(|message| anyhow::anyhow!("--freq: {message}"))
}
