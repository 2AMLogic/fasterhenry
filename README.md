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
parallel matrix fill, `num-complex`. No BLAS/LAPACK, no C dependencies: one
static binary on arm64 and x86-64.

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
