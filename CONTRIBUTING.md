# Contributing

## The one hard rule: clean room

`fasterhenry` is a from-scratch implementation of published methods. It must
never contain code, comments, data tables, or test fixtures derived from MIT's
FastHenry or FastCap, from any mirror or fork (`ediloren/FastHenry2`,
`ediloren/FastCap2`, `wrcad/xictools`'s `fasthenry-3.0wr` tarball, Blender or
FreeCAD wrappers that vendor it, …).

Why: those sources carry MIT RLE's 1990s notice — *"Permission to use, copy and
modify for internal, noncommercial purposes is hereby granted. Any distribution
of this program or any part thereof is strictly prohibited without prior
written consent of M.I.T."* A derivative work is a copy; it could never be
released under MIT, and this project exists to be released.

What is allowed and encouraged:

- The **papers**: Ruehli (PEEC); Kamon, Tsuk, White (FastHenry, IEEE T-MTT
  1994); Grover, *Inductance Calculations*; Rosa (1908); Mohan et al. (spiral
  closed forms, JSSC 1999); Greenhouse (1974).
- The **FastHenry `.inp` deck format** as an input convenience — a file format
  is not code. Write the parser from the format description and from example
  decks you author yourself.
- **Independent oracles** for validation: PyPEEC (MPL-2.0), closed forms,
  measured data.

If you have read FastHenry source code in the past, that is fine; do not have
it open while writing code here, and do not transcribe from memory. CI greps
every file for the MIT notice text and fails the build if it appears.

## Everything else

- Rust stable, `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` green.
- `#![forbid(unsafe_code)]` in the library stays. SIMD goes through
  `wide`/`simba`, not intrinsics.
- No BLAS/LAPACK or other C/Fortran dependencies.
- One PR per issue; PRs are reviewed by Loom's Judge and merged by Champion.
