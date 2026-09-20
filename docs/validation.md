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
