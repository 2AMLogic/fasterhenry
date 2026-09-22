#!/usr/bin/env python3
"""Unit tests for `tools/bench_table.py`'s Markdown-formatting logic.

Run with:
    python3 -m unittest tools/test_bench_table.py
"""

import json
import pathlib
import tempfile
import unittest

import bench_table


class FormatSecondsTests(unittest.TestCase):
    def test_microseconds(self) -> None:
        self.assertEqual(bench_table.format_seconds(17e-6), "17.0 µs")

    def test_milliseconds(self) -> None:
        self.assertEqual(bench_table.format_seconds(12.5e-3), "12.5 ms")

    def test_seconds(self) -> None:
        self.assertEqual(bench_table.format_seconds(2.345), "2.35 s")

    def test_boundary_is_milliseconds_not_microseconds(self) -> None:
        # Exactly 1 ms: the µs branch is `< 1e-3`, so this falls to ms.
        self.assertEqual(bench_table.format_seconds(1e-3), "1.0 ms")


class FormatRateTests(unittest.TestCase):
    def test_fil_per_s(self) -> None:
        self.assertEqual(bench_table.format_rate(48, 1.0), "48.0 fil/s")

    def test_kfil_per_s(self) -> None:
        self.assertEqual(bench_table.format_rate(5000, 1.0), "5.0 kfil/s")

    def test_mfil_per_s(self) -> None:
        self.assertEqual(bench_table.format_rate(19_600, 0.001), "19.60 Mfil/s")

    def test_zero_seconds_is_infinite_not_a_crash(self) -> None:
        # inf >= 1e6, so it takes the Mfil/s branch rather than raising.
        self.assertEqual(bench_table.format_rate(48, 0.0), "inf Mfil/s")


class LoadGroupTests(unittest.TestCase):
    def _write_estimates(self, root: pathlib.Path, group: str, filaments: int, point_ns: float) -> None:
        new_dir = root / group / str(filaments) / "new"
        new_dir.mkdir(parents=True)
        (new_dir / "estimates.json").write_text(
            json.dumps({"mean": {"point_estimate": point_ns}})
        )

    def test_reads_mean_point_estimate_as_seconds(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            self._write_estimates(root, "assembly", 48, 17_000.0)
            self._write_estimates(root, "assembly", 5000, 1_500_000_000.0)
            result = bench_table.load_group(root, "assembly")
            self.assertAlmostEqual(result[48], 17e-6)
            self.assertAlmostEqual(result[5000], 1.5)

    def test_ignores_non_numeric_directories(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            self._write_estimates(root, "assembly", 48, 17_000.0)
            (root / "assembly" / "report").mkdir(parents=True)
            result = bench_table.load_group(root, "assembly")
            self.assertEqual(set(result), {48})

    def test_missing_group_returns_empty(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            result = bench_table.load_group(pathlib.Path(tmp), "solve")
            self.assertEqual(result, {})


class BuildTableTests(unittest.TestCase):
    def _write_estimates(self, root: pathlib.Path, group: str, filaments: int, point_ns: float) -> None:
        new_dir = root / group / str(filaments) / "new"
        new_dir.mkdir(parents=True)
        (new_dir / "estimates.json").write_text(
            json.dumps({"mean": {"point_estimate": point_ns}})
        )

    def test_union_of_filament_counts_across_groups(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            self._write_estimates(root, "assembly", 48, 17_000.0)
            self._write_estimates(root, "assembly", 5000, 1.5e9)
            self._write_estimates(root, "solve", 48, 5_000.0)
            table = bench_table.build_table(root)
            lines = table.splitlines()
            # header + separator + two data rows (48, 5000), sorted ascending.
            self.assertEqual(len(lines), 4)
            self.assertIn("48", lines[2])
            self.assertIn("5,000", lines[3])
            # solve has no data at 5000: shown as an em dash, not a crash.
            self.assertIn("—", lines[3])

    def test_raises_when_nothing_found(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(SystemExit):
                bench_table.build_table(pathlib.Path(tmp))


class UpdateFileTests(unittest.TestCase):
    def test_replaces_only_between_markers(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = pathlib.Path(tmp) / "benchmarks.md"
            path.write_text(
                "# Title\n\nbefore\n\n"
                f"{bench_table.BEGIN_MARKER}\nold table\n{bench_table.END_MARKER}"
                "\n\nafter\n"
            )
            new_text = bench_table.update_file(path, "new table")
            self.assertIn("before", new_text)
            self.assertIn("after", new_text)
            self.assertIn("new table", new_text)
            self.assertNotIn("old table", new_text)

    def test_missing_markers_raises(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = pathlib.Path(tmp) / "benchmarks.md"
            path.write_text("# Title\n\nno markers here\n")
            with self.assertRaises(SystemExit):
                bench_table.update_file(path, "new table")


if __name__ == "__main__":
    unittest.main()
