# Changelog

All notable changes to the `fasterhenry` library and the `fasterhenry-cli`
binary crate are recorded here. Both crates share one version number and this
one file (each crate ships a copy in its published package).

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
While the version is `0.x`, a minor release (`0.1` → `0.2`) may make
breaking changes to the API or the command-line interface; patch releases
(`0.1.0` → `0.1.1`) stay compatible.

## [Unreleased]

### Added

- Deck reader: FastHenry's ground-plane (`G`) statement syntax (issue #69,
  phase 1 of the 0.2.0 deck-compatibility epic). A plane may now be
  declared in the documented corner-point form — three corner points,
  `thick=`, `seg1=`/`seg2=`, `sigma=`, `nhinc=`, with in-plane nodes
  (`N<name> (x, y, z)`), `hole rect (…)` and `contact rect (…)` clauses on
  the statement itself — as well as in this project's existing
  `G<name> x1 y1 z1 x2 y2 z2 t` shorthand with `.hole` / `.contact`. The
  two grammars are told apart by the shape of the first token after the
  plane name, never by a deck-wide mode, so one deck may mix them and
  `.hole`, `.contact`, `.equiv` and endpoint landing behave identically
  whichever form declared the plane. The corner-point form names the
  plane's *mid-thickness* surface where the shorthand names its top, and
  `seg1`/`seg2` become the background cell counts of this engine's own
  cell-centre PEEC mesh. `.equiv` now joins any number of nodes, and an
  in-plane node makes its whole joined set land on that plane whichever
  side of the directive named it first — so wiring a via into a plane does
  not depend on argument order. Joining in-plane nodes of two *different*
  planes is rejected rather than silently attaching to one.
  `nhinc=` is also accepted on the shorthand form.
  Every remaining documented plane parameter is rejected by name, with the
  statement's line number and the alternative: `rh`, `segwid1`/`segwid2`,
  `relx`/`rely`/`relz`, `file`, and every hole or contact shape other than `rect` (`point`, `circle`,
  `decay_rect`, `trace`, the `initial_*`/`equiv_*` forms and the
  user-defined `user1…user7`) — nothing on a `G` statement is silently
  ignored, and representing those shapes is tracked in #80.
  `fasterhenry-cli/tests/data/plane_fasthenry.inp` and
  `plane_extension.inp` are the same self-authored plane problem in the two
  syntaxes, and a new test requires them to produce the same geometry and
  the same `Z(ω)`.
- Deck reader: `rho=` (resistivity, per deck unit) is accepted on
  `.default`, `E` segments and extension-form `G` ground planes as the
  reciprocal of `sigma=` (issue #70); `rho` must be positive, and giving
  both `sigma=` and `rho=` on one line is a line-numbered error. The
  extension form `G<name> x1 y1 z1 x2 y2 z2 t` now also takes a per-plane
  `sigma=` overriding `.default`, alongside its `nx=`/`ny=`/`nhinc=`.
- Deck reader: `rho=` is accepted directly on the corner-point `G`
  ground-plane statement too — the syntax real FastHenry decks use — so the
  whole deck format now takes the conductivity either way wherever it takes
  it at all (issue #88, follow-up to #70). It converts to a conductivity
  exactly as elsewhere (`1/rho` per deck unit), so a corner-point plane with
  `rho=0.5` parses to the same deck as one with `sigma=2`; naming both
  `sigma=` and `rho=` on one corner-point statement — continuation lines
  included — is the same line-numbered error as everywhere else, and naming
  neither still falls back to the `.default` conductivity.
- Deck reader: `.units` accepts the full documented list — `km`, `m`, `cm`,
  `mm`, `um`, `in`, `mils` (`mil` kept as a synonym) — case-insensitively.
- `docs/fasthenry-compat.md`: a field-by-field compatibility table auditing
  the deck reader against the public FastHenry `.inp` input-format
  description — every directive, supported / differs / deferred, with the
  reason (issue #72). Linked from `fasterhenry-cli/README.md`. Tests now
  pin the multi-node `.equiv a b c …` join and its self-alias rejection.
- Deck reader: segment filament ratios and width direction (issue #71).
  `E` lines and `.default` now accept `rw`/`rh` — the ratio of adjacent
  filament extents across the width and the height, each axis
  independently, coarsening from the surfaces inward (`nwinc=5 rw=2` cuts
  the width 1:2:4:2:1) — and `wx`/`wy`/`wz`, a vector along the
  cross-section's width that orients the segment (a flat bar turned on
  edge). An omitted ratio keeps the uniform grid, so existing decks solve
  exactly as before. A ratio that is not a number ≥ 1, a zero width
  direction, and one parallel to the segment are errors on their own line;
  a segment the geometry rejects is now reported on its `E` line instead
  of line 0.
- Library: `discretize_graded_per_axis`, the surface-graded grid with an
  independent ratio along each cross-section axis (`discretize_graded` is
  its equal-ratio case, unchanged), and `Discretization::PerSegmentGraded`
  with its per-segment grid `AxisGrading { nw, nh, width_ratio,
  height_ratio }` — the per-segment counterpart of `Discretization::Graded`.

### Changed

- Deck reader: `rho=` is no longer rejected with a "use sigma = 1/rho" hint
  on `.default`, `E` and extension-form `G` lines.
- No behavior change to existing valid decks from the #72 audit: the
  `.title` directive, mandatory `.units`, and `.end` semantics (required,
  rejects trailing content) are unchanged — the audit confirmed each as an
  intentional, documented difference from the public format rather than a
  bug, and recorded the reasoning in `docs/fasthenry-compat.md` (issue #72).
- Library (breaking): `Discretization` gained the `PerSegmentGraded`
  variant (issue #71). Code that matches `Discretization` exhaustively
  without a `_` arm no longer compiles, so the next release must be a minor
  bump (0.2.0).

## [0.1.1] - 2026-09-27

### Added

- Tag-triggered release workflow (`.github/workflows/release.yml`):
  pushing `vX.Y.Z` publishes both crates to crates.io via Trusted
  Publishing (GitHub OIDC, no stored registry token), after checking the
  tag matches the workspace version and is on `main`.
- Dense/iterative size threshold, and the scaling measurement behind it
  (issue #44, phase 3 of #24). `fasterhenry::DENSE_PATH_MAX_FILAMENTS`
  (10 000) is the filament count at or below which the new
  `SolverChoice::Auto` — the default for `fasterhenry_cli::run_reporting`
  and the CLI's new `--solver auto` — keeps the dense `MeshSystem`;
  above it it selects the matrix-free `IterativeSystem`.
  `SolverChoice::Dense` / `SolverChoice::Iterative` (`--solver dense` /
  `--solver iterative`) force either path at any size, `Solver` names the
  resolved path, and `Discretization::filament_count` sizes a problem
  without assembling it so a front end can choose before paying for
  assembly. A deck that truncates coupling (`.couples`) stays dense at any
  size; `--solver iterative` on one is an error rather than a silently
  different approximation, and the CLI reports on stderr whenever a run
  leaves the dense path. The threshold sits deliberately above the
  measured ~3 000-filament wall-clock crossover — where the dense path
  stops being tractable, not where it stops being fastest — because it is
  exact where the pFFT far field approximates. New criterion bench
  `fasterhenry/benches/scaling.rs` measures both paths end to end (960 to
  99 224 filaments, wall time plus model and peak-RSS memory) and
  `docs/benchmarks.md` records the numbers, hardware and date; a budgeted
  regression guard for the matrix-free headline joins the dense one in
  `fasterhenry/tests/perf_smoke.rs`. Determinism of the iterative path is
  asserted directly: repeated solves are bit-identical and `Z(ω)`,
  GMRES iteration counts and residuals are unchanged across rayon thread
  counts.

### Changed

- Above `fasterhenry::DENSE_PATH_MAX_FILAMENTS` (10 000) filaments,
  `fasterhenry run` and `fasterhenry_cli::run_reporting` now default to
  the pFFT + GMRES path (`--solver auto`) instead of the dense one (issue
  #44). Results agree with the dense path within the documented tolerance
  (better than `1e-4` relative on `Z`), the CLI notes the choice on
  stderr, `--solver dense` restores the previous behaviour, and decks with
  `.couples` stay dense. At or below the threshold nothing changes.
  `fasterhenry_cli::cli::Command::Run` gained a `solver` field, which is
  source-breaking for code that constructs that variant or matches it
  without `..`; the `cli` module is the binary's argument definition, not
  a supported library API.

## [0.1.0] - 2026-09-25

First release to crates.io.

### Added

- Graded contact regions in the ground-plane mesh (issue #36):
  `fasterhenry::plane::ContactRegion` refines a plane locally under a via
  landing — inside the rectangle the mesh is uniform at a declared fine
  cell, outside it each cell grows by a geometric `ratio` until it reaches
  the plane's background cell. `GroundPlane::mesh` returns the resulting
  `PlaneMesh` (per-axis cell edges) instead of a single uniform
  `(dx, dy)`; grading is applied per axis so the mesh stays a conforming
  tensor-product grid with no hanging nodes, regions compose with `Hole`s
  and with each other (overlapping regions merge at the finer cell and
  gentler ratio), and a region straddling the footprint edge is clipped
  while keeping its resolution. A plane declared without regions meshes
  bit-identically to before. Decks get a matching `.contact G<name> x1 y1
  x2 y2 [nx=] [ny=] [ratio=]` directive, following `.hole`'s
  plane-name lookup. Measured on a contact-dominated fixture: the graded
  mesh matches a fully uniform 25 µm plane to 0.10 % on L and 0.80 % on R
  using 580 filaments instead of 2 992 (5.2×), and its landing-separation
  differential matches an independent PyPEEC voxel reference to 0.12 %
  (`fasterhenry/tests/plane_validation.rs`, `docs/validation.md`).
- Iterative port-impedance solve on the pFFT operator (issue #43, phase 2
  of #24): `fasterhenry::IterativeSystem` computes the same `Z(ω)` as
  `MeshSystem` without forming `L` or the dense internal-loop block
  `Z_ee`. The action of `Z_ee = M_e (R + jωL) M_eᵀ` is two sparse
  loop-basis products around two real `PfftOperator` products, and
  `Z_ee⁻¹·Z_ep` is solved by a hand-rolled restarted, right-preconditioned
  GMRES (`fasterhenry::gmres`, real or complex arithmetic, Givens-rotation
  least squares) with a Jacobi preconditioner — one solve per port. DC is
  solved in real arithmetic without the operator, so its imaginary part is
  exactly zero. `IterativeParams` bundles the pFFT and GMRES parameters;
  a GMRES solve that misses its tolerance is reported as
  `SolveError::NotConverged`, and an operator that cannot be built as
  `SolveError::Pfft`. Matches the dense path to better than `1e-7`
  relative on the spiral and coupled-structure fixtures (tested at `1e-4`,
  including a floating ring and DC). The dense `MeshSystem` stays the
  default; the size threshold between the two is issue #44.
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
  plain-text `--zc-mat` experiment is superseded (pre-release CLI).
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

- `fasterhenry::plane::GroundPlane` gained a `contacts: Vec<ContactRegion>`
  field (issue #36). It is `Default`-able and empty means "uniform, exactly
  as before", but a struct-literal construction that lists every field now
  has to name it (`contacts: Vec::new()`) — the only source-breaking part
  of that issue.
- `PfftOperator::new` set-up: the near-field precorrection's local box
  potential (`near::grid_interactions`, issue #51's profiling found it about
  7-8× the cost of the exact kernel evaluation it is paired with, and about
  75% of total set-up at 10 000 filaments) now accumulates with a SIMD
  multiply-add instead of a scalar loop, about 10-15% off total set-up time
  at that size. A reusable phase-by-phase profiling test
  (`cargo test --release -p fasterhenry --lib pfft::near::profile --
  --ignored --nocapture`) and the profiling results, including the levers
  considered and set aside, are documented in the `pfft` module docs
  ("Set-up cost"). Accuracy and the `O(n)` scaling from issue #42 are
  unchanged.
- `fasterhenry-cli` now takes the library from `[workspace.dependencies]` with
  both a `path` and a `version`, so the CLI crate is publishable.

[Unreleased]: https://github.com/2AMLogic/fasterhenry/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/2AMLogic/fasterhenry/releases/tag/v0.1.1
[0.1.0]: https://github.com/2AMLogic/fasterhenry/releases/tag/v0.1.0
