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
| `fasthenry_compare.py` | Head-to-head against a locally built FastHenry: runs each `.inp` deck through both tools, compares the port impedance matrices per frequency (relative Frobenius error, ports matched by name), and times both. Prints numbers, fixed failure categories and exit codes only — never deck text, port names or either tool's output — so its table and JSON can be committed. The FastHenry binary and decks stay outside the repo (`CONTRIBUTING.md`). | Operator, by hand (#74); `docs/benchmarks.md` |
| `test_fasthenry_compare.py` | Unit tests for `fasthenry_compare.py`'s Zc.mat parsing and port matching (`python3 -m unittest test_fasthenry_compare`, run from this directory). | Operator, by hand |
| `fasthenry_oracle_run.py` | The runner behind the FastHenry oracle (below): runs each self-authored deck in a directory through a black-box FastHenry binary in its own fresh run directory and writes exit status, stdout, stderr and produced files (e.g. `Zc.mat`) per deck. | `.github/workflows/fasthenry-oracle.yml` (new); Builders via the workflow |
| `test_fasthenry_oracle_run.py` | Unit tests for the runner's fresh-directory and rejected-deck behaviour, against a stub binary (`python3 -m unittest test_fasthenry_oracle_run`, run from this directory). | Local |
| `test_bench_table.py` | Unit tests for `bench_table.py`'s table formatting (`python3 -m unittest test_bench_table`, run from this directory). | `bench.yml` |

Each script's module docstring — or, for the shell script, its `--help` — has
its full usage.

## FastHenry oracle

Live FastHenry runs (the #158 node-placement probes, #187 measurements) go
through the manual `fasthenry-oracle` workflow, never through a binary on the
dispatch worker (host tools come from the worker spec, not from sweeps).

**What runs.** On an ephemeral GitHub-hosted runner the job downloads the
public FastHenry release tarball (default: `ediloren/FastHenry2` tag `R3.0.1`;
change `source_url` to probe another release), builds it in `RUNNER_TEMP`, and
deletes the source before uploading anything. No agent reads that source; the
artifact holds only run outputs and the build identity.

**Build identity** (`build-identity.txt` in the artifact): release URL, tarball
sha256, whether that sha256 was pinned by the dispatcher or merely recorded,
build date, runner image, and the binary's sha256. Quote it next to any result
you cite. Pin `source_sha256` once the first run has recorded the real value.

**Builder access path.**

1. Write decks yourself under a directory in your branch (the smoke deck is
   `tools/oracle_decks/smoke/single_segment.inp`) and push the branch.
2. `gh workflow run fasthenry-oracle.yml --ref <branch> -f deck_dir=<dir>`
   (the workflow file must be on the default branch for `gh` to dispatch it).
3. `gh run watch <id>` then `gh run download <id> -D <empty dir>`. Per deck you
   get `runs/<deck>/result.json` (exit status, timeout flag, seconds, file
   list), `stdout.txt`, `stderr.txt` and `files/` (e.g. `Zc.mat`). Each deck
   runs in its own fresh directory, so stale output cannot pass as evidence; a
   deck FastHenry rejects shows its non-zero status and stderr and no
   `Zc.mat`.

**Must not.** Read FastHenry source or its distributed example decks, add them
to a branch, or paste them into a deck; submit anything but self-authored
decks; commit raw FastHenry output wholesale. Record retained diagnostics
(numbers, exit codes, fixed categories) in issues and docs, as
`fasthenry_compare.py` does.

**Provenance.** Only self-authored decks go in and only diagnostics from those
runs come out, which is the `CONTRIBUTING.md` clean-room rule: the FastHenry
binary exists only as a black box on a throwaway runner.
