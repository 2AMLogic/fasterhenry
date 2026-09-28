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

One deck, two spellings of the same command:

```bash
fasterhenry deck.inp            # writes ./Zc.mat, as a FastHenry run does
fasterhenry run deck.inp        # writes a Zc.mat only when --zc-mat asks
```

Both take the same options and produce the same JSON on stdout; they differ
only in that default output (see
[Migrating from FastHenry](#migrating-from-fasthenry)).

```text
$ fasterhenry --help
Clean-room PEEC inductance/resistance extractor

Usage: fasterhenry [OPTIONS] <INPUT>

Arguments:
  <INPUT>
          Input: `.inp`/`.fh` deck, or a JSON problem document (any other extension)

Options:
      --freq <FMIN_HZ> <FMAX_HZ> <NDEC>
          Override the deck's sweep: min Hz, max Hz, points per decade (decade-sampled, log-spaced; FMIN == FMAX runs one frequency)

      --json <OUT_JSON>
          Write the JSON result to this file instead of stdout

      --zc-mat <OUT_MAT>
          Write the impedance sweep as a binary MAT v4 `Zc.mat`-format file: `Zc_1 … Zc_K` (complex, ohms) and `freqs` (Hz). A bare invocation writes `./Zc.mat` without this flag

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

      --fasthenry-compat
          Read a `.inp`/`.fh` deck's first line as an always-ignored title, as the public FastHenry format does, for third-party decks whose line 1 is prose. Off by default: line 1 is parsed like any other and `.title <text>` sets the title. A later `.title` is still honored in this mode

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

`fasterhenry run <INPUT> [OPTIONS]` is the same command with the same options; it differs only in writing no Zc.mat unless --zc-mat asks for one.
```

`fasterhenry run --help` prints the same option list under
`Usage: fasterhenry run [OPTIONS] <INPUT>`, and `fasterhenry help` lists the
subcommands.

## Migrating from FastHenry

A script that calls FastHenry today usually names the deck and nothing else,
then reads the `Zc.mat` the run left in the working directory. `fasterhenry`
answers that shape directly:

```bash
fasterhenry deck.inp        # ./Zc.mat is written, whether or not you ask
```

- **The input** is the deck's path — `.inp` or `.fh` by extension, or a JSON
  problem document (see below). It may be given before or after the options
  (`fasterhenry --solver dense deck.inp` works).
- **`./Zc.mat`** is written by this bare form even without `--zc-mat`: same
  MAT level-4 layout FastHenry writes (`Zc_1 … Zc_K`, complex ohms, plus
  `freqs`), readable by `scipy.io.loadmat` and MATLAB/Octave. Pass
  `--zc-mat <path>` to put it somewhere else — the flag replaces the default,
  it does not add a second file.
- **The JSON result** still goes to stdout (`--json <path>` to a file
  instead), so a migrating script may ignore it or start using it.
- **`fasterhenry run <deck>`** is the same command with the same options and
  the 0.1 behaviour: it writes a `Zc.mat` only when `--zc-mat` asks for one.
  Existing invocations keep working unchanged.

What is *not* claimed: FastHenry's own command-line options are not
reimplemented, and this project does not consult that program's source or
manuals (see [`CONTRIBUTING.md`](https://github.com/2AMLogic/fasterhenry/blob/main/CONTRIBUTING.md)).
The policy is instead that nothing is silently ignored — any option or extra
argument this CLI does not define is an error naming it, so a flag your script
passes today is reported rather than quietly dropped:

```text
$ fasterhenry deck.inp -S 10
error: unexpected argument '-S' found
```

Translate such a flag into the equivalent option above (or into a deck
directive) rather than expecting it to be honored. One naming corner: the
first argument is read as a subcommand name when it is exactly one
(`run`, `help`), so a deck literally named `run` needs `fasterhenry run run`
or a qualified path such as `./run`.

`--solver auto` (the default) is a size threshold, not a limit: the dense
path up to 10 000 filaments, GMRES on the precorrected-FFT operator above
it, with the crossover measured and justified in
[`docs/benchmarks.md`](https://github.com/2AMLogic/fasterhenry/blob/main/docs/benchmarks.md).
A run that leaves the dense path says so on stderr, so which path produced
the JSON on stdout is never ambiguous.

`--version` prints the crate version and the source revision stamped at
build time (`fasterhenry <version> (git: <git describe>)`, or `(git: unknown)`
when built outside a repository, e.g. from a crates.io tarball).

The deck reader covers a subset of the public FastHenry `.inp` format:
`.units`, `.default`, `N` nodes, `E` segments (with `nwinc`/`nhinc`
filament counts, `rw`/`rh` filament grading ratios toward the surfaces, and
a `wx`/`wy`/`wz` width direction), `.external` ports, `.freq`, `.equiv`, `G`
ground planes with holes and contact refinement, `.couples` coupling
truncation, and
`.end`. Anything outside it is rejected with a line-numbered error rather
than guessed at; `src/inp.rs` documents the exact syntax and semantics
(note that `.units` is mandatory — one of `km`, `m`, `cm`, `mm`, `um`, `in`
or `mils` — and conductivity is given as `sigma` or resistivity as `rho`,
both per deck unit, but not both on one line).
[`docs/fasthenry-compat.md`](https://github.com/2AMLogic/fasterhenry/blob/main/docs/fasthenry-compat.md)
is the field-by-field compatibility table against the public format
description: every directive, supported / differs / deferred, with the
reason. The JSON problem document is the library's own (validated) types;
`src/problem.rs` documents it.

### Ground planes: two `G` grammars

A ground plane may be declared either in FastHenry's corner-point syntax or
in this crate's own shorthand, and one deck may mix them freely — the two
are told apart by the shape of the first token after the plane's name, not
by a deck-wide mode. Both build the same plane, so `.hole`, `.contact`,
`.equiv` and endpoint landing behave identically whichever form declared
it.

```text
* FastHenry corner-point form: three corners, thickness, cells per edge,
* with in-plane nodes, holes and contacts declared on the statement.
Gplane x1=0 y1=0 z1=0 x2=10 y2=0 z2=0 x3=10 y3=6 z3=0
+ thick=0.035 seg1=5 seg2=3 sigma=5.8e4
+ hole rect (0.5, 4.5, 0, 1.5, 5.5, 0)
+ contact rect (4, 2, 0, 6, 4, 0)
+ Nland1 (1, 1, 0)

* The same plane in this crate's shorthand — which names the plane's TOP
* surface where the corner-point form names its mid-thickness surface.
Gplane 0 0 0.0175 10 6 0.0175 0.035 nx=5 ny=3
.hole Gplane 0.5 4.5 1.5 5.5
.contact Gplane 4 2 6 4
```

In-plane nodes (`N<name> (x, y, z)`) are ordinary deck nodes that belong to
their plane: reference one from a segment or `.external`, or join it to a
segment's node with `.equiv`, and the connection lands on the nearest cell
of *that* plane whichever side `.equiv` named first.

`seg1`/`seg2` become the background cell counts of this engine's own
cell-centre PEEC mesh (the `nx`/`ny` of the shorthand form), so equal
counts mean equal resolution rather than an identical node set. Both forms
take the plane's conductivity as either `sigma=` or its reciprocal `rho=` on
the statement itself, alongside `nhinc=` — naming both on one statement,
continuation lines included, is a line-numbered error, and naming neither
falls back to the `.default` conductivity. Every other documented plane
parameter is either mapped or **rejected by name** with the statement's line
number — `rh`, `segwid1`/`segwid2`, `relx`/`rely`/`relz`, `file`, and every
hole or contact shape other than
`rect`. Nothing on a `G` statement is silently ignored.
`tests/data/plane_fasthenry.inp` and `tests/data/plane_extension.inp` are
the same problem in the two syntaxes, and a test requires them to produce
the same `Z(ω)`.

Results are written as JSON with provenance (version, counts, timing).
Two further outputs are optional:

- `--zc-mat out.mat` — the sweep as a MATLAB level-4 binary file in the
  layout of FastHenry's `Zc.mat` (`Zc_1 … Zc_K`, complex ohms, plus
  `freqs`), readable by `scipy.io.loadmat` and MATLAB/Octave. A bare
  invocation writes `./Zc.mat` without being asked; under `run` the flag is
  the only way to get one.
- `--spice out.cir [--spice-freq HZ]` — a SPICE subcircuit at one
  frequency (default: the last): coupled inductors for `L`, current-
  controlled sources for `R`.

Both formats are specified in
[`docs/output-formats.md`](https://github.com/2AMLogic/fasterhenry/blob/main/docs/output-formats.md).

## License

MIT — see `LICENSE`. Changes are recorded in `CHANGELOG.md`. Source and issues:
<https://github.com/2AMLogic/fasterhenry>.
