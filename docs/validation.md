# Validation

The measured agreement between `fasterhenry` and its independent
references, for the M0 milestone. Regenerate everything with:

```bash
python3 -m venv /tmp/pypeec-venv && /tmp/pypeec-venv/bin/pip install pypeec==5.8.0
/tmp/pypeec-venv/bin/python tools/pypeec_reference.py --out tools/pypeec_reference.json
cargo test -p fasterhenry --test spiral_validation -- --nocapture
```

CI reruns the same steps on the Linux legs (the reference is regenerated
there, never committed) and asserts the test did not take its skip path.

### How CI asserts a gate actually ran

Every PyPEEC- or reference-backed gate in this document prints a **positive
marker on stdout** once its comparison has completed and passed —
`SPIRAL GATE PASSED`, `SLOT GATE PASSED`, `CONTACT GATE PASSED`,
`CROSS-SECTION GATE PASSED` — and the CI step counts occurrences of that
marker against an exact expected number. Two details make that necessary
rather than decorative (#59):

- **The reference path passed to a test must be absolute.** `cargo test`
  runs an integration test with its *package* directory as the cwd, so a
  workspace-relative `target/pypeec_plane.json` resolves under
  `fasterhenry/` and never exists. CI passes
  `"$GITHUB_WORKSPACE/target/..."` for every `FASTERHENRY_*_REFERENCE`
  variable.
- **Skip reasons go to stderr.** They are `eprintln!`, so a step that pipes
  only stdout into its log (`cargo test … | tee log`) and then greps for
  `SKIPPED` searches text the message never entered — a check that cannot
  fail. CI folds stderr in with `2>&1 | tee` *and* counts the positive
  marker, so a skip fails the step under either mechanism.

Between them, the relative path and the invisible skip meant the spiral and
slot gates took their "reference absent" path on every CI run from their
introduction until #59, while the job stayed green.

## Fixture

The 2-turn square spiral klayout-tools validates its MoM PEEC against
(`docs/design/mom-validation.md` §5 in that repository), so numbers are
comparable across the two projects: starting side 60 µm, 15 µm pitch
growth per turn — segment centreline lengths 60, 60, 75, 75, 90, 90,
105, 105 µm — 2 × 2 µm copper cross-section (σ = 5.8e7 S/m), port closing
the spiral from its end back to its start. The engine uses 4 × 4
filaments per segment; klayout-tools used 1 µm filament size (2 × 2).

## Measured (2026-09-20, `fasterhenry` post-#18, pypeec 5.8.0)

| quantity | fasterhenry | PyPEEC 5.8.0 | Greenhouse | Mohan m-W | klt MoM PEEC (reference) |
|---|---|---|---|---|---|
| L (1 kHz) | **0.637460 nH** | 0.634505 nH | 0.638215 nH | 0.515 nH | 0.637718 nH |
| R (DC) | 2.844828 Ω | 2.816871 Ω | — | — | — |

| comparison | stated tolerance | measured |
|---|---|---|
| L vs PyPEEC | 2 % | **0.47 %** |
| L vs Greenhouse | 5 % | **0.12 %** |
| R vs PyPEEC (sanity band) | 25 % | 0.99 % |
| R vs analytic series sum | 0.5 % | < 0.1 % |

Mohan's modified-Wheeler estimate is reported but not asserted: at
−19 % it sits at the edge of its ±20 % accuracy class for a spiral this
small, which is the expected behaviour of a current-sheet expression
here, not a defect of the engine.

## Oracles

- **PyPEEC 5.8.0** (`pip install pypeec==5.8.0`, MPL-2.0, Thomas Guillod /
  Dartmouth): a fully independent voxel quasi-magnetostatic solve.
  `tools/pypeec_reference.py` builds the spiral as a 1 µm voxel structure
  (2 648 conductor voxels, grid 107 × 107 × 2), drives a 1 A current
  source across the terminals at DC and 1 kHz, and writes the terminal
  impedance as JSON. The difference against the filament model is dominated
  by the voxelization of the corners and the terminal pads.
- **Greenhouse segment summation** (in `tests/spiral_validation.rs`,
  sharing no code with the production kernels): Rosa/Greenhouse
  rectangular self terms plus the exact parallel-filament mutual — the
  closed form of the double line integral — signed by relative winding
  direction; perpendicular segments do not couple. Verified against
  numerical quadrature and against klayout-tools' independent
  implementation of the same method (which measures 0.638065 nH for this
  fixture, 0.02 % from ours).
- **Mohan modified-Wheeler** (same test file, reported only):
  `L = K₁ µ₀ n² d_avg / (1 + K₂ ρ)`, square constants `K₁ = 2.34`,
  `K₂ = 2.75`, `d_out`/`d_in` measured from the metal extents.
- **Analytic DC resistance**: the series sum `l_k / (σ w t)` over the
  eight segments.


## Ground planes (issue #22)

Plane validation is gated by the independent **PyPEEC slot differential**.
The **trace-over-plane** fixture supplies convergence and coarse sanity
checks; the earlier 5–10 % method-of-images accuracy claim is retired
(issue #37).

The trace-over-plane fixture uses a 0.2 mm × 35 µm trace, 8 mm long,
0.5 mm above a 10 × 6 × 0.035 mm plane, with vias snapped to the mesh.
Its DC resistance is bounded between 3 mΩ and the trace-alone resistance,
and low-frequency loop inductance is checked for grid convergence
(2.4 % measured from 10 × 3 to 40 × 12, bound 5 %) and LF/RF ordering.
`trace_over_plane_rf_image_estimate_is_a_coarse_sanity_check` reports RF
inductance against the rectangle-image + via estimate: 3.3051 nH versus
5.2397 nH at 1 GHz (36.92 % relative deviation), with the existing
**45 % coarse sanity bound** retained.
The vias, snap offsets, finite plane, and single-filament plane bars are
outside what that estimate bounds at 5–10 %; this diagnostic does not
establish an image-oracle accuracy guarantee.

The separate **slot-differential** fixture is a
1.2 × 0.8 × 0.02 mm copper plane with a 0.2 × 0.64 mm slot, driven across
its short ends; comparing `L(slotted) − L(solid)` cancels the differently
modelled drive contacts on both sides, isolating the slot's physics.

| quantity | fasterhenry (96 × 64) | PyPEEC 5 µm voxels | rel |
|---|---|---|---|
| ΔL (slot) | 0.173660 nH | 0.165443 nH | **4.97 %** (bound 5.5 %) |
| ΔR (slot) | 2.280 mΩ | 2.222 mΩ | 2.6 % (bound 15 %) |

Convergence, measured: fasterhenry's ΔL is 0.184 / 0.174 / 0.172 nH at
48 × 32 / 96 × 64 / 192 × 128 (cell-centre mesh, ~1/n); PyPEEC moves 0.2 %
from 5 µm to 2.5 µm voxels. The ~4 % residual at full convergence is the
two discretizations' method bias; the bound leaves headroom rather than
encoding today's number. CI regenerates both PyPEEC references and runs
the gate in release mode, asserting it printed its `SLOT GATE PASSED`
marker exactly once (the positive assertion described above).

The `--fixture plane` / `--fixture plane-solid` modes of
`tools/pypeec_reference.py` regenerate the references
(`--voxel-um 5`, ~10 s each locally).

The gate is the most expensive one in CI: the differential is two 96 × 64
solves plus a 48 × 32 pair for the grid-sensitivity check — 12 128 plane
bars on the reference grid — measured at **4 min 22 s wall and 2.5 GB peak
RSS**, essentially single-threaded, in release on an 8-core Linux box under
load (2026-09-25). The `rust` job's `timeout-minutes` carries the headroom
for it.

### Graded contact regions (issue #36, measured 2026-09-25)

A uniform cell-centre plane converges like `1/n` (the ΔL series above), so
resolving a via landing by refining everywhere is ruinous. A
`ContactRegion` refines *locally* instead: inside the rectangle the mesh is
uniform at the fine cell, and outside it each cell grows by a geometric
ratio until it reaches the background cell.

The **contact-dominated fixture** is the same 1.2 × 0.8 × 0.02 mm sheet,
driven between two 25 µm via landings *inside* it rather than across its
short ends, so the port impedance is dominated by the current crowding
under the contacts. It is meshed three ways: a 100 µm background graded to
25 µm under each landing (`ratio = 2`, 5 fine cells per axis per region),
a fully uniform 25 µm plane, and the bare 100 µm background. Landings
0.825 mm apart:

| mesh | plane bars | L (1 kHz) | R (DC) | rel L | rel R |
|---|---|---|---|---|---|
| graded (100 → 25 µm) | **580** | 0.274043 nH | 1.809 mΩ | **0.10 %** | **0.80 %** |
| uniform 100 µm | 172 | 0.308044 nH | 1.520 mΩ | 12.52 % | 15.30 % |
| uniform 25 µm (reference) | 2 992 | 0.273774 nH | 1.794 mΩ | — | — |

Grading reaches the fully-fine answer inside 1 % on both L and R at
**5.2× fewer filaments** (2 992 → 580), and is 125× closer than the
background mesh it is built on. Bounds: 1 % on each, ≥ 4× saving.

The same fixture carries an independent **PyPEEC separation
differential**: `L(far) − L(near)` for landings 0.825 mm and 0.225 mm
apart, which cancels the pad terms the two solvers model differently
(a cell-centre node here, a 25 µm voxel pad there).

| quantity | fasterhenry (graded, 580 bars) | PyPEEC 5 µm voxels | rel |
|---|---|---|---|
| ΔL (separation) | 0.224802 nH | 0.224530 nH | **0.12 %** (bound 3 %) |
| ΔR (separation) | 0.7180 mΩ | 0.7129 mΩ | 0.72 % (bound 15 %) |

Grading moves that differential by 0.06 % against the fully uniform 25 µm
mesh, so the comparison measures physics rather than the mesh. The
`--fixture contact` / `--fixture contact-near` modes of
`tools/pypeec_reference.py` regenerate the references (`--voxel-um 5`,
~3 min each locally).

Both contact gates are **release-mode**: the fully-fine reference is 2 992
filaments, ~12 s optimized and ~10 min unoptimized, so the first skips
itself in a debug build. Each prints `CONTACT GATE PASSED` on success and
CI asserts it counted two — the same positive assertion every gate here now
carries (see "How CI asserts a gate actually ran" above).

## Surface-graded filaments (issue #23, measured 2026-09-21)

A 400 mm × 40 mm × 200 µm copper trace (`σ = 5.8e7 S/m`) uses one
filament across its width and 14 across its thickness at a 2:1 inward
ratio. Against the one-dimensional slab solution
`Z_int = l k coth(kt/2)/(2 σ w)`, `k = (1+j)/δ`, its resistance errors
at `t/δ = 5, 10, 20` are `+0.17 %, −1.03 %, −1.14 %`. The analytic
formula models internal impedance; the solver's imaginary port impedance
includes the much larger external inductance and is compared through
frequency-dependent differences. Its extracted internal inductance is within
14.3 % of the slab expression over `t/δ = 2 … 20`, including the high
frequency `L_int ∝ δ` transition. The DC partial inductance agrees with the
Rosa/Grover rectangular-bar closed form to better than `1e-4` relative.

On a separate 200 µm square trace, graded 4 × 4 at 10:1 differs from
uniform 16 × 16 by `0.097 %, 0.002 %, <0.001 %` in complex impedance at
`δ/t = 1, 3, 10`, using 16 instead of 256 filaments. The Criterion
`skin_resistance_accuracy` bench reports resistance error against a
128-layer uniform reference together with assembly-and-solve timing for
4, 8, 12 and 16 layers.

The slab is a one-dimensional reference. It does not certify grids graded
across the *width* of a finite trace; those are validated against the
independent two-dimensional reference in the next section. Near-singular
kernel handling is tracked in issue #30.

## Width-graded filaments (issue #32, measured 2026-09-22)

The slab above has no edges, so it cannot tell whether a grid graded across
the *width* resolves the current crowding into a finite trace's edges. That
needs a two-dimensional oracle. The infinite-width slab is not used as one
here.

### Reference: `tools/cross_section_reference.py`

The reference is a finite-width cross-section solve that shares no code with
the crate. It needs only numpy.

- **Geometry and material.** An infinitely long, straight, isolated copper
  bar, 5 mm × 200 µm (`w/t = 25`), `σ = 5.8e7 S/m`, `µ = µ0`. Nothing else
  is present: no ground plane and no return conductor.
- **Drive.** A total current `I` along the bar, with a uniform voltage drop
  per unit length `V'`. The output is the internal impedance per unit length,
  `z' = V'/I`.
- **Equations.** The exact magneto-quasi-static problem, neglecting
  displacement current as FastHenry does: `J/σ + jωA = V'` inside the
  conductor, with `A = −(µ0/2π) ∬ J ln|r − r'|`. That is the diffusion
  equation `∇²J = jωµ0σJ` inside plus the open-space exterior field, so no
  outer boundary is imposed.
- **Discretization.** Piecewise-constant `J` on a tensor mesh of one symmetry
  quadrant, with Galerkin cell averages of `ln r`. Near cell pairs use the
  closed-form fourth antiderivative (self-tested against Gauss quadrature and
  against Maxwell's geometric mean distance of a square, `0.44705 a`). Far
  pairs use a second-order moment expansion. Faces follow a blended sine map
  toward the edges and faces, deliberately not fasterhenry's geometric
  progression. Each mesh's uniform-current inductance matches the whole
  rectangle's closed form to `< 1e-8`.
- **Convergence.** Four doubled meshes, from 16 × 4 to 128 × 32 cells per
  quadrant (16 384 cells in total). Every quantity is Richardson-extrapolated.
  The generator refuses to write a reference unless the observed order over
  the last three levels is between 1.6 and 2.5; it measured 1.99–2.07. The
  recorded uncertainty is the whole last refinement step, about 3× the
  applied correction.
- **Quantities.** `R'(f)` and `ΔL'(f) = X'(f)/ω − L'_DC`. `ΔL'` is the
  frequency-dependent drop of the internal inductance from its
  uniform-current value. Both are independent of the arbitrary reference
  length of the 2-D log kernel.

To regenerate it (about 35 s of CPU):

```bash
python3 tools/cross_section_reference.py --out target/cross_section_reference.json
```

| `t/δ` | f | `R'` (Ω/m) | ± | `ΔL'` (H/m) | ± |
|---|---|---|---|---|---|
| 0.3 | 9.826 kHz | 1.752614e-2 | <0.001 % | −1.184873e-9 | 0.033 % |
| 1 | 109.2 kHz | 2.316251e-2 | 0.004 % | −1.707837e-8 | 0.010 % |
| 3 | 982.6 kHz | 4.212494e-2 | 0.073 % | −2.490836e-8 | 0.022 % |
| 10 | 10.92 MHz | 1.421672e-1 | 0.490 % | −2.923833e-8 | 0.045 % |

(`R'_DC = 1.7241e-2 Ω/m`.)

### Comparison: `fasterhenry/tests/width_graded_skin_validation.rs`

The 3-D solve produces the same per-unit-length quantity from a 1-segment
trace by differencing two lengths: `z' = (Z(200 mm) − Z(100 mm)) / 100 mm`.
The common `l ln l` and end-correction parts of the partial inductances
cancel in that difference. Doubling the lengths moves `R'` by at most
0.008 % and `ΔL'` by at most 0.042 %; a reference-free test asserts that
this stays below 0.1 %. `L'_DC` is read from the same grid at 1 Hz.

Signed errors against the reference, in %:

| grid | `t/δ` = 0.3: `R'` / `ΔL'` | 1 | 3 | 10 |
|---|---|---|---|---|
| `uniform(36, 10)` | −0.01 / +0.49 | −0.46 / +0.39 | −8.70 / +2.14 | **−28.24** / +5.66 |
| `graded(36, 10, 1.2)` | −0.01 / +1.00 | −0.01 / +0.12 | −0.56 / +0.18 | −4.00 / +0.35 |
| `graded(18, 5, 1.44)` | −0.02 / +3.74 | +0.00 / +0.39 | −2.24 / +0.59 | −17.98 / +1.42 |
| `graded(72, 20, 1.0954)` | −0.00 / +0.29 | −0.01 / +0.06 | −0.15 / +0.07 | −0.98 / +0.11 |
| `graded(36, 10, 1.5)` | −0.02 / +3.89 | +0.11 / +0.28 | −0.23 / +0.19 | −0.23 / +0.14 |
| `SkinDepthGrading(f, 0.5, 2)` (7, 20, 52, 112 filaments) | −0.11 / **+13.39** | −0.10 / +1.34 | −2.31 / +0.88 | +0.27 / +0.30 |

**The `nw ≥ 4`, ratio `≥ 1.2` case from the issue.** At 36 × 10, width
grading at 1.2 raises `R'` at `t/δ = 10` by a factor of 1.34 over the uniform
grid. That matches the 6.65 mΩ vs 5.00 mΩ (1.33) first reported. The
reference shows the **uniform** grid is the inaccurate one: its 139 µm edge
cells cannot resolve the edge crowding, so it reads `R'` 28 % low. The graded
grid is 4 % low. Width grading therefore needs no fix.

**Convergence-based accuracy bound (asserted).** The refinement family
`graded(18, 5, 1.44) → (36, 10, 1.2) → (72, 20, √1.2)` halves every cell while
keeping the grading profile fixed. At every frequency, the test asserts three
things for both quantities:

- Each refinement shrinks every error larger than the reference's resolution.
- The remaining error is at most the change from the previous level. So a
  user can bound the error of a width-graded grid without any reference, by
  refining once and taking the change.
- On the finest grid, `R'` is within 1.5 % (measured ≤ 0.98 %) and `ΔL'` is
  within 0.5 % (measured ≤ 0.29 %).

**Supported regime (documented and asserted).** Grading is not free:

- **Steep width grading at weak skin effect.** A steep width ratio coarsens
  the middle of the trace. At `t/δ = 0.3`, the current redistributes smoothly
  across the whole width, and `graded(36, 10, 1.5)`, with 0.83 mm (w/6)
  middle cells, reads `ΔL'` 3.9 % high. From `t/δ = 1` up, and on `R'`
  everywhere, it is within 0.3 %. Where `δ > t`, use a mild width ratio
  (≤ 1.2 at this cell count) or refine.
- **The skin-depth-adaptive grid.** `SkinDepthGrading` sizes cells against
  `δ` alone. At `t/δ = 0.3` it gives 7 filaments and `ΔL'` is 13 % high. That
  is a small absolute error, because the internal inductance there is ~0.1 %
  of the trace's total. It holds `R'` within 2.4 % at every frequency, and
  `ΔL'` within 1.4 % from `t/δ = 1`.

Both limits are asserted as bounds, so a change in them is noticed.

**CI.** CI regenerates the reference on `ubuntu-latest` and runs the
comparison in release mode. The step fails unless all four reference-backed
tests print their `CROSS-SECTION GATE PASSED` line. It counts that positive
marker instead of only grepping for `SKIPPED`. Without a reference, the
ordinary workspace test prints `SKIPPED` and returns before any solve.

```bash
FASTERHENRY_CROSS_SECTION_REFERENCE=target/cross_section_reference.json \
  cargo test --release -p fasterhenry --test width_graded_skin_validation -- --nocapture
```
