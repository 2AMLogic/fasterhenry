# Work Log

Chronological record of completed work in this repository, maintained by the Guide role.

<!-- Maintained automatically by the Guide triage agent. Manual edits are fine but may be overwritten. -->

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
