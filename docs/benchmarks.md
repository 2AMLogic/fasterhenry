# Benchmarks: fasterhenry vs FastHenry

Method, numbers, and caveats for the head-to-head the project has wanted
since the beginning. Everything below is reproducible from self-authored
decks; **no FastHenry code is in this repository** — the binary was built
from the public mirror into a scratch container (compile only, build
flags `-std=gnu89 -fcommon` for 1994 K&R C on modern gcc), per the
operator's decision that running the original tool for comparison
contaminates nothing. CI never invokes it.

## Setup

- **Hardware**: Apple M3 Ultra, 28 cores, host under background load
  (~2× slowdown typical; both engines equally affected).
- **FastHenry**: ediloren/FastHenry2 `master`, built in a native arm64
  `debian:stable` Docker container (gcc 14). Defaults: iterative GMRES +
  multipole, order 2, preconditioner on. Its timing is single-threaded.
- **fasterhenry**: this tree, `--release`, dense path, rayon over all 28
  threads unless noted.
- Decks: identical self-authored `.inp` files where the dialects overlap;
  timing measured as process wall time.

## Speed

| deck | filaments | FastHenry (multipole, 1 thread) | fasterhenry (dense, 28 threads) | fasterhenry (dense, 1 thread) |
|---|---|---|---|---|
| spiral | 48 | 31 ms | **10 ms** | — |
| plane_trace | ~250 | 21 ms | **13 ms** | — |
| serpentine | 1 000 | 69 ms | **73 ms** | — |
| serpentine | 5 000 | 4.52 s | **2.06 s** | 31.6 s |
| serpentine | 19 600 | 103.7 s | **83.4 s** | 1 475 s |
| serpentine | 80 000 | did not finish (> 30 min) | not attempted (dense) | — |

Reading it honestly:

- **Wall clock on a many-core machine, fasterhenry's dense path beats
  FastHenry's multipole on every completed deck** — 3× at small sizes,
  ~1.2× at 20 k filaments, crossover around 30–50 k where neither
  finishes comfortably.
- **Per thread, FastHenry's multipole is far ahead at scale** (14× at
  20 k filaments single-threaded). Our dense wall-clock advantage is
  parallelism + SIMD + a blocked LU; it is not an algorithmic win.
  Precorrected-FFT acceleration (#24) is what converts this into one.

## Accuracy (the replacement claim)

Same decks, impedance matrices compared entry by entry
(FastHenry's ASCII `Zc.mat` vs our JSON):

| deck | frequency | relative difference |
|---|---|---|
| spiral | 1 MHz | **7.3e-4** |
| spiral | 10 MHz | 4.5e-3 |
| serpentine 1k | 1 MHz | **4.6e-4** |
| serpentine 5k | 1 MHz | **3.8e-4** |

Sub-0.1 % engine agreement on shared segment fixtures — the strongest
cross-validation available to this project, and the first literal
FastHenry comparison in either this repository or klayout-tools.

**Open**: the trace-over-plane deck disagrees (FastHenry 6.65 nH vs our
3.2 nH at 1 GHz) because FastHenry's ground planes connect through
explicit `contact` regions with their own adaptive meshing
(`contact decay_rect … nhinc= rh=`) — our deck's vias never attached, so
FastHenry solved trace-plus-dangling-stubs (its DC resistance equals the
trace alone, confirming the diagnosis). A fair plane comparison needs
matched contact modeling in their dialect; tracked in #27 alongside the
committed criterion-based benchmark suite.

## Deck dialect notes (for the record)

Both tools read the common core (`.units`, `.default` with `w/h/nwinc/
nhinc/sigma`, `N`/`E` keyword lines, `.external` two-node ports,
`.freq`). Differences observed: FastHenry's `G` uses three corner points
(`x1..z3`), `thick=` and `sigma=`, requires `seg1`/`seg2`, and has no
`nx=`/`ny=` (discretization comes from `contact` regions); its default
`Zc.mat` is ASCII text, not MAT v4; `rho` is accepted for segments.
fasterhenry's dialect (two corners + thickness + `nx`/`ny`, automatic
landing-snap, `sigma` in deck units) is documented in
`fasterhenry-cli/src/inp.rs`.
