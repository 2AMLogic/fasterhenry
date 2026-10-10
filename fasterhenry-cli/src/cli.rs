//! The command-line definition (clap). Lives in the library so the tests
//! can validate it and the binary stays a thin wrapper.
//!
//! Two spellings reach the same sweep: the drop-in bare form
//! `fasterhenry <deck.inp>`, for scripts written against FastHenry, and the
//! explicit `fasterhenry run <deck.inp>` of 0.1. They take the same options
//! ([`RunArgs`]) and differ only in their default output — the bare form
//! always writes a [`DEFAULT_ZC_MAT`] file, `run` writes one only when
//! `--zc-mat` asks. [`Invocation`] is the parse of either.
//!
//! These types track the command line and are not a stable library API:
//! they may change (for example, gain a field) in a patch release whenever
//! the CLI gains an option.

use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use fasterhenry::SolverChoice;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// The file a bare invocation writes the impedance sweep to when `--zc-mat`
/// does not name another path: `Zc.mat` in the working directory.
pub const DEFAULT_ZC_MAT: &str = "Zc.mat";

const ABOUT: &str = "Clean-room PEEC inductance/resistance extractor";

/// Printed under the bare form's own `--help`, where `run` is the thing the
/// reader has not been told about yet.
const BARE_AFTER_HELP: &str = "\
`fasterhenry run <INPUT> [OPTIONS]` is the same command with the same options; \
it differs only in writing no Zc.mat unless --zc-mat asks for one.";

/// Printed under `fasterhenry help`, where the bare form is the thing the
/// reader has not been told about yet.
const SUBCOMMAND_AFTER_HELP: &str = "\
With no subcommand, `fasterhenry <INPUT> [OPTIONS]` runs the same sweep and \
writes ./Zc.mat by default, for scripts that call FastHenry today. Run \
`fasterhenry --help` for that form's options.";

/// The 0.1 command line: the explicit `run` subcommand.
#[derive(Parser)]
#[command(
    name = "fasterhenry",
    version,
    long_version = version(),
    about = ABOUT,
    after_help = SUBCOMMAND_AFTER_HELP,
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
    Run(RunArgs),
}

/// The bare, FastHenry-style command line: the input and the options, with
/// no subcommand in front of them.
#[derive(Parser)]
#[command(
    name = "fasterhenry",
    version,
    long_version = version(),
    about = ABOUT,
    after_help = BARE_AFTER_HELP,
    arg_required_else_help = true
)]
pub struct BareCli {
    #[command(flatten)]
    pub args: RunArgs,
}

/// The input and options both forms take.
#[derive(Args, Debug)]
pub struct RunArgs {
    /// Input: `.inp`/`.fh` deck, or a JSON problem document (any other
    /// extension).
    pub input: PathBuf,
    /// Override the deck's sweep: min Hz, max Hz, points per decade
    /// (decade-sampled, log-spaced; FMIN == FMAX runs one frequency).
    #[arg(
        long,
        value_names = ["FMIN_HZ", "FMAX_HZ", "NDEC"],
        num_args = 3,
        allow_hyphen_values = false
    )]
    pub freq: Option<Vec<f64>>,
    /// Write the JSON result to this file instead of stdout.
    #[arg(long, value_name = "OUT_JSON")]
    pub json: Option<PathBuf>,
    /// Write the impedance sweep as a binary MAT v4 `Zc.mat`-format
    /// file: `Zc_1 … Zc_K` (complex, ohms) and `freqs` (Hz). A bare
    /// invocation writes `./Zc.mat` without this flag.
    #[arg(long, value_name = "OUT_MAT")]
    pub zc_mat: Option<PathBuf>,
    /// Write a SPICE subcircuit at one frequency (coupled inductors
    /// for L, H sources for R).
    #[arg(long, value_name = "OUT_CIR")]
    pub spice: Option<PathBuf>,
    /// The frequency for `--spice`, in hertz; default: the last one.
    #[arg(long, value_name = "HZ")]
    pub spice_freq: Option<f64>,
    /// Which solve path to use. `auto` keeps the dense path up to
    /// `fasterhenry::DENSE_PATH_MAX_FILAMENTS` filaments and switches to
    /// the matrix-free precorrected-FFT + GMRES path above it; `dense`
    /// and `iterative` force one. A deck that truncates coupling
    /// (`.couples`) is dense-only.
    #[arg(long, value_enum, default_value_t = SolverArg::Auto)]
    pub solver: SolverArg,
    /// Read a `.inp`/`.fh` deck's first line as an always-ignored title,
    /// as the public FastHenry format does, for third-party decks whose
    /// line 1 is prose. Off by default: line 1 is parsed like any other
    /// and `.title <text>` sets the title. A later `.title` is still
    /// honored in this mode. Also gives a segment or ground plane with no
    /// conductivity FastHenry's copper default (5.8e7 S/m), with a warning,
    /// and turns a `.equiv` of a node with itself into a warning.
    #[arg(long)]
    pub fasthenry_compat: bool,
}

/// Which of the two entry points a command line named.
#[derive(Debug)]
pub enum Invocation {
    /// `fasterhenry <deck>`: the drop-in form, which writes
    /// [`DEFAULT_ZC_MAT`] unless `--zc-mat` names another path.
    Bare(RunArgs),
    /// `fasterhenry run <deck>`: the 0.1 form, where `--zc-mat` is opt-in.
    Run(RunArgs),
}

impl Invocation {
    /// Parses this process's own command line, exiting with clap's usage
    /// message on an unrecognized flag or a missing input — the behaviour of
    /// [`Parser::parse`].
    pub fn parse() -> Self {
        Self::try_parse_from(std::env::args_os()).unwrap_or_else(|error| error.exit())
    }

    /// [`Invocation::parse`] over an explicit argument list (program name
    /// first), returning clap's error instead of exiting the process.
    ///
    /// The first argument decides which form this is: a subcommand name of
    /// [`Cli`] (`run`, `help`) selects the 0.1 command line, anything else —
    /// a path, an option, or nothing at all — the bare one. So an option may
    /// precede the input (`fasterhenry --solver dense deck.inp`), and an
    /// unrecognized one is rejected by name either way. An input file whose
    /// own name is a subcommand name needs the subcommand form
    /// (`fasterhenry run run`) or a qualified path (`./run`).
    pub fn try_parse_from<I, T>(args: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        let args: Vec<OsString> = args.into_iter().map(Into::into).collect();
        match args.get(1) {
            Some(first) if names_a_subcommand(first) => match Cli::try_parse_from(args)?.command {
                Command::Run(args) => Ok(Self::Run(args)),
            },
            _ => Ok(Self::Bare(BareCli::try_parse_from(args)?.args)),
        }
    }

    /// The input and options, whichever form supplied them.
    pub fn args(&self) -> &RunArgs {
        match self {
            Self::Bare(args) | Self::Run(args) => args,
        }
    }

    /// [`Invocation::args`], consuming the invocation. Take
    /// [`Invocation::zc_mat`] first: the default this form implies is not
    /// recoverable from the options alone.
    pub fn into_args(self) -> RunArgs {
        match self {
            Self::Bare(args) | Self::Run(args) => args,
        }
    }

    /// The MAT v4 file this invocation writes, if any: `--zc-mat`'s path, or
    /// [`DEFAULT_ZC_MAT`] for a bare invocation that gave none.
    pub fn zc_mat(&self) -> Option<&Path> {
        match self {
            Self::Bare(args) => Some(
                args.zc_mat
                    .as_deref()
                    .unwrap_or_else(|| Path::new(DEFAULT_ZC_MAT)),
            ),
            Self::Run(args) => args.zc_mat.as_deref(),
        }
    }
}

/// Whether `argument` is the name (or an alias) of one of [`Cli`]'s
/// subcommands, asked of clap itself so the two cannot drift apart.
fn names_a_subcommand(argument: &OsStr) -> bool {
    let mut command = Cli::command();
    // `help` joins the tree only once the command is built.
    command.build();
    let named = command.get_subcommands().any(|subcommand| {
        std::iter::once(subcommand.get_name())
            .chain(subcommand.get_all_aliases())
            .any(|name| argument == OsStr::new(name))
    });
    named
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
