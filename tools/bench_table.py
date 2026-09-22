#!/usr/bin/env python3
"""Regenerate the committed criterion results table in `docs/benchmarks.md`.

`cargo bench -p fasterhenry --bench assembly` / `--bench solve` (criterion,
built with the `cargo_bench_support` feature) write machine-readable JSON
under `target/criterion/<group>/<filament-count>/new/` regardless of
whether the `html_reports` feature (plots) is enabled. This script reads
that JSON and replaces the table between the `<!-- bench-table:begin -->`
/ `<!-- bench-table:end -->` markers in the target file with a freshly
generated, dated one — no criterion output is hand-transcribed.

Usage:
    cargo bench -p fasterhenry --bench assembly --bench solve
    python3 tools/bench_table.py

Run from the repository root, or pass `--criterion-dir` /
`--fasterhenry-dir` explicitly. Idempotent: running it again over the same
criterion output (other than the date stamp) reproduces the same table.
"""

from __future__ import annotations

import argparse
import datetime
import json
import pathlib
import re
import subprocess

BEGIN_MARKER = "<!-- bench-table:begin -->"
END_MARKER = "<!-- bench-table:end -->"

# One bench group per stage; see fasterhenry/benches/assembly.rs and
# fasterhenry/benches/solve.rs.
GROUPS = ["assembly", "solve"]


def load_group(criterion_dir: pathlib.Path, group: str) -> dict[int, float]:
    """Filament count -> mean seconds per iteration, for one bench group.

    Criterion's `BenchmarkId::from_parameter(filaments)` (what both bench
    files use) makes the filament count the directory name directly, e.g.
    `target/criterion/assembly/5000/new/estimates.json`.
    """
    results: dict[int, float] = {}
    group_dir = criterion_dir / group
    if not group_dir.is_dir():
        return results
    for entry in sorted(group_dir.iterdir()):
        try:
            filaments = int(entry.name)
        except ValueError:
            continue  # criterion's own "report" aggregate directory, etc.
        estimates_path = entry / "new" / "estimates.json"
        if not estimates_path.is_file():
            continue
        estimates = json.loads(estimates_path.read_text())
        point_estimate_ns = estimates["mean"]["point_estimate"]
        results[filaments] = point_estimate_ns * 1e-9
    return results


def format_seconds(seconds: float) -> str:
    if seconds < 1e-3:
        return f"{seconds * 1e6:.1f} µs"
    if seconds < 1.0:
        return f"{seconds * 1e3:.1f} ms"
    return f"{seconds:.2f} s"


def format_rate(filaments: int, seconds: float) -> str:
    rate = filaments / seconds if seconds > 0 else float("inf")
    if rate >= 1e6:
        return f"{rate / 1e6:.2f} Mfil/s"
    if rate >= 1e3:
        return f"{rate / 1e3:.1f} kfil/s"
    return f"{rate:.1f} fil/s"


def build_table(criterion_dir: pathlib.Path) -> str:
    by_group = {group: load_group(criterion_dir, group) for group in GROUPS}
    filaments = sorted(set().union(*by_group.values())) if any(by_group.values()) else []
    if not filaments:
        raise SystemExit(
            f"no criterion output found under {criterion_dir} for groups "
            f"{GROUPS}; run `cargo bench -p fasterhenry --bench assembly "
            f"--bench solve` first"
        )

    header = "| filaments | assembly (mean) | assembly throughput | solve (mean) | solve throughput |"
    separator = "|---|---|---|---|---|"
    rows = [header, separator]
    for n in filaments:
        assembly_s = by_group["assembly"].get(n)
        solve_s = by_group["solve"].get(n)
        assembly_time = format_seconds(assembly_s) if assembly_s is not None else "—"
        assembly_rate = format_rate(n, assembly_s) if assembly_s is not None else "—"
        solve_time = format_seconds(solve_s) if solve_s is not None else "—"
        solve_rate = format_rate(n, solve_s) if solve_s is not None else "—"
        rows.append(f"| {n:,} | {assembly_time} | {assembly_rate} | {solve_time} | {solve_rate} |")
    return "\n".join(rows)


def rustc_version() -> str:
    try:
        completed = subprocess.run(
            ["rustc", "--version"], capture_output=True, text=True, check=True
        )
        return completed.stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown rustc"


def render_section(table: str, *, date: str, rustc: str, runner: str) -> str:
    caption = (
        f"_Regenerated {date} by `tools/bench_table.py` from a `cargo bench "
        f"-p fasterhenry` run on a GitHub `{runner}` runner ({rustc}); see "
        f"`.github/workflows/bench.yml`. The largest point in the sweep "
        f"defined in `fasterhenry/benches/support.rs` may be capped on CI "
        f"via `FASTERHENRY_BENCH_MAX_FILAMENTS` to fit the job's time "
        f"budget — a local `cargo bench` with no override runs the full "
        f"sweep._"
    )
    return "\n".join([caption, "", table])


def update_file(path: pathlib.Path, section: str) -> str:
    text = path.read_text()
    if BEGIN_MARKER not in text or END_MARKER not in text:
        raise SystemExit(
            f"{path} is missing the {BEGIN_MARKER} / {END_MARKER} markers"
        )
    pattern = re.compile(re.escape(BEGIN_MARKER) + r".*?" + re.escape(END_MARKER), re.DOTALL)
    replacement = f"{BEGIN_MARKER}\n{section}\n{END_MARKER}"
    new_text = pattern.sub(replacement, text, count=1)
    path.write_text(new_text)
    return new_text


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--criterion-dir",
        type=pathlib.Path,
        default=pathlib.Path("target/criterion"),
        help="criterion's output directory (default: target/criterion)",
    )
    parser.add_argument(
        "--out",
        type=pathlib.Path,
        default=pathlib.Path("docs/benchmarks.md"),
        help="Markdown file whose bench-table markers get replaced",
    )
    parser.add_argument(
        "--runner",
        default="ubuntu-latest",
        help="runner label to stamp the table with (default: ubuntu-latest)",
    )
    parser.add_argument(
        "--date",
        default=None,
        help="ISO date to stamp the table with (default: today, UTC)",
    )
    args = parser.parse_args()

    table = build_table(args.criterion_dir)
    date = args.date or datetime.datetime.now(datetime.timezone.utc).date().isoformat()
    section = render_section(table, date=date, rustc=rustc_version(), runner=args.runner)
    update_file(args.out, section)
    print(f"updated {args.out} with the {date} criterion table")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
