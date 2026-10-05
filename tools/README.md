# tools

Helpers that sit outside the Rust crates: independent validation oracles, the
benchmark-table generator, and the third-party license bundle generator. None
of them ship in the published crates.

| Script | What it does | Used by |
|---|---|---|
| `third-party-licenses.sh` | Regenerates `THIRD_PARTY_LICENSES.md` (the Apache-2.0 §4(a) attribution bundle a binary distribution must ship) with cargo-about, pinned by version and checksum. `--check` fails if the committed bundle is stale, if `about.toml` and `deny.toml` disagree, or if a workflow ships an artifact without the bundle. Its template is `third-party-licenses.hbs`. | CI (`ci.yml`, `third-party-licenses` job); `README.md` |
| `pypeec_reference.py` | Builds a validation fixture (the 2-turn spiral, or the ground-plane fixtures via `--fixture`) as a PyPEEC voxel geometry and writes its terminal impedance as JSON — the independent oracle for the spiral and plane validation tests. | CI (`ci.yml`, PyPEEC gates); `docs/validation.md` |
| `cross_section_reference.py` | Independent 2-D skin-effect reference for a rectangular trace: per-unit-length internal impedance vs frequency, as JSON. Oracle for `fasterhenry/tests/width_graded_skin_validation.rs`. | CI (`ci.yml`); `docs/validation.md` |
| `bench_table.py` | Reads criterion's JSON output and regenerates the dated results table between the `bench-table` markers in `docs/benchmarks.md`. | `bench.yml` (manual workflow) |
| `fasthenry_compare.py` | Head-to-head against a locally built FastHenry: runs each `.inp` deck through both tools, compares the port impedance matrices per frequency (relative Frobenius error, ports matched by name), and times both. Prints numbers and fasterhenry's own parse errors only — never deck text or FastHenry output — so its table can be committed. The FastHenry binary and decks stay outside the repo (`CONTRIBUTING.md`). | Operator, by hand (#74); `docs/benchmarks.md` |
| `test_bench_table.py` | Unit tests for `bench_table.py`'s table formatting (`python3 -m unittest test_bench_table`, run from this directory). | `bench.yml` |

Each script's module docstring — or, for the shell script, its `--help` — has
its full usage.
