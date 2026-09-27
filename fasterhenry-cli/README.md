# fasterhenry-cli

Command-line front end for [`fasterhenry`](https://crates.io/crates/fasterhenry),
a clean-room, MIT-licensed Rust engine for **PEEC** (partial-element equivalent
circuit) extraction of resistance and inductance of 3-D conductor geometries.

This crate installs a binary named **`fasterhenry`** (the crate is
`fasterhenry-cli`; the library crate owns the `fasterhenry` package name).

> **Status: 0.1, early release.** Following semver for `0.x`, a minor release
> (`0.2`, `0.3`, …) may change the command-line interface; patch releases
> stay compatible.

## Install

```bash
cargo install fasterhenry-cli
```

## Usage

```text
$ fasterhenry --help
Clean-room PEEC inductance/resistance extractor

Usage: fasterhenry <COMMAND>

Commands:
  run   Run a frequency sweep on a FastHenry .inp deck or a JSON problem document.
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

```text
$ fasterhenry run --help
Run a frequency sweep on a FastHenry .inp deck or a JSON problem document.

Usage: fasterhenry run [OPTIONS] <INPUT>

Arguments:
  <INPUT>
          Input: `.inp`/`.fh` deck, or a JSON problem document (any other extension)

Options:
      --freq <FMIN_HZ> <FMAX_HZ> <NDEC>
          Override the deck's sweep: min Hz, max Hz, points per decade (decade-sampled, log-spaced; FMIN == FMAX runs one frequency)

      --json <OUT_JSON>
          Write the JSON result to this file instead of stdout

      --zc-mat <OUT_MAT>
          Write the impedance sweep as a binary MAT v4 `Zc.mat`-format file: `Zc_1 … Zc_K` (complex, ohms) and `freqs` (Hz)

      --spice <OUT_CIR>
          Write a SPICE subcircuit at one frequency (coupled inductors for L, H sources for R)

      --spice-freq <HZ>
          The frequency for `--spice`, in hertz; default: the last one

      --solver <SOLVER>
          Which solve path to use. `auto` keeps the dense path up to `fasterhenry::DENSE_PATH_MAX_FILAMENTS` filaments and switches to the matrix-free precorrected-FFT + GMRES path above it; `dense` and `iterative` force one. A deck that truncates coupling (`.couples`) is dense-only

          Possible values:
          - auto:      Dense below the built-in filament threshold, matrix-free above it
          - dense:     Always the dense path
          - iterative: Always the matrix-free pFFT + GMRES path

          [default: auto]

  -h, --help
          Print help (see a summary with '-h')
```

`--solver auto` (the default) is a size threshold, not a limit: the dense
path up to 10 000 filaments, GMRES on the precorrected-FFT operator above
it, with the crossover measured and justified in
[`docs/benchmarks.md`](https://github.com/2AMLogic/fasterhenry/blob/main/docs/benchmarks.md).
A run that leaves the dense path says so on stderr, so which path produced
the JSON on stdout is never ambiguous.

`--version` prints the crate version and the source revision stamped at
build time (`fasterhenry 0.1.0 (git: <git describe>)`, or `(git: unknown)`
when built outside a repository, e.g. from a crates.io tarball).

The deck reader covers a subset of the public FastHenry `.inp` format:
`.units`, `.default`, `N` nodes, `E` segments (with `nwinc`/`nhinc`
filament counts), `.external` ports, `.freq`, `.equiv`, `G` ground planes
with `.hole` and `.contact` refinement, `.couples` coupling truncation, and
`.end`. Anything outside it is rejected with a line-numbered error rather
than guessed at; `src/inp.rs` documents the exact syntax and semantics
(note that `.units` is mandatory and conductivity is given as `sigma`, not
`rho`). The JSON problem document is the library's own (validated) types;
`src/problem.rs` documents it.

Results are written as JSON with provenance (version, counts, timing).
Two further outputs are optional:

- `--zc-mat out.mat` — the sweep as a MATLAB level-4 binary file in the
  layout of FastHenry's `Zc.mat` (`Zc_1 … Zc_K`, complex ohms, plus
  `freqs`), readable by `scipy.io.loadmat` and MATLAB/Octave.
- `--spice out.cir [--spice-freq HZ]` — a SPICE subcircuit at one
  frequency (default: the last): coupled inductors for `L`, current-
  controlled sources for `R`.

Both formats are specified in
[`docs/output-formats.md`](https://github.com/2AMLogic/fasterhenry/blob/main/docs/output-formats.md).

## License

MIT — see `LICENSE`. Changes are recorded in `CHANGELOG.md`. Source and issues:
<https://github.com/2AMLogic/fasterhenry>.
