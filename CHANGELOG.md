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

- Matrix-free partial-inductance operator by the precorrected FFT
  (issue #42, phase 1 of #24): `fasterhenry::pfft::PfftOperator` evaluates
  `L·x` without assembling `L` — filaments projected onto a uniform grid by
  Lagrange interpolation, the far field convolved with `1/r` by zero-padded
  (and pruned) 3-D FFT, and near pairs precorrected to the exact kernel
  value. Grid spacing, near-field radius and interpolation order are named
  `PfftParams`, with the accuracy-versus-cost trade-off measured and
  documented; the defaults agree with the dense product to about `2e-7`
  relative, and `fasterhenry/tests/pfft.rs` checks `1e-6` on random sets,
  the spiral and coupled-structure fixtures, and flat, line-like and
  single-cell geometries. Memory and time per product grow linearly with
  the filament count (criterion bench `fasterhenry/benches/pfft.rs`,
  1 000–10 000 filaments). The solver is unchanged; iterative solution on
  the operator is issue #43. New dependency: `rustfft` (pure Rust, MIT OR
  Apache-2.0).
- Width-graded skin-effect validation against an independent 2-D reference
  (issue #32): `tools/cross_section_reference.py` solves the finite-width
  cross-section of an isolated rectangular trace (Richardson-extrapolated,
  second-order convergence enforced) and
  `fasterhenry/tests/width_graded_skin_validation.rs` compares uniform,
  width-graded and skin-depth-adaptive grids against it at `t/δ = 0.3 … 10`,
  asserting a refinement-based accuracy bound for `R'` and the internal
  inductance change `ΔL'`. It shows width grading is the accurate grid; a
  width-uniform 36 × 10 grid reads `R'` 28 % low at `t/δ = 10`. Results and
  the supported regime are recorded in `docs/validation.md`; CI regenerates
  the reference and requires every gate to report `PASSED`.
- Committed criterion bench suite: `fasterhenry/benches/assembly.rs` and
  `fasterhenry/benches/solve.rs` measure `MeshSystem::assemble` and
  `MeshSystem::sweep` throughput against a fixed filament-count sweep
  (48–19 600, matching the scale of `docs/benchmarks.md`'s head-to-head
  table), alongside the existing kernel-level `benches/kernels.rs`. A new
  `workflow_dispatch` CI job (`.github/workflows/bench.yml`) runs the
  suite on a pinned `ubuntu-latest` runner and regenerates a dated results
  table in `docs/benchmarks.md` via `tools/bench_table.py`, parsed from
  criterion's own JSON output rather than hand-typed.
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
- Non-transitive `.couples` declarations are rejected (issue #46): when two
  declared groups are reachable only through a third — `.couples a b` plus
  `.couples b c` with no `a`–`c` — `Coupling::resolve` returns the new
  `CouplingError::NonTransitiveCoupling`, naming the undeclared pair. Such a
  relation can make the truncated `L` indefinite, and every previously
  accepted deck of this shape was silently solving an unphysical system;
  there is no regime where the shape is intended (declare the whole clique,
  or use the `.couples a b c` shorthand). Decks with no `.couples` line and
  `.couples all` are unaffected, as are fully declared cliques and isolated
  pairs.

### Changed

- `fasterhenry-cli` now takes the library from `[workspace.dependencies]` with
  both a `path` and a `version`, so the CLI crate is publishable.

[Unreleased]: https://github.com/2AMLogic/fasterhenry/commits/main
