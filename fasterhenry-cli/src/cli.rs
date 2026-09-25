//! The command-line definition (clap). Lives in the library so the tests
//! can validate it and the binary stays a thin wrapper.

use clap::{Parser, Subcommand, ValueEnum};
use fasterhenry::SolverChoice;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "fasterhenry",
    version,
    long_version = version(),
    about = "Clean-room PEEC inductance/resistance extractor",
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run a frequency sweep on a FastHenry .inp deck or a JSON problem document.
    #[command(verbatim_doc_comment)]
    Run {
        /// Input: `.inp`/`.fh` deck, or a JSON problem document (any other
        /// extension).
        input: PathBuf,
        /// Override the deck's sweep: min Hz, max Hz, points per decade
        /// (decade-sampled, log-spaced; FMIN == FMAX runs one frequency).
        #[arg(
            long,
            value_names = ["FMIN_HZ", "FMAX_HZ", "NDEC"],
            num_args = 3,
            allow_hyphen_values = false
        )]
        freq: Option<Vec<f64>>,
        /// Write the JSON result to this file instead of stdout.
        #[arg(long, value_name = "OUT_JSON")]
        json: Option<PathBuf>,
        /// Write the impedance sweep as a binary MAT v4 `Zc.mat`-format
        /// file: `Zc_1 … Zc_K` (complex, ohms) and `freqs` (Hz).
        #[arg(long, value_name = "OUT_MAT")]
        zc_mat: Option<PathBuf>,
        /// Write a SPICE subcircuit at one frequency (coupled inductors
        /// for L, H sources for R).
        #[arg(long, value_name = "OUT_CIR")]
        spice: Option<PathBuf>,
        /// The frequency for `--spice`, in hertz; default: the last one.
        #[arg(long, value_name = "HZ")]
        spice_freq: Option<f64>,
        /// Which solve path to use. `auto` keeps the dense path up to
        /// `fasterhenry::DENSE_PATH_MAX_FILAMENTS` filaments and switches to
        /// the matrix-free precorrected-FFT + GMRES path above it; `dense`
        /// and `iterative` force one. A deck that truncates coupling
        /// (`.couples`) is dense-only.
        #[arg(long, value_enum, default_value_t = SolverArg::Auto)]
        solver: SolverArg,
    },
}

/// The `--solver` values, mapped onto [`SolverChoice`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum SolverArg {
    /// Dense below the built-in filament threshold, matrix-free above it.
    #[default]
    Auto,
    /// Always the dense path.
    Dense,
    /// Always the matrix-free pFFT + GMRES path.
    Iterative,
}

impl From<SolverArg> for SolverChoice {
    fn from(arg: SolverArg) -> Self {
        match arg {
            SolverArg::Auto => Self::Auto,
            SolverArg::Dense => Self::Dense,
            SolverArg::Iterative => Self::Iterative,
        }
    }
}

/// The `--version` string: crate version plus the source revision stamped
/// by `build.rs` (`git describe`, or `unknown` without a repository).
/// Leaked once: clap's version wants a `&'static str`.
pub fn version() -> &'static str {
    Box::leak(
        format!(
            "{} (git: {})",
            env!("CARGO_PKG_VERSION"),
            option_env!("FASTERHENRY_GIT_DESCRIBE").unwrap_or("unknown")
        )
        .into_boxed_str(),
    )
}
