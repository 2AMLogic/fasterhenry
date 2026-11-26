# Work Log Archive — 2026-09

Sections moved byte-for-byte from [WORK_LOG.md](../WORK_LOG.md) by `loom-daemon work-docs archive`.

### 2026-09-26

- **PR #64**: Release readiness for 0.1.0 on crates.io (#9)

### 2026-09-25

- **Issue #59** (closed): PyPEEC validation gates silently skip in CI (relative reference path + SKIPPED on stderr)
- **Issue #51** (closed): pFFT: reduce PfftOperator set-up cost (precorrection box potential + near-pair kernels)
- **Issue #36** (closed): Plane mesh: graded contact-region refinement near via landings (uniform cells converge ~1/n)
- **Issue #34** (closed): CI: cache the PyPEEC venv (validation steps pay ~40 s of pip install per leg, every PR)

### 2026-09-25

- **PR #62**: ci: make the PyPEEC spiral and plane validation gates actually run
- **PR #61**: feat(plane): graded contact-region refinement near via landings
- **PR #60**: perf(inductance): fix memoization hash so near-field bundle throughput actually improves (issue #14 investigation)
- **PR #58**: perf(pfft): profile and speed up PfftOperator set-up
- **PR #57**: ci: cache the PyPEEC venv across runs on both Linux legs
- **Issue #14** (closed): Kernel throughput: SIMD lanes gain only ~1.2x and near-field bundle pairs dominate assembly cost

### 2026-09-24

- **Issue #53** (closed): Remove unused simba dependency: cargo-machete flags it, no code references it
- **Issue #43** (closed): GMRES solve on the pFFT operator, wired into the port-impedance path

### 2026-09-24

- **PR #55**: feat(iterative): GMRES port-impedance solve on the pFFT operator
- **PR #54**: chore: remove unused direct simba dependency

### 2026-09-23

- **Issue #46** (closed): Warn when a `.couples` declaration is not transitive (risk of an indefinite L)
- **Issue #42** (closed): pFFT grid operator: projection, FFT far-field, near-field correction

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

- **Issue #37** (closed): Validation: trace-over-plane image oracle — redesign the fixture or retire the oracle claim
- **Issue #33** (closed): Reject numerically unusable surface-graded filament grids

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
### 2026-09-18

- **Project bootstrapped** (from klayout-tools#1886 / #2083: FastHenry is MIT-noncommercial and unpackaged; PyPEEC chosen as the validation oracle; the modernization is a clean-room Rust engine)
  - Project type: library + CLI (Rust workspace)
  - Stack: nalgebra + simba/wide (SIMD), rayon, num-complex; no BLAS/LAPACK
  - Visibility: public, MIT
  - CI: fmt / clippy / build / test on ubuntu x86-64, ubuntu arm64, macOS; clean-room grep gate

