#!/usr/bin/env python3
"""Run self-authored .inp decks through a black-box FastHenry binary, each in
a fresh run directory, and collect exit status, stdout, stderr and the files
the run produced.

Usage:
    fasthenry_oracle_run.py --fasthenry PATH --out OUT_DIR DECK_DIR
                            [--timeout SECONDS]

For every `*.inp` directly inside DECK_DIR the script makes a new temporary
directory, copies only that deck into it, runs `fasthenry <deck>` there, and
writes into OUT_DIR/<deck name>/:

    result.json   exit status (null on timeout), timed_out, seconds, file list
    stdout.txt, stderr.txt
    files/        everything the run left in its directory (e.g. Zc.mat),
                  minus the copied deck itself

A fresh directory per run means a stale `Zc.mat` from an earlier run or deck
can never be mistaken for this run's output; a deck FastHenry rejects reports
its own non-zero status and stderr and an empty (or partial) `files/`.

Only decks you wrote yourself go in, and only this output comes out
(CONTRIBUTING.md, "clean room"). The script never reads the FastHenry binary's
source, and prints nothing but a one-line summary per deck.

The script exits 0 when every deck was run (whatever FastHenry's own status
was) and 2 on a usage error or when DECK_DIR holds no decks.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time


def run_deck(fasthenry, deck, out_root, timeout):
    name = os.path.splitext(os.path.basename(deck))[0]
    out = os.path.join(out_root, name)
    os.makedirs(out)
    with tempfile.TemporaryDirectory() as run_dir:
        local = os.path.join(run_dir, os.path.basename(deck))
        shutil.copyfile(deck, local)
        start = time.monotonic()
        timed_out = False
        try:
            proc = subprocess.run(
                [fasthenry, os.path.basename(deck)],
                cwd=run_dir,
                capture_output=True,
                text=True,
                errors="replace",
                timeout=timeout,
            )
            rc, stdout, stderr = proc.returncode, proc.stdout, proc.stderr
        except subprocess.TimeoutExpired as e:
            timed_out = True
            rc = None
            stdout = e.stdout.decode(errors="replace") if isinstance(e.stdout, bytes) else (e.stdout or "")
            stderr = e.stderr.decode(errors="replace") if isinstance(e.stderr, bytes) else (e.stderr or "")
        secs = time.monotonic() - start
        os.remove(local)
        produced = sorted(os.listdir(run_dir))
        files_out = os.path.join(out, "files")
        os.makedirs(files_out)
        for f in produced:
            src = os.path.join(run_dir, f)
            if os.path.isfile(src):
                shutil.copyfile(src, os.path.join(files_out, f))
    with open(os.path.join(out, "stdout.txt"), "w") as fh:
        fh.write(stdout)
    with open(os.path.join(out, "stderr.txt"), "w") as fh:
        fh.write(stderr)
    result = {
        "deck": os.path.basename(deck),
        "exit_status": rc,
        "timed_out": timed_out,
        "seconds": round(secs, 3),
        "files": produced,
    }
    with open(os.path.join(out, "result.json"), "w") as fh:
        json.dump(result, fh, indent=2)
        fh.write("\n")
    return result


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--fasthenry", required=True, help="path to the FastHenry binary")
    ap.add_argument("--out", required=True, help="output directory (must not exist or be empty)")
    ap.add_argument("--timeout", type=float, default=600.0)
    ap.add_argument("deck_dir")
    args = ap.parse_args(argv)

    decks = sorted(
        os.path.join(args.deck_dir, f)
        for f in os.listdir(args.deck_dir)
        if f.endswith(".inp") and os.path.isfile(os.path.join(args.deck_dir, f))
    )
    if not decks:
        print(f"no .inp decks in {args.deck_dir}", file=sys.stderr)
        return 2
    if os.path.exists(args.out) and os.listdir(args.out):
        print(f"--out {args.out} is not empty", file=sys.stderr)
        return 2
    os.makedirs(args.out, exist_ok=True)
    exe = os.path.abspath(args.fasthenry) if os.sep in args.fasthenry else args.fasthenry
    for deck in decks:
        r = run_deck(exe, deck, os.path.abspath(args.out), args.timeout)
        print(f"{r['deck']}: exit={r['exit_status']} timed_out={r['timed_out']} files={len(r['files'])}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
