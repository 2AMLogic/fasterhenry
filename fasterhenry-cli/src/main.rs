//! `fasterhenry` — clean-room PEEC inductance/resistance extraction.
//!
//! Run a sweep with `fasterhenry run <deck.inp | problem.json>`; see
//! `--help`.

use clap::Parser;
use fasterhenry_cli::cli::{Cli, Command};
use fasterhenry_cli::spice::write_spice_subckt;
use fasterhenry_cli::{read_inputs, run_reporting};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Run {
            input,
            freq,
            json,
            zc_mat,
            spice,
            spice_freq,
        } => {
            let problem = read_inputs(&input).map_err(|m| anyhow::anyhow!("{m}"))?;
            let override_frequencies = match &freq {
                Some(values) => Some(decade_frequencies(values[0], values[1], values[2])?),
                None => None,
            };
            let (result, warnings) = run_reporting(&problem, override_frequencies)
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
                let name = input
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("fasterhenry")
                    .replace('.', "_");
                std::fs::write(path, write_spice_subckt(&result, index, &name))?;
            }
            Ok(())
        }
    }
}

/// The `--freq` override; same decade sampling as `.freq`.
fn decade_frequencies(fmin: f64, fmax: f64, ndec: f64) -> anyhow::Result<Vec<f64>> {
    if !(fmin.is_finite() && fmin >= 0.0 && fmax.is_finite() && fmax >= 0.0) {
        anyhow::bail!("--freq values must be finite and >= 0");
    }
    if fmax < fmin {
        anyhow::bail!("--freq FMAX_HZ is below FMIN_HZ");
    }
    if fmin == fmax {
        return Ok(vec![fmin]);
    }
    if fmin == 0.0 {
        anyhow::bail!("--freq 0 is only allowed as the single-frequency case");
    }
    if !(ndec >= 1.0 && ndec.fract() == 0.0) {
        anyhow::bail!("--freq NDEC must be a whole number >= 1");
    }
    let ndec = ndec as usize;
    let decades = fmax.log10() - fmin.log10();
    let count = (decades * ndec as f64 + 1.0 + 1e-9).floor() as usize;
    Ok((0..count)
        .map(|k| {
            let (quotient, remainder) = (k / ndec, k % ndec);
            if remainder == 0 {
                fmin * 10f64.powi(quotient as i32)
            } else {
                fmin * 10f64.powf(k as f64 / ndec as f64)
            }
        })
        .collect())
}
