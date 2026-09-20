//! `fasterhenry` — clean-room PEEC inductance/resistance extraction.
//!
//! Run a sweep with `fasterhenry run <deck.inp | problem.json>`; see
//! `--help`.

use clap::Parser;
use fasterhenry_cli::cli::{Cli, Command};
use fasterhenry_cli::{read_inputs, run, write_zc_text};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Run {
            input,
            freq,
            json,
            zc_mat,
        } => {
            let problem = read_inputs(&input).map_err(|m| anyhow::anyhow!("{m}"))?;
            let override_frequencies = match &freq {
                Some(values) => Some(decade_frequencies(values[0], values[1], values[2])?),
                None => None,
            };
            let result = run(&problem, override_frequencies).map_err(|m| anyhow::anyhow!("{m}"))?;
            let serialized = serde_json::to_string_pretty(&result)?;

            match json {
                Some(path) => std::fs::write(&path, serialized)?,
                None => println!("{serialized}"),
            }
            if let Some(path) = &zc_mat {
                std::fs::write(path, write_zc_text(&result))?;
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
