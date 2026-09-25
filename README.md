# fasterhenry

A clean-room, MIT-licensed Rust engine for **PEEC** (partial-element equivalent
circuit) extraction of resistance and inductance of 3-D conductor geometries:
spirals, busbars, bond wires, on-chip interconnect. FastHenry's problem class,
none of its code, and built for today's hardware (portable SIMD, all cores).

## Vision

FastHenry (MIT RLE, 1994) is still the reference tool for frequency-dependent
R/L extraction, and it is unmaintained, single-threaded C under a license that
permits only internal, noncommercial use and forbids redistribution. Every
open-source EDA flow that needs partial inductances either shells out to it in
a legal grey zone or does without. `fasterhenry` is the replacement: the
published method, implemented from the papers, released under MIT, fast enough
to sit inside a design loop.

What exists today: a filament model with uniform, surface-graded and
skin-depth-graded subdivision; partial self/mutual inductance kernels;
ground planes with holes and graded contact regions; coupling truncation;
mesh assembly with a dense complex solve over a frequency sweep; and a
matrix-free precorrected-FFT operator with a GMRES solve for large problems.
Every piece is cross-checked against independent references (PyPEEC, the
Greenhouse closed forms, and a head-to-head with FastHenry itself) —
see [`docs/validation.md`](docs/validation.md). The CLI reads FastHenry
`.inp` decks and writes JSON, a MAT v4 `Zc.mat`, or a SPICE subcircuit.

## Method

Ruehli's PEEC formulation; the FastHenry approach of Kamon, Tsuk and White
(*FASTHENRY: a multipole-accelerated 3-D inductance extraction program*, IEEE
Trans. MTT 42(9), 1994); Grover/Rosa closed forms for parallel filament
partial inductances; numerical quadrature for arbitrary orientation. See
`docs/` as the design lands.

## Clean-room rule

This project must never contain code derived from MIT's FastHenry or FastCap,
in any branch or mirror (`ediloren/FastHenry2`, `wrcad/xictools`, …). Their
notice grants "internal, noncommercial" use only and prohibits distribution
of copies or derivatives — a port could never be released. Contributors work
from the papers and from this repository's own code. Reading the FastHenry
`.inp` deck *format* is fine (a file format is not code); reading FastHenry
*source* while contributing here is not. See `CONTRIBUTING.md`.

## Stack

Rust (stable), `nalgebra` + `simba`/`wide` for portable SIMD, `rayon` for
parallel matrix fill, `num-complex`, and `rustfft` (pure Rust, MIT OR
Apache-2.0) for the precorrected-FFT operator. No BLAS/LAPACK, no C
dependencies: one static binary on arm64 and x86-64.

## Performance

The committed, regenerated numbers live in
[`docs/benchmarks.md`](docs/benchmarks.md#committed-criterion-bench-suite):
a `cargo bench` (criterion) sweep of assembly- and solve-stage throughput
against filament count (`fasterhenry/benches/assembly.rs`,
`fasterhenry/benches/solve.rs`; kernel-level throughput is
`fasterhenry/benches/kernels.rs`), regenerated on demand by
`.github/workflows/bench.yml` on a pinned `ubuntu-latest` runner —
parsed straight from criterion's own JSON output, not hand-typed off
whichever machine ran it last. The `perf_smoke` CI test backs the same
posture with a hard budget, on every run: a 2 000-filament dense sweep
must solve within 10 s on that same pinned runner class
(`fasterhenry/tests/perf_smoke.rs`).

The dense path is parallel across all cores and SIMD-batched in the
kernels, so it is dramatically faster than a single-threaded 1994-era
solver on modern hardware — for problems that fit the dense regime
(~10⁴ filaments comfortably, ~10⁵ with patience and memory). Beyond that,
the library's `IterativeSystem` (precorrected FFT + GMRES, #24) computes
the same impedance matrix in near-linear time and memory; the size
threshold at which it becomes the default is being measured in #44.

Measured head-to-head against the original FastHenry (operator-run,
one machine, self-authored decks; method, hardware and caveats in
[`docs/benchmarks.md`](docs/benchmarks.md)):
fasterhenry's dense path is faster on wall clock at every size up to
~20 000 filaments (3× at small sizes, ~1.2× at 20 k, where FastHenry's
multipole stays 14× ahead per thread — our edge is parallelism today,
algorithmics pending #24). On shared segment fixtures the two engines
agree to **better than 0.1 %** on the extracted impedance — the
cross-validation behind the "replacement" claim.

## Layout

| crate | what |
|---|---|
| `fasterhenry` | the library: geometry, filaments, kernels, assembly, solve |
| `fasterhenry-cli` | `fasterhenry` binary: `.inp`/JSON in, JSON out |

## Development

This repository is developed with [Loom](https://github.com/rjwalters/loom)
orchestration. To drive an approved issue through Curator → Builder → Judge →
Doctor → Merge:

```bash
cd fasterhenry
/loom:sweep <issue>
```

## License

MIT — see `LICENSE`.
