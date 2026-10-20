# Work Log

Chronological record of completed work in this repository, maintained by the Guide role.

<!-- Maintained automatically by the Guide triage agent. Manual edits are fine but may be overwritten. -->

### 2026-10-20

- **PR #202**: chore(loom): resync installed Loom to v0.19.1193

### 2026-10-17

- **PR #200**: chore(loom): resync installed Loom to v0.19.1182

### 2026-10-16

- **PR #194**: Bump Repo Skills 0.24.1 -> 0.24.3
- **PR #178**: fix(cli): reject output destinations that alias the input or each other
- **PR #164**: Deck reader: case-insensitive node names and .equiv pseudonyms
- **Issue #175** (closed): CLI: reject output destinations that alias the input or each other
- **Issue #156** (closed): Deck reader: case-insensitive node names and .equiv pseudonyms (documented)

### 2026-10-15

- **PR #179**: fix(cli): normalize SPICE subckt names derived from file names
- **PR #162**: feat: support segwid1/segwid2 meshed planes (#155)
- **Issue #176** (closed): SPICE export: normalize filename-derived subcircuit names into one safe token
- **Issue #155** (closed): Deck reader: support segwid1/segwid2 meshed planes (documented)
- **PR #188**: docs: record documented vs hypothesised plane node placement in compat table
- **PR #184**: Bump Repo Skills 0.19.5 -> 0.24.1
- **PR #177**: fix(cli): propagate final buffered MAT output failures
- **Issue #174** (closed): CLI: propagate final buffered MAT output failures
- **Issue #158** (closed): Investigate: FastHenry nonuniform-plane node placement vs our cell-centre model

### 2026-10-14

- **PR #169**: docs: align benchmark speed summary with table (1k serpentine within noise)
- **PR #153**: Compat: FastHenry .units spellings, missing .units, and repeated .units
- **PR #152**: Compat: clamp a contact cell at or above the rectangle width to 0.99x width
- **PR #151**: Compat: file=NONE plane with no initial grid is a single root cell
- **Issue #167** (closed): benchmarks.md summary contradicts its own 1,000-filament row
- **Issue #145** (closed): Compat: clamp a contact cell at or above the rectangle width to 0.99x width
- **Issue #144** (closed): Compat: FastHenry .units spellings, missing .units, and repeated .units
- **Issue #143** (closed): Compat: a ground plane with file=NONE and no initial grid is a single root cell
- **Issue #48** (closed): README: embed the fleet burndown chart (one line)

### 2026-10-13

- **PR #170**: Forbid unsafe code in fasterhenry-cli; CI guard for crate roots
- **PR #159**: docs: repo hygiene pass 2026-10-05
- **Issue #168** (closed): Add #![forbid(unsafe_code)] to the fasterhenry-cli crate

### 2026-10-12

- **Issue #181** (closed): Guard decision review: keep main-checkout worktree confinement flagged
- **Issue #180** (closed): Guard decision review: keep unresolved-variable worktree confinement flagged

### 2026-10-11

- **Issue #173** (closed): Guard review: keep gh-api-rawfield-body-literal-at protection

### 2026-10-06

- **Issue #157** (closed): Compat: approximate a diagonal contact trace by refining its bounding box

### 2026-10-05

- **Issue #146** (closed): Deck reader: support relx/rely/relz on ground planes (documented offset)
- **Issue #142** (closed): Compat: default an unspecified conductivity to copper (5.8e7 S/m), with a warning
- **Issue #141** (closed): Deck reader: accept whitespace around '=' on N, E and .default lines
- **Issue #136** (closed): Align parse warning ordering with reporting API documentation
- **Issue #134** (closed): Deck reader: a contact rect ending on the plane edge up to float rounding fails assembly with a zero-length segment on line 0
- **Issue #105** (closed): Deck reader: a hole whose xy lies entirely outside the plane's footprint is a silent no-op
- **Issue #35** (closed): Enable required status checks on main (a stale-green merge landed a failing change that needed a hotfix)

### 2026-09-30

- **Issue #131** (closed): Hygiene: the test suite leaves an untracked Zc.mat at the repo root

### 2026-09-29

- **Issue #124** (closed): Plane mesh: a background-resolution band merges with a finer one and widens it across the axis
- **Issue #122** (closed): Deck reader: 'file=' on a ground plane is an input hierarchy file, not an output option
- **Issue #118** (closed): Deck reader: contact cell sizes are applied along global x/y, but the public memo defines them in the plane's own p1->p2 coordinate system
- **Issue #116** (closed): Deck reader: a contact point/line met on one axis still perturbs that axis's mesh
- **Issue #113** (closed): Deck reader: 'contact initial_grid' / 'initial_mesh_grid' need the documented row/column convention
- **Issue #110** (closed): Deck reader: the 'contact trace' refinement primitive
- **Issue #95** (closed): Deck reader: 'contact rect' takes corners where the documented form takes a centre and widths
- **Issue #94** (closed): CI: extend the cargo-deny gate with a [sources] check (crates.io only, no git dependencies)

### 2026-09-28

- **Issue #114** (closed): inp parser panics instead of erroring on a deck with no segments and no planes
- **Issue #112** (closed): Simplify inp.rs: split the 394-line parse_plane_statement into per-clause helpers
- **Issue #109** (closed): Deck reader: the 'contact circle' refinement primitive
- **Issue #108** (closed): Simplify inp.rs: split the 804-line parse_with_options into per-directive functions
- **Issue #106** (closed): Docs: two stale cross-references left by #98's hole-model widening (CHANGELOG line 61, inp.rs intra-doc link)
- **Issue #101** (closed): Deck reader: named equipotential contacts ('equiv_rect', 'connection') and the initial-grid forms
- **Issue #100** (closed): Deck reader: the 'contact point' / 'line' / 'circle' / 'trace' refinement primitives
- **Issue #99** (closed): Deck reader: decide the disposition of 'hole user1'…'user7' (library predicate, or permanent rejection)
- **Issue #98** (closed): Deck reader: 'hole point' and 'hole circle' need a non-rectangular hole model
- **Issue #97** (closed): Ship a third-party license bundle for Apache-2.0-only dependencies with binary distributions
- **Issue #88** (closed): Deck reader: accept rho= directly on the corner-point G plane form
- **Issue #87** (closed): Consider #[non_exhaustive] on public solver enums (Discretization, SolveError) before 0.2.0
- **Issue #83** (closed): Deck reader: compatibility mode for third-party FastHenry decks (implicit first-line title)
- **Issue #80** (closed): Deck reader: ground-plane hole and contact shapes the plane model cannot represent yet
- **Issue #75** (closed): CI: cargo-deny license allowlist gate for the dependency tree
- **Issue #73** (closed): CLI: drop-in FastHenry mode (fasterhenry deck.inp writes Zc.mat) and --solver override
- **Issue #72** (closed): Deck reader: audit against the public FastHenry format and commit a compatibility table
- **Issue #71** (closed): Deck reader: rw/rh non-uniform filament ratios and wx/wy/wz width direction on segments
- **Issue #70** (closed): Deck reader: accept rho= (resistivity) and the full documented .units list
- **Issue #69** (closed): Deck reader: FastHenry ground-plane (G) statement syntax, with in-plane nodes, holes and contacts

### 2026-09-27

- **Issue #67** (closed): gitignore: add .claude/skills/repo/logs/ so guard-hook logs don't block fleet resync (2am#1184)
- **Issue #65** (closed): ci: Actions cache at 9.94/10 GB (99%) — no save-if on either rust-cache step, plus a 0.955 GB venv cached per ref
- **Issue #63** (closed): Slot-differential PyPEEC bound has only 0.5 pp of headroom now that the gate actually runs
- **Issue #44** (closed): pFFT scaling benchmark and dense/iterative size threshold

### 2026-09-25

- **Issue #59** (closed): PyPEEC validation gates silently skip in CI (relative reference path + SKIPPED on stderr)
- **Issue #51** (closed): pFFT: reduce PfftOperator set-up cost (precorrection box potential + near-pair kernels)
- **Issue #36** (closed): Plane mesh: graded contact-region refinement near via landings (uniform cells converge ~1/n)
- **Issue #34** (closed): CI: cache the PyPEEC venv (validation steps pay ~40 s of pip install per leg, every PR)

### 2026-09-24

- **Issue #53** (closed): Remove unused simba dependency: cargo-machete flags it, no code references it
- **Issue #43** (closed): GMRES solve on the pFFT operator, wired into the port-impedance path

### 2026-09-23

- **Issue #46** (closed): Warn when a `.couples` declaration is not transitive (risk of an indefinite L)
- **Issue #42** (closed): pFFT grid operator: projection, FFT far-field, near-field correction

### 2026-09-21

- **Issue #37** (closed): Validation: trace-over-plane image oracle — redesign the fixture or retire the oracle claim
- **Issue #33** (closed): Reject numerically unusable surface-graded filament grids

### 2026-10-06

- **PR #163**: Compat: approximate diagonal contact trace by padded bounding box (#157)

### 2026-10-05

- **PR #149**: Deck reader: support relx/rely/relz on ground planes (documented offset)
- **PR #147**: tools: FastHenry head-to-head script + example-corpus results (#74)
- **PR #148**: Compat: default an unspecified conductivity to copper (5.8e7 S/m), with a warning
- **PR #150**: Deck reader: accept whitespace around '=' on N, E and .default lines
- **PR #140**: docs: note that merges into main are CI-gated (#35)
- **PR #139**: fix: restore main build — tag contact touch warnings with clause order
- **PR #138**: Return parse warnings in source-clause order
- **PR #137**: fix: contacts touching the plane edge no longer fail assembly (#134)
- **PR #135**: feat(inp): warn when a hole or contact rect lies wholly off its plane

### 2026-09-30

- **PR #89**: chore: update Repo Skills 0.11.17 → 0.14.1
- **PR #133**: chore: ignore the CLI's default Zc.mat output

### 2026-09-29

- **PR #132**: fix(deck): accept 'file=NONE' and reject a named hierarchy file correctly
- **PR #129**: fix(deck): read a contact clause's x/y lengths in the plane's own coordinate system
- **PR #130**: feat(deck): read 'contact rect' in its documented centre/widths/cell form too
- **PR #121**: feat(deck): 'contact trace' along x or y as its documented five contact lines
- **PR #128**: fix(plane): drop a whole-axis background band before merging bands
- **PR #127**: ci(deny): gate dependency sources on crates.io only, no git dependencies
- **PR #126**: feat(deck): 'contact initial_grid' / 'initial_mesh_grid' with the documented row/column convention
- **PR #125**: fix(deck): a one-axis-met contact point/line leaves that axis's mesh alone

### 2026-09-28

- **PR #123**: fix: error instead of panic on a deck with no conductor
- **PR #120**: feat(deck): 'contact circle' is not a documented shape — permanent rejection with the bounding-square alternative
- **PR #119**: refactor: split parse_plane_statement into per-clause helpers
- **PR #117**: Deck reader: named contact areas ('contact equiv_rect', 'contact connection')
- **PR #115**: feat(inp): contact point / contact line refinement clauses
- **PR #111**: refactor(inp): split the deck parser into a DeckBuilder with per-directive methods
- **PR #107**: decide: 'hole user1'…'user7' are permanently rejected (issue #99)
- **PR #104**: feat(plane): non-rectangular hole model (hole point / hole circle)
- **PR #103**: feat(ci): ship a third-party license bundle for the Apache-2.0-only dependencies
- **PR #102**: feat(inp): accept the FastHenry 'contact decay_rect' ground-plane clause
- **PR #93**: ci: gate the dependency tree on a cargo-deny license allowlist
- **PR #96**: feat(cli): drop-in FastHenry invocation mode (fasterhenry deck.inp)
- **PR #92**: feat(api): mark growing public enums #[non_exhaustive] before 0.2.0
- **PR #91**: feat(inp): accept rho= on the corner-point G plane statement
- **PR #90**: feat(inp): opt-in FastHenry compat mode with an implicit first-line title
- **PR #85**: Deck reader: rw/rh filament ratios and wx/wy/wz width direction
- **PR #86**: docs: refresh WORK_PLAN/WORK_LOG; add tools/ and docs/ READMEs
- **PR #84**: Deck reader: audit against public FastHenry format, commit compatibility table
- **PR #82**: Deck reader: accept rho= and the full documented .units list
- **PR #81**: feat(inp): accept FastHenry's ground-plane (G) statement syntax

### 2026-09-27

- **PR #78**: docs: record thin margin on the slot-differential PyPEEC bounds
- **PR #79**: chore(gitignore): ignore .claude/skills/repo/logs/ so guard-hook logs don't block fleet resync
- **PR #77**: release: 0.1.1
- **PR #56**: feat(scaling): pFFT/dense size threshold and the benchmark behind it (#44)
- **PR #68**: ci: tag-triggered crates.io release via Trusted Publishing
- **PR #66**: ci: restrict Actions cache saves to main (rust-cache save-if, PyPEEC venv split)

### 2026-09-18

- **Project bootstrapped** (from klayout-tools#1886 / #2083: FastHenry is MIT-noncommercial and unpackaged; PyPEEC chosen as the validation oracle; the modernization is a clean-room Rust engine)
  - Project type: library + CLI (Rust workspace)
  - Stack: nalgebra + simba/wide (SIMD), rayon, num-complex; no BLAS/LAPACK
  - Visibility: public, MIT
  - CI: fmt / clippy / build / test on ubuntu x86-64, ubuntu arm64, macOS; clean-room grep gate

### 2026-09-27

- **0.1.0 published to crates.io** (`fasterhenry`, `fasterhenry-cli`; #9, release prep PR #64) — first release: dense + pFFT/GMRES solvers, ground planes, skin-depth grading, coupling truncation, MAT v4 / SPICE output
- **Tag-triggered release workflow** (PR #68): crates.io Trusted Publishing on `v*` tags, resumes a partial publish on re-run
- **0.1.1 published** (PR #77): dense/iterative size threshold and `--solver` (#44, PR #56). Library published by the release workflow; CLI by hand after its Trusted Publishing entry was found misconfigured (fixed)
- **0.2.0 epic filed** (#76, children #69–#75): drop-in FastHenry replacement. #69 (FastHenry `G` plane syntax) merged

### 2026-09-28

- **Issue #6** (closed): Epic: fasterhenry M0 → release — dense validated core, physics completeness, acceleration, crates.io

### 2026-09-27

- **Issue #9** (closed): Publish 0.0.1 to crates.io to reserve fasterhenry + fasterhenry-cli (once M0 merits it)
- **Issue #24** (closed): Precorrected-FFT acceleration for large problems

### 2026-09-26

- **PR #64**: Release readiness for 0.1.0 on crates.io (#9)

### 2026-09-25

- **PR #62**: ci: make the PyPEEC spiral and plane validation gates actually run
- **PR #61**: feat(plane): graded contact-region refinement near via landings
- **PR #60**: perf(inductance): fix memoization hash so near-field bundle throughput actually improves (issue #14 investigation)
- **PR #58**: perf(pfft): profile and speed up PfftOperator set-up
- **PR #57**: ci: cache the PyPEEC venv across runs on both Linux legs
- **Issue #14** (closed): Kernel throughput: SIMD lanes gain only ~1.2x and near-field bundle pairs dominate assembly cost

### 2026-09-24

- **PR #55**: feat(iterative): GMRES port-impedance solve on the pFFT operator
- **PR #54**: chore: remove unused direct simba dependency

### 2026-09-23

- **PR #52**: feat: reject non-transitive .couples declarations
- **PR #50**: feat(pfft): matrix-free precorrected-FFT partial-inductance operator
- **PR #49**: test: validate width-graded skin effect against an independent 2-D cross-section solve
- **Issue #32** (closed): Validate width-graded skin effect against an independent 2D reference

### 2026-09-22

- **PR #47**: feat(bench): committed criterion suite for assembly/solve throughput + CI table regen
- **PR #45**: feat: coupling truncation (.couples) for sparse assembly
- **Issue #27** (closed): Benchmark: operator-run FastHenry head-to-head + a committed, hardware-normalized perf table
- **Issue #25** (closed): Coupling truncation (.couples-style) for sparse assembly

### 2026-09-21

- **PR #41**: fix: reject ill-conditioned graded filament grids
- **PR #40**: feat: expose unresolved pairs in detailed inductance batches
- **PR #39**: docs: retire the trace-over-plane image accuracy claim
- **PR #31**: Surface-graded filament subdivision
- **PR #29**: feat: ground planes (G/.hole) with PyPEEC slot-differential validation
- **PR #26**: feat: binary Zc.mat (MAT v4) and SPICE subcircuit output
- **Issue #30** (closed): Inductance kernel: silently-unresolved near-singular pairs + no 2D reference for width-graded grid divergence
- **Issue #23** (closed): Skin-depth-adaptive filament subdivision
- **Issue #22** (closed): Ground planes (G directive) with holes
- **Issue #21** (closed): Output formats: binary Zc.mat (MAT v4) and SPICE subcircuit export
- **Issue #15** (closed): Batch inductance API should surface unresolved (reduced-accuracy) pairs; add independent reference for that regime

### 2026-09-20

- **PR #20**: feat: validation harness — PyPEEC + Greenhouse/Mohan references on the 2-turn spiral
- **PR #19**: feat: CLI run subcommand — .inp deck reader, JSON problem documents, sweep output
- **PR #18**: feat: mesh basis, dense complex solve, and port-impedance frequency sweep
- **PR #17**: fix: harden geometry/discretize against overflow, extreme magnitudes, and unknown fields
- **PR #13**: feat: add partial inductance kernels (closed forms, adaptive Neumann quadrature, SIMD batch)
- **PR #10**: chore: make both crates packageable and dry-run the publish in CI
- **Issue #8** (closed): Publish-readiness: packageable crates + cargo publish --dry-run in CI
- **Issue #5** (closed): CLI: FastHenry .inp deck reader (public format) + JSON in/out
- **Issue #4** (closed): Validation harness: PyPEEC + Mohan/Greenhouse closed forms on a 2-turn spiral (≤2 %)
- **Issue #3** (closed): Mesh assembly + dense complex solve + frequency sweep
- **Issue #2** (closed): Partial self/mutual inductance kernels (Grover/Rosa closed forms + arbitrary-orientation quadrature, SIMD-batched)
- **Issue #12** (closed): Harden geometry/discretize inputs: nw*nh overflow panic, misleading ZeroLength on norm overflow, unknown serde fields

### 2026-09-19

- **PR #7**: Install Loom v0.19.174 and Repo Skills v0.11.17
- **PR #11**: feat: add geometry model and uniform filament discretization
- **Issue #1** (closed): Geometry model + filament discretization
