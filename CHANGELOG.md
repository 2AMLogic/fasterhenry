# Changelog

All notable changes to the `fasterhenry` library and the `fasterhenry-cli`
binary crate are recorded here. Both crates share one version number and this
one file (each crate ships a copy in its published package).

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
While the version is `0.0.x`, the API and the command-line interface are
unstable and may change in any release.

## [Unreleased]

### Added

- Coupling truncation (`.couples`): tag segments with `group=<name>` and
  declare which groups are magnetically coupled; the mutual inductance of
  every other pair is truncated to zero and — the point of the knob — never
  evaluated. `fasterhenry::coupling` is the library API (`Coupling`,
  `MeshSystem::assemble_with_coupling`, `PairMask`), and documents when the
  approximation is safe: closed loops more than about ten of their own
  extents apart cost under a part in a thousand, since the leading term falls
  as `(a/d)³`. Truncated groups closer than one extent are reported on
  stderr. A deck with no `.couples` line, and `.couples all`, keep today's
  all-pairs assembly bit for bit.
- Ground planes (`G` directive with `.hole`): thick rectangular sheets
  discretized into a cell-centre bar mesh on the same kernels/mesh/solve
  machinery; segment and port endpoints landing in a plane's footprint
  snap to the nearest live cell node (assembly order-independent,
  `.equiv` preserved). Deck syntax, library `fasterhenry::plane` API,
  and validation: trace-over-plane DC/grid checks plus the PyPEEC
  slot-differential gate (4.97 % measured, 5.5 % bound; CI regenerates
  the references and asserts the test did not skip).
- Surface-graded filament grids and skin-depth-sized subdivision for the Rust
  API. The existing uniform grid and `.inp` `nwinc`/`nhinc` path are unchanged.
  A wide-trace validation reaches 1.2 % resistance error against the
  one-dimensional skin-effect asymptote; see `docs/validation.md`.

- Output formats for tool interop: `--zc-mat` writes a binary MAT v4
  `Zc.mat`-format file (`Zc_1 … Zc_K` complex matrices + `freqs`; exact
  round-trip asserted against `scipy.io.loadmat` in CI), and `--spice
  [--spice-freq HZ]` writes a SPICE subcircuit stamping `Z = R + jωL`
  (coupled inductors for L, H sources for R; ngspice-verified to
  reproduce Z to six digits). See `docs/output-formats.md`. The previous
  plain-text `--zc-mat` experiment is superseded (0.0.x-unstable CLI).
- Validation harness (M0's "validated" gate): the 2-turn square spiral
  fixture checked against PyPEEC 5.8.0 (2 % tolerance; `tools/pypeec_reference.py`
  regenerates the reference, CI regenerates it on the Linux legs and
  asserts the test did not skip), Greenhouse segment summation (5 %), and
  the analytic DC resistance; Mohan modified-Wheeler reported for the
  record. Measured: 0.47 % vs PyPEEC, 0.12 % vs Greenhouse. See
  `docs/validation.md`.
- `fasterhenry run <deck.inp | problem.json> [--freq FMIN FMAX NDEC]
  [--json OUT] [--zc-mat OUT]`: the CLI's first real subcommand. Reads an
  M0-subset FastHenry `.inp` deck (public format; clean-room parser with
  line-numbered errors) or a JSON problem document, runs the sweep, and
  writes the JSON result (with provenance) and optionally a plain-text
  impedance matrix. `--version` prints the crate version plus a
  build-time `git describe`.
- Round-trip guarantee: a self-authored two-turn spiral as an `.inp` deck
  and as the equivalent JSON document produce bit-identical sweeps
  (`fasterhenry-cli` integration test).
- `--freq` overrides the deck's sweep with the same decade sampling `.freq`
  uses (`fmin == fmax` runs one frequency; `0` runs the DC solve).

- Cargo workspace with the `fasterhenry` library crate and the
  `fasterhenry-cli` crate, which installs the `fasterhenry` binary.
- Packaging metadata for both crates (`readme`, `documentation`, `keywords`,
  `categories`), per-crate READMEs, and an `include` allowlist so a published
  package carries only sources, the manifest, README, LICENSE, and this
  changelog.
- CI packaging job: `cargo publish --dry-run` for the library and a
  package-contents check for both crates.

### Changed

- `fasterhenry-cli` now takes the library from `[workspace.dependencies]` with
  both a `path` and a `version`, so the CLI crate is publishable.

[Unreleased]: https://github.com/2AMLogic/fasterhenry/commits/main
