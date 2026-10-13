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
  Precorrected-FFT acceleration (#24) is what converts this into one —
  measured in "Scaling: dense vs pFFT + GMRES" below.

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

**Planes, resolved (#36)**: the trace-over-plane deck used to disagree
(FastHenry 6.65 nH vs our 3.2 nH at 1 GHz) because FastHenry's ground
planes connect through explicit `contact` regions with their own adaptive
meshing (`contact decay_rect … nhinc= rh=`) — our deck's vias never
attached, so FastHenry solved trace-plus-dangling-stubs (its DC resistance
equals the trace alone, confirming the diagnosis). The engine now has the
matching capability: a `.contact` region on a `G` line refines the plane
mesh locally under a landing and decays geometrically back to the
background cell, so a deck can express the same contact modelling their
dialect needs (`fasterhenry::plane::ContactRegion`; see
`docs/validation.md` § *Graded contact regions*, where the graded mesh
matches a fully-fine uniform plane to 0.10 % on L at 5.2× fewer filaments,
and an independent PyPEEC separation differential to 0.12 %).

What is **still open** is the head-to-head itself: rerunning the
trace-over-plane deck against FastHenry with contacts declared on both
sides is an operator-run, single-machine measurement like the rest of this
section, and it has not been redone since the capability landed. The
number above is therefore left standing as the last measured one, not
claimed as fixed. This repository's own plane accuracy is gated
independently by the PyPEEC slot and contact differentials, which need no
FastHenry.

## Committed criterion bench suite

The head-to-head above is operator-run, on a single machine, by hand —
useful for the replacement claim, not something CI can regenerate. This
section is the automatable half: `fasterhenry/benches/assembly.rs` and
`fasterhenry/benches/solve.rs` are `cargo bench` (criterion) suites that
measure `MeshSystem::assemble` and `MeshSystem::sweep` throughput against
a fixed, synthetic filament sweep (48, ~250, 1 000, 5 000, 19 600 —
matching the Speed table's scale, not its decks; see the module docs in
those files for the geometry). `fasterhenry/benches/kernels.rs` benchmarks
the underlying partial-inductance kernels the same way (scalar vs. SIMD vs.
SIMD+rayon), `fasterhenry/benches/pfft.rs` the matrix-free operator, and
`fasterhenry/benches/scaling.rs` the dense-versus-iterative sweep of the
next section; none of the three is part of the table below.

Run locally with `cargo bench -p fasterhenry --bench assembly --bench
solve` (the full sweep; large problems take real time — see the module
docs). The table below is regenerated by `.github/workflows/bench.yml`
(`workflow_dispatch`, pinned to `ubuntu-latest`) via `tools/bench_table.py`,
which parses criterion's own JSON output — nothing here is hand-typed. That
workflow caps the sweep with `FASTERHENRY_BENCH_MAX_FILAMENTS` to fit its
time budget, so the largest point may be missing from a CI-produced table
even though the bench suite itself defines it.

<!-- bench-table:begin -->
_Not yet regenerated by CI — run `.github/workflows/bench.yml`
(`workflow_dispatch`) or `cargo bench -p fasterhenry --bench assembly
--bench solve && python3 tools/bench_table.py` locally to fill this in._
<!-- bench-table:end -->

## Scaling: dense vs pFFT + GMRES, and where the default switches

The section above measures the dense path against FastHenry. This one
measures it against *us* — `MeshSystem` (dense assembly of `L`, dense LU
of the internal-loop block) against `IterativeSystem` (#43: GMRES on the
matrix-free precorrected-FFT operator, #42) — and is the measurement
behind `fasterhenry::DENSE_PATH_MAX_FILAMENTS`, the filament count at
which `SolverChoice::Auto` and the CLI's `--solver auto` hand over from
one path to the other.

- **Measured**: 2026-09-25, `fasterhenry/benches/scaling.rs` at
  `feature/issue-44`, `--release`, on a tree that predates two speed-ups
  since merged to `main` — #58 (pFFT operator set-up, about 10–15 % off
  set-up at 10 000 filaments) and #60 (memoized aligned-bar kernel, which
  speeds dense near-field assembly). The *model* memory columns are
  unaffected by either; the wall-clock columns are that pre-#58/#60
  measurement and have not been re-run, so read them as an upper bound
  on both arms. The threshold rests on the memory wall, which neither
  change moves.
- **Hardware**: AWS EC2, 8 vCPU (Xeon-class), 30 GB RAM, Linux.
  **One thread**: the measuring process was CPU-budgeted to a single core
  by its cgroup, so rayon ran one worker and *both* arms are
  single-threaded, like the "fasterhenry (dense, 1 thread)" column above.
  Treat the crossover as a per-thread figure; a many-core run moves both
  arms, not obviously by the same factor.
- **Fixture**: a square meander — `bars` conductor runs at a 100 µm pitch,
  each cut into `bars` collinear one-pitch segments, joined by jogs, one
  port across the chain, 2 × 2 filaments per segment. The footprint stays
  square and the segments stay short as the size grows, so the pFFT grid
  is charged for the problem's size and not for its aspect ratio (module
  docs in `benches/scaling.rs` explain why the serpentine of
  `benches/support.rs` is the wrong fixture for this).
- **Timed**: `assemble` + `sweep(&[1 MHz])`, end to end — the dense path's
  cost is dominated by the `O(n²)` assembly, the iterative path's by the
  GMRES solve, so timing the solve stage alone would flatter the dense
  path by exactly the term that makes it unaffordable.
- **Memory**: *model* is what each path must hold by construction (`8n²`
  for `L` plus `16m²` twice for the complex `m × m` internal-loop block it
  factorizes, against `PfftStats::memory_bytes` plus GMRES's `restart + 1`
  Krylov vectors and the transient FFT buffers) — machine-independent.
  *peak* is the process high-water mark (`VmHWM`) around each measurement.

| filaments | loops | dense s | dense model | dense peak | iterative s | iterative model | iterative peak | GMRES its |
|---|---|---|---|---|---|---|---|---|
| 960 | 720 | **0.84** | 24 MB | 61 MB | 1.71 | 53 MB | 55 MB | 5 |
| 2 400 | 1 800 | **4.15** | 150 MB | 227 MB | 4.69 | 129 MB | 130 MB | 5 |
| 4 760 | 3 570 | 20.9 | 589 MB | 635 MB | **8.95** | 240 MB | 223 MB | 5 |
| 9 800 | 7 350 | 156 | 2 497 MB | 2 198 MB | **19.4** | 487 MB | 416 MB | 5 |
| 29 928 | 22 446 | not attempted | 23 GB | — | **60.9** | 1 474 MB | 1 204 MB | 5 |
| 99 224 | 74 418 | not attempted | 256 GB | — | **207** | 4 803 MB | 3 831 MB | 5 |

The two "not attempted" rows are the point of the table: 23 GB does not
fit this host and 256 GB does not fit most, while the same problems cost
the iterative path 1.5 GB and 4.8 GB. The bench caps its dense arm at
`DENSE_PATH_MAX_FILAMENTS` for that reason;
`FASTERHENRY_BENCH_DENSE_MAX_FILAMENTS` lifts the cap for anyone with the
memory to spend.

Reading it honestly:

- **The wall-clock crossover is near 3 000 filaments** — dense wins at
  960, the two are within 13 % at 2 400, and the iterative path is 2.3×
  ahead at 4 760 and 8× ahead at 9 800.
- **Time grows superquadratically on the dense arm and about linearly on
  the iterative one.** Doubling the dense problem (2 400 → 4 760 → 9 800)
  multiplies its time by 5.0 then 7.5. Tripling the iterative problem
  (9 800 → 29 928 → 99 224) multiplies its time by 3.1 then 3.4 — near
  linear, with the `n log n` of the transforms and a grid that grows with
  the footprint accounting for the excess.
- **GMRES converges in 5 iterations at every size.** Convergence is
  mesh-independent here, so the iterative arm's growth is the cost of a
  product, not a worsening conditioning.
- **Memory is the harder wall.** `8n²` bytes of `L` is 256 GB at 100 k
  filaments before the `16m²` factorization is counted; the operator and
  the Krylov basis are linear.

### The threshold, and why it is not 3 000

`fasterhenry::DENSE_PATH_MAX_FILAMENTS` is **10 000**: at or below it
`SolverChoice::Auto` (the library default, and `--solver auto`) keeps
`MeshSystem`; above it, `IterativeSystem`. That is deliberately *above*
the wall-clock crossover, and the table shows the price — a default run
at 9 800 filaments takes the 156 s column, not the 19.4 s one. The
reasons to pay it:

- **The dense path is exact; the iterative path approximates.** The pFFT
  far field agrees with the exact kernel to about `1e-6` on `L·x` and the
  two paths agree to better than `1e-4` on `Z`. Changing which answer a
  user gets by default should happen where the alternative is
  *unaffordable*, not merely where it is slower.
- **The crossover is geometry-dependent; the memory wall is not.** A
  geometry with a few filaments much longer than the rest, or a strongly
  elongated footprint, drives the pFFT grid spacing down and its
  projection cost up, pushing the crossover to larger sizes. `8n²` bytes
  is `8n²` bytes on every geometry.
- **10 000 is one size below intractable**, not one above comfortable:
  2.5 GB at the threshold, 23 GB at the next point up.

Below the threshold the iterative path is one flag away —
`fasterhenry run deck.inp --solver iterative`, or
`run_reporting_with(…, SolverChoice::Iterative)` — and the CLI prints a
line on stderr whenever a run leaves the dense path, so which path
produced a given JSON is never ambiguous. A deck that truncates coupling
(`.couples`) stays dense at any size: truncation is a dense-path feature,
and `--solver iterative` on such a deck is an error rather than a
silently different approximation.

### Determinism

`Z(ω)` from the iterative path is reproducible, which is what makes these
numbers a baseline rather than a snapshot: repeated `solve` calls on the
same system are bit-identical, and the sweep is unchanged on 1, 2, 4 and 8
rayon threads — impedances, GMRES iteration counts and residuals all —
because GMRES has no random or time-dependent input, the loop-basis
products are indexed reductions in a fixed order, and the pFFT convolution
is a fixed sequence of transforms. Both are asserted in
`fasterhenry/tests/solve.rs`
(`repeated_iterative_solves_are_bit_identical`,
`iterative_impedance_does_not_depend_on_the_thread_count`).

### Reproducing, and the regression guard

```sh
# The table above (one run per size, no criterion sampling):
cargo bench -p fasterhenry --bench scaling -- __report_only__
# With criterion's sampled groups as well (hours at the upper sizes):
cargo bench -p fasterhenry --bench scaling
# Smaller machine, or just the crossover region:
FASTERHENRY_BENCH_MAX_FILAMENTS=10000 cargo bench -p fasterhenry --bench scaling
```

`.github/workflows/bench.yml` does **not** run this suite — the upper
sizes are minutes per sample and gigabytes resident. What CI does run, on
every commit, is the headline as a budgeted test:
`iterative_path_holds_its_scaling_headline` in
`fasterhenry/tests/perf_smoke.rs` solves the 4 760-filament point on the
iterative path within `FASTERHENRY_PERF_ITERATIVE_BUDGET_S` (default
120 s), in under half the dense path's model working set, in fewer than 50
GMRES iterations — the same convention as the 2 000-filament dense smoke
test beside it.

### Scaling plot

The filaments-vs-wall-clock plot #27 asks for, with "the pFFT issue (#24)
marked as the inflection point", now has its data: the two columns above
are the two curves, and the inflection is the ~3 000-filament crossover
(with the default handover at 10 000 marked separately). Drawing it is
#27's own scope; the numbers it needed are here.

## FastHenry example corpus (#74)

Every `.inp` deck shipped in the `examples/` directory of the public
ediloren/FastHenry2 mirror (`363e43e`), run through both tools with
`tools/fasthenry_compare.py`. Only numbers and deck file names are recorded
here; no deck text is in this repository. FastHenry was built natively
(macOS arm64, `-std=gnu89 -fcommon`) outside the tree and run with its
defaults. fasterhenry ran `--fasthenry-compat`, 0.1.1 + #139. Host: Apple M3
Ultra, 28 threads. 2026-10-05.

**Coverage: 3 of 19 decks parse.** (A 20th deck, `tree_sample`, needs a
hierarchy file the corpus doesn't ship, and FastHenry fails on it too.)
Each failure is the *first* error in that deck, so fixing one may expose
another:

| issue | gap | decks stopped |
|---|---|---|
| #141 | whitespace around `=` on `N`/`E`/`.default` lines | msm, trace_over_mesh_new, vias |
| #142 | no conductivity given (FastHenry: copper) | 30pin, broken, holey_gp, simple_gp, together, together_nonuni |
| #143 | `file=NONE` plane with no initial grid (FastHenry: one root cell) | nonuni01, template |
| #144 | `.units meters` and other long spellings | hole, onebargp |
| #145 | `decay_rect` cell ≥ rectangle width (FastHenry: clamps) | 3d_example2, 3d_example2_coarse |
| #146 | `relx`/`rely`/`relz` plane offsets | gpexamp_copper |

Since this run, #141 (PR #150), #142 (#148) and #146 (#149) have merged. A
rerun on `main` at `9f4ecd8`, which has #141 and #142, parses **4 of 19**:
30pin is new, at 3.2e-3 agreement and 7× faster wall-clock. The decks that
#141/#142 unblocked now stop on further gaps, filed as #154 (fractional
`ndec`), #155 (`segwid1`/`segwid2`), #156 (case-insensitive node names) and
#157 (diagonal `contact trace`).

**Where both tools run**, agreement is relative Frobenius error of the port
impedance matrix, ports matched by name, at every frequency both report:

| deck | filaments | FastHenry | fasterhenry (28 thr) | fasterhenry (1 thr) | rel. err, lowest f | worst rel. err |
|---|---|---|---|---|---|---|
| pin-con2seg | 30 | 0.09 s | 0.016 s | 0.011 s | 2.7e-6 (10 Hz) | 1.2e-3 (21.5 MHz) |
| pin-con7 | 735 | 6.2 s | 0.33 s | 4.1 s | 4.2e-7 (1 Hz) | 1.2e-2 (1 THz) |
| pin-connect | 2625 | 5.3 s | 1.1 s | 10.1 s | 4.3e-7 (1 Hz) | 4.3e-7 (one frequency) |

The pattern matches "Speed" above: about 5–19× faster wall-clock on this host
(it is under background load, and the ratio moved by up to 1.5× between
runs), from parallelism; slower per thread at 2.6 k filaments. The worst-case
1.2e-2 at 1 THz on pin-con7 has not been attributed yet. FastHenry ran its
default multipole + GMRES path, and a direct-solve rerun (`-sludecomp
-aoff`) would separate its approximation error from ours.

Rerun once a gap closes:

```bash
python3 tools/fasthenry_compare.py --fasthenry <fasthenry> \
  --fasterhenry target/release/fasterhenry --single-thread <FastHenry2>/examples/*.inp
```

## Deck dialect notes (for the record)

Both tools read the common core (`.units`, `.default` with `w/h/nwinc/
nhinc/sigma`, `N`/`E` keyword lines, `.external` two-node ports,
`.freq`). Differences observed: FastHenry's `G` uses three corner points
(`x1..z3`), `thick=` and `sigma=`, requires `seg1`/`seg2`, and has no
`nx=`/`ny=` (discretization comes from `contact` regions); its default
`Zc.mat` is ASCII text, not MAT v4. fasterhenry reads FastHenry's
corner-point `G` form as well as its own two-corner shorthand
(`nx`/`ny`), accepts `rho`, and takes `sigma` in deck units; see
`docs/fasthenry-compat.md`.
