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

Two gates. First, a **trace-over-plane** fixture (0.2 mm × 35 µm trace, 8 mm
long, 0.5 mm above a 10 × 6 × 0.035 mm plane, vias snapped to the mesh)
against method-of-images references: the DC resistance is the parallel of
trace and plane paths (verified against independent node analysis to seven
digits during development), the loop inductance is grid-converged (2.4 %
from 10 × 3 to 40 × 12), and the RF value is reported against the
rectangle-image + via oracle — a composite fixture (vias, snap offsets,
finite plane) that oracle does not bound at 10 %, so it is not asserted.

Second, the **slot differential** — the AC's PyPEEC gate. A
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
the gate in release mode (assert-not-skipped, like the spiral gate).

The `--fixture plane` / `--fixture plane-solid` modes of
`tools/pypeec_reference.py` regenerate the references
(`--voxel-um 5`, ~10 s each locally).
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
across the *width* of a finite trace; those need an independent two-dimensional
reference. This limitation and near-singular kernel handling are tracked in
issue #30.
