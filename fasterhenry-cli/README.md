# fasterhenry-cli

Command-line front end for [`fasterhenry`](https://crates.io/crates/fasterhenry),
a clean-room, MIT-licensed Rust engine for **PEEC** (partial-element equivalent
circuit) extraction of resistance and inductance of 3-D conductor geometries.

This crate installs a binary named **`fasterhenry`** (the crate is
`fasterhenry-cli`; the library crate owns the `fasterhenry` package name).

> **Status: 0.0.x, pre-release.** The command-line interface is unstable and
> will change without notice between patch versions. The engine behind it is
> the dense M0 core.

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
  <INPUT>  Input: `.inp`/`.fh` deck, or a JSON problem document (any other extension)

Options:
      --freq <FMIN_HZ> <FMAX_HZ> <NDEC>
          Override the deck's sweep: min Hz, max Hz, points per decade (decade-sampled, log-spaced; FMIN == FMAX runs one frequency)
      --json <OUT_JSON>
          Write the JSON result to this file instead of stdout
      --zc-mat <OUT_MAT>
          Also write a plain-text Z matrix (one block per frequency)
  -h, --help
          Print help
```

`--version` prints the crate version and the source revision stamped at
build time (`fasterhenry 0.0.1 (git: <git describe>)`, or `(git: unknown)`
when built outside a repository, e.g. from a crates.io tarball).

The deck reader covers the M0 subset of the public FastHenry `.inp` format
(`.units`, `.default`, `N`, `E`, `.external`, `.freq`, `.equiv`, `.end`),
with clear line-numbered errors for anything outside it — see
`src/inp.rs` for the exact syntax and semantics. The JSON problem document
is the library's own (validated) types; `src/problem.rs` documents it.
Results are written as JSON with provenance (version, counts, timing);
`--zc-mat` additionally writes a plain-text impedance matrix in a
documented format of our own — not FastHenry's binary `Zc.mat`.

## License

MIT — see `LICENSE`. Changes are recorded in `CHANGELOG.md`. Source and issues:
<https://github.com/2AMLogic/fasterhenry>.
