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

Current milestone: **M0 — dense core, validated**. A filament model, partial
self/mutual inductance kernels, mesh assembly, a dense complex solve with a
frequency sweep, and a validation harness that cross-checks a spiral fixture
against PyPEEC and the Mohan/Greenhouse closed forms. Acceleration (FFT/FMM)
and skin/proximity refinement are M1.

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
whichever machine ran it last. The `perf_smoke` CI tests back the same
posture with hard budgets, on every run: a 2 000-filament dense sweep
within 10 s on that same pinned runner class, and a 4 760-filament
matrix-free sweep within 120 s and under half the dense path's working
set (`fasterhenry/tests/perf_smoke.rs`).

The dense path is parallel across all cores and SIMD-batched in the
kernels, so it is dramatically faster than a single-threaded 1994-era
solver on modern hardware — for problems that fit the dense regime,
because its working set is `8n²` bytes of partial inductances before the
factorization is counted. Beyond that regime the matrix-free path takes
over: a precorrected-FFT operator (#42) under GMRES (#43), linear in
memory and near-linear in time.

Measured on one machine (AWS 8 vCPU, one thread, 2026-09-25) over
assembly plus a one-frequency sweep of a square-meander fixture, dense
against pFFT + GMRES — full table, method and caveats in
[`docs/benchmarks.md`](docs/benchmarks.md#scaling-dense-vs-pfft--gmres-and-where-the-default-switches):

| filaments | dense | pFFT + GMRES | dense working set |
|---|---|---|---|
| 2 400 | **4.2 s** | 4.7 s | 0.15 GB |
| 9 800 | 156 s | **19.4 s** | 2.5 GB |
| 29 928 | not attempted | **61 s** | 23 GB |
| 99 224 | not attempted | **207 s** | 256 GB |

GMRES converges in 5 iterations at every size, and `Z(ω)` is reproducible
run to run and thread-count to thread-count.

The wall-clock crossover is near 3 000 filaments, but the **default**
switches at `fasterhenry::DENSE_PATH_MAX_FILAMENTS` = 10 000: the dense
path is exact where the matrix-free one approximates the far field
(`< 1e-4` on `Z`), so the handover is placed where dense stops being
affordable on any geometry rather than where it stops being fastest.
`--solver dense` / `--solver iterative` (library: `SolverChoice`) force
either path at any size, and the CLI says on stderr when a run leaves the
dense path.

Measured head-to-head against the original FastHenry (operator-run,
one machine, self-authored decks; method, hardware and caveats in
[`docs/benchmarks.md`](docs/benchmarks.md)):
fasterhenry's dense path is faster on wall clock at every size up to
~20 000 filaments (3× at small sizes, ~1.2× at 20 k, where FastHenry's
multipole stays 14× ahead *per thread* — that dense-path edge is
parallelism; the algorithmic answer is the pFFT + GMRES path above).
On shared segment fixtures the two engines
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
