# tools

Python helpers that sit outside the Rust crates: independent validation
oracles and the benchmark-table generator. None of them ship in the
published crates.

| Script | What it does | Used by |
|---|---|---|
| `pypeec_reference.py` | Builds a validation fixture (the 2-turn spiral, or the ground-plane fixtures via `--fixture`) as a PyPEEC voxel geometry and writes its terminal impedance as JSON — the independent oracle for the spiral and plane validation tests. | CI (`ci.yml`, PyPEEC gates); `docs/validation.md` |
| `cross_section_reference.py` | Independent 2-D skin-effect reference for a rectangular trace: per-unit-length internal impedance vs frequency, as JSON. Oracle for `fasterhenry/tests/width_graded_skin_validation.rs`. | CI (`ci.yml`); `docs/validation.md` |
| `bench_table.py` | Reads criterion's JSON output and regenerates the dated results table between the `bench-table` markers in `docs/benchmarks.md`. | `bench.yml` (manual workflow) |
| `test_bench_table.py` | Unit tests for `bench_table.py`'s table formatting (`python3 -m unittest test_bench_table`, run from this directory). | `bench.yml` |

Each script's module docstring has its full usage.
