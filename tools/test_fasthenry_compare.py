#!/usr/bin/env python3
"""Unit tests for `tools/fasthenry_compare.py`.

Run with:
    python3 -m unittest tools/test_fasthenry_compare.py
"""

import argparse
import json
import os
import pathlib
import stat
import tempfile
import unittest

import fasthenry_compare as fc

FH_STUB = """#!/usr/bin/env python3
open("Zc.mat", "w").write(
    "Row 1: a to b\\n"
    "Impedance matrix for frequency = 1000 1 x 1\\n"
    "   1.0 +2.0j\\n")
"""

OURS_STUB = """#!/usr/bin/env python3
import json, sys
out = sys.argv[sys.argv.index("--json") + 1]
json.dump({"ports": [{"name": "a/b"}], "frequencies_hz": [1000],
           "impedance_ohm": [[[{"re": 1.0, "im": 2.0}]]],
           "provenance": {}}, open(out, "w"))
"""


def write_exe(path, text):
    path.write_text(text)
    path.chmod(path.stat().st_mode | stat.S_IXUSR)


class CompareTests(unittest.TestCase):
    def test_reordered_ports_align_by_name(self) -> None:
        fh = [(1.0, [[1, 2], [3, 4]])]
        ours = [(1.0, [[4, 3], [2, 1]])]
        errs = fc.compare(["a", "b"], fh, ["b", "a"], ours)
        self.assertEqual(errs, [(1.0, 0.0)])

    def test_name_mismatch_raises(self) -> None:
        with self.assertRaises(ValueError):
            fc.compare(["a", "b"], [(1.0, [[1, 0], [0, 1]])],
                       ["x", "y"], [(1.0, [[1, 0], [0, 1]])])

    def test_duplicate_port_names_raise(self) -> None:
        # With names ['p', 'p'] every entry would be read from z[0][0],
        # hiding a difference confined to the second port.
        fh = [(1.0, [[1, 0], [0, 1]])]
        ours = [(1.0, [[1, 0], [0, 9]])]
        with self.assertRaisesRegex(ValueError, "duplicate"):
            fc.compare(["p", "p"], fh, ["p", "p"], ours)


class ResolveExeTests(unittest.TestCase):
    def test_path_is_made_absolute_and_bare_name_kept(self) -> None:
        self.assertEqual(fc.resolve_exe("target/release/x"),
                         os.path.abspath("target/release/x"))
        self.assertEqual(fc.resolve_exe("fasterhenry"), "fasterhenry")


class OneDeckRelativeBinaryTests(unittest.TestCase):
    def test_relative_binary_paths_survive_temp_cwd(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            root = pathlib.Path(d)
            (root / "bin").mkdir()
            write_exe(root / "bin" / "fh", FH_STUB)
            write_exe(root / "bin" / "ours", OURS_STUB)
            (root / "deck.inp").write_text("title\n")
            old = os.getcwd()
            os.chdir(root)
            try:
                args = argparse.Namespace(
                    fasthenry="bin/fh", fasterhenry="bin/ours",
                    timeout=60, single_thread=False)
                row = fc.one_deck("deck.inp", args)
            finally:
                os.chdir(old)
        self.assertEqual(row["fasthenry"], "ok")
        self.assertEqual(row["fasterhenry"], "ok")
        self.assertNotIn("compare_error", row)
        self.assertEqual(row["err_worst"], (1000.0, 0.0))


if __name__ == "__main__":
    unittest.main()
