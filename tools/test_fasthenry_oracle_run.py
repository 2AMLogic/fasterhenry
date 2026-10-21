"""Tests for fasthenry_oracle_run.py's fresh-directory behaviour, using a stub
"FastHenry" so no real binary is needed (python3 -m unittest
test_fasthenry_oracle_run, run from this directory)."""

import json
import os
import stat
import tempfile
import unittest

import fasthenry_oracle_run as runner

# Stub: a deck containing "REJECT" fails with status 3 and writes no Zc.mat; any
# other deck writes Zc.mat. A pre-existing marker in the cwd is reported on
# stderr, which would show a run directory being reused.
STUB = """#!/bin/sh
[ -e Zc.mat ] && echo "stale Zc.mat present" >&2
if grep -q REJECT "$1"; then echo "bad deck" >&2; exit 3; fi
echo "Zc for $1" > Zc.mat
echo ran
"""


class OracleRunTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.stub = os.path.join(self.tmp.name, "fasthenry")
        with open(self.stub, "w") as f:
            f.write(STUB)
        os.chmod(self.stub, os.stat(self.stub).st_mode | stat.S_IXUSR)
        self.decks = os.path.join(self.tmp.name, "decks")
        os.makedirs(self.decks)
        self.out = os.path.join(self.tmp.name, "out")

    def deck(self, name, text):
        with open(os.path.join(self.decks, name), "w") as f:
            f.write(text)

    def result(self, name):
        with open(os.path.join(self.out, name, "result.json")) as f:
            return json.load(f)

    def test_runs_use_fresh_directories_and_return_outputs(self):
        self.deck("a.inp", ".end\n")
        self.deck("b.inp", ".end\n")
        rc = runner.main(["--fasthenry", self.stub, "--out", self.out, self.decks])
        self.assertEqual(rc, 0)
        for n in ("a", "b"):
            r = self.result(n)
            self.assertEqual(r["exit_status"], 0)
            self.assertEqual(r["files"], ["Zc.mat"])
            self.assertTrue(os.path.exists(os.path.join(self.out, n, "files", "Zc.mat")))
            with open(os.path.join(self.out, n, "stderr.txt")) as f:
                self.assertNotIn("stale", f.read())

    def test_rejected_deck_reports_status_and_no_zc_mat(self):
        self.deck("ok.inp", ".end\n")
        self.deck("bad.inp", "REJECT\n")
        runner.main(["--fasthenry", self.stub, "--out", self.out, self.decks])
        bad = self.result("bad")
        self.assertEqual(bad["exit_status"], 3)
        self.assertNotIn("Zc.mat", bad["files"])
        with open(os.path.join(self.out, "bad", "stderr.txt")) as f:
            self.assertIn("bad deck", f.read())

    def test_empty_deck_dir_and_dirty_out_are_usage_errors(self):
        self.assertEqual(runner.main(["--fasthenry", self.stub, "--out", self.out, self.decks]), 2)
        self.deck("a.inp", ".end\n")
        os.makedirs(self.out)
        open(os.path.join(self.out, "old"), "w").close()
        self.assertEqual(runner.main(["--fasthenry", self.stub, "--out", self.out, self.decks]), 2)


if __name__ == "__main__":
    unittest.main()
