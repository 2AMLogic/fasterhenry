# Work Log

Chronological record of completed work in this repository, maintained by the Guide role.

<!-- Maintained automatically by the Guide triage agent. Manual edits are fine but may be overwritten. -->

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
