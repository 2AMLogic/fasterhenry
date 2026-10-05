#!/usr/bin/env python3
"""Head-to-head: run .inp decks through FastHenry and fasterhenry, compare
the impedance matrices, and time both.

Usage:
    fasthenry_compare.py --fasthenry PATH --fasterhenry PATH DECK [DECK ...]
                         [--json OUT.json] [--timeout SECONDS] [--single-thread]

Neither the FastHenry binary nor its decks belong in this repository (see
CONTRIBUTING.md); build and keep them elsewhere and point this script at
them. The script reports numbers only: it never echoes deck text or
FastHenry's console output, so its table can be committed as-is. The one
message it does print is fasterhenry's own parse error, which names the
unsupported directive.

Each tool runs in a fresh temporary directory (FastHenry writes `Zc.mat` to
its working directory) with the deck given by absolute path. fasterhenry
runs with `--fasthenry-compat` (line 1 is a title, as in FastHenry decks).

Agreement metric, per frequency: ||Z_ours - Z_fh||_F / ||Z_fh||_F over the
port impedance matrix, ports matched by name. The table reports the worst
frequency and the lowest (resistance-dominated) frequency. Timing is process
wall time; FastHenry is single-threaded, fasterhenry uses every core unless
`--single-thread` adds a RAYON_NUM_THREADS=1 run.
"""

import argparse
import json
import math
import os
import re
import subprocess
import sys
import tempfile
import time

_HEADER = re.compile(r"Impedance matrix for frequency = (\S+) (\d+) x (\d+)")
_ROW = re.compile(r"Row (\d+):\s+([^\s,]+)\s+to\s+([^\s,]+)(?:,\s+port name:\s+(\S+))?")
_NUM = re.compile(r"[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?")


def parse_fasthenry_zc(path):
    """Read FastHenry's ASCII Zc.mat: port names by row, then one complex
    matrix per frequency. Returns (names, [(freq, matrix)])."""
    names = {}
    sweeps = []
    with open(path) as f:
        lines = f.read().splitlines()
    i = 0
    while i < len(lines):
        line = lines[i]
        m = _ROW.search(line)
        if m:
            # An unnamed port is labelled by its node pair; FastHenry
            # lowercases node names, so keys are compared case-folded.
            names[int(m.group(1))] = (m.group(4) or f"{m.group(2)}/{m.group(3)}").lower()
            i += 1
            continue
        m = _HEADER.search(line)
        if m:
            freq, rows, cols = float(m.group(1)), int(m.group(2)), int(m.group(3))
            matrix = []
            for r in range(rows):
                text = lines[i + 1 + r].replace("j", " ")
                vals = [float(v) for v in _NUM.findall(text)]
                if len(vals) != 2 * cols:
                    raise ValueError(f"row {r} at {freq} Hz: expected {2 * cols} numbers")
                matrix.append([complex(vals[2 * c], vals[2 * c + 1]) for c in range(cols)])
            sweeps.append((freq, matrix))
            i += 1 + rows
            continue
        i += 1
    order = [names[k] for k in sorted(names)] if names else []
    return order, sweeps


def parse_fasterhenry_json(path):
    with open(path) as f:
        d = json.load(f)
    names = [(p.get("name") or "").lower() for p in d["ports"]]
    sweeps = [
        (freq, [[complex(e["re"], e["im"]) for e in row] for row in z])
        for freq, z in zip(d["frequencies_hz"], d["impedance_ohm"])
    ]
    return names, sweeps, d.get("provenance", {})


def fasthenry_filaments(log):
    """Filament count FastHenry reports before multipole refinement (the
    physical discretisation, comparable to ours)."""
    m = re.search(r"filaments before multipole refine:\s*(\d+)", log)
    if not m:
        m = re.search(r"Number of filaments:\s*(\d+)", log)
    return int(m.group(1)) if m else None


def run(cmd, cwd, timeout, env=None):
    start = time.perf_counter()
    try:
        p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True,
                           timeout=timeout, env=env, errors="replace")
    except subprocess.TimeoutExpired:
        return None, "", "", time.perf_counter() - start
    return p.returncode, p.stdout, p.stderr, time.perf_counter() - start


def rel_frobenius(ours, ref):
    num = sum(abs(a - b) ** 2 for ra, rb in zip(ours, ref) for a, b in zip(ra, rb))
    den = sum(abs(b) ** 2 for rb in ref for b in rb)
    return math.sqrt(num / den) if den else float("nan")


def compare(fh_names, fh_sweeps, our_names, our_sweeps):
    """Per-frequency relative error, ports aligned by name (FastHenry lists
    them in its own order). Returns [(freq, err)] or raises ValueError."""
    # Never fall back to positional matching: a silent misalignment would
    # publish a wrong agreement number.
    if not fh_names or sorted(fh_names) != sorted(our_names):
        raise ValueError(f"port names differ: {fh_names} vs {our_names}")
    # Duplicate labels cannot identify a port: `.index` would map every
    # repeat to the first one and hide differences in the others.
    if len(set(fh_names)) != len(fh_names):
        raise ValueError(f"duplicate port names: {fh_names}")
    perm = [our_names.index(n) for n in fh_names]
    n = len(perm)
    out = []
    for f, ref in sorted(fh_sweeps, key=lambda s: s[0]):
        if len(ref) != n or any(len(r) != n for r in ref):
            raise ValueError(f"FastHenry matrix at {f:g} Hz is not {n} x {n}")
        # FastHenry prints frequencies to ~6 significant digits; match on
        # relative distance (absolute at 0 Hz, the DC solve).
        near = min(our_sweeps, key=lambda s: abs(s[0] - f))
        if abs(near[0] - f) > 1e-4 * max(abs(f), 1e-12):
            continue
        z = near[1]
        aligned = [[z[perm[r]][perm[c]] for c in range(len(perm))] for r in range(len(perm))]
        out.append((f, rel_frobenius(aligned, ref)))
    if not out:
        raise ValueError("no common frequencies")
    return out


def first_error_line(stderr):
    for line in stderr.splitlines():
        if line.startswith("Error"):
            return line.strip()
    lines = [l for l in stderr.splitlines() if l.strip()]
    return lines[-1].strip() if lines else ""


def resolve_exe(path):
    """Make a filesystem path absolute against the invoking directory, since
    each tool runs in a temporary cwd. A bare command name is left alone so
    PATH lookup still works."""
    return os.path.abspath(path) if os.sep in path else path


def one_deck(deck, args):
    deck = os.path.abspath(deck)
    fasthenry = resolve_exe(args.fasthenry)
    fasterhenry = resolve_exe(args.fasterhenry)
    row = {"deck": os.path.basename(deck)}
    with tempfile.TemporaryDirectory() as fh_dir:
        rc, out, err, secs = run([fasthenry, deck], fh_dir, args.timeout)
        zc = os.path.join(fh_dir, "Zc.mat")
        if rc is None:
            row["fasthenry"] = "timeout"
        elif rc != 0 or not os.path.exists(zc):
            row["fasthenry"] = f"failed (exit {rc})"
        else:
            row["fasthenry"] = "ok"
            row["fasthenry_s"] = secs
            row["fasthenry_filaments"] = fasthenry_filaments(out + err)
            try:
                fh_names, fh_sweeps = parse_fasthenry_zc(zc)
            except (ValueError, IndexError) as e:
                row["fasthenry"] = f"unreadable Zc.mat ({e})"
    with tempfile.TemporaryDirectory() as our_dir:
        js = os.path.join(our_dir, "out.json")
        cmd = [fasterhenry, "--fasthenry-compat", "--json", js, deck]
        rc, _, err, secs = run(cmd, our_dir, args.timeout)
        if rc is None:
            row["fasterhenry"] = "timeout"
        elif rc != 0:
            row["fasterhenry"] = "parse/solve error"
            row["fasterhenry_error"] = first_error_line(err)
        else:
            row["fasterhenry"] = "ok"
            row["fasterhenry_s"] = secs
            our_names, our_sweeps, prov = parse_fasterhenry_json(js)
            row["filaments"] = prov.get("counts", {}).get("filaments")
            row["threads"] = prov.get("timing", {}).get("threads")
            if args.single_thread:
                env = dict(os.environ, RAYON_NUM_THREADS="1")
                rc1, _, _, secs1 = run(cmd, our_dir, args.timeout, env)
                if rc1 == 0:
                    row["fasterhenry_1t_s"] = secs1
    if row.get("fasthenry") == "ok" and row.get("fasterhenry") == "ok":
        try:
            errs = compare(fh_names, fh_sweeps, our_names, our_sweeps)
            worst = max(errs, key=lambda e: e[1])
            row["nfreq"] = len(errs)
            row["err_lowest_f"] = errs[0]
            row["err_worst"] = worst
        except ValueError as e:
            row["compare_error"] = str(e)
    return row


def fmt_s(v):
    return "—" if v is None else (f"{v:.3f}" if v < 10 else f"{v:.1f}")


def table(rows):
    out = ["| deck | filaments (ours / FH) | FastHenry s | fasterhenry s | 1-thread s "
           "| rel. err @ lowest f | worst rel. err (f) | status |",
           "|---|---|---|---|---|---|---|---|"]
    for r in rows:
        status = []
        if r.get("fasthenry") != "ok":
            status.append(f"FastHenry {r.get('fasthenry')}")
        if r.get("fasterhenry") != "ok":
            status.append(f"fasterhenry: {r.get('fasterhenry_error') or r.get('fasterhenry')}")
        if "compare_error" in r:
            status.append(r["compare_error"])
        lo = r.get("err_lowest_f")
        worst = r.get("err_worst")
        out.append("| {} | {} / {} | {} | {} | {} | {} | {} | {} |".format(
            r["deck"],
            r.get("filaments", "—"), r.get("fasthenry_filaments", "—"),
            fmt_s(r.get("fasthenry_s")), fmt_s(r.get("fasterhenry_s")),
            fmt_s(r.get("fasterhenry_1t_s")),
            f"{lo[1]:.1e} ({lo[0]:.3g} Hz)" if lo else "—",
            f"{worst[1]:.1e} ({worst[0]:.3g} Hz)" if worst else "—",
            "; ".join(status) or "ok"))
    return "\n".join(out)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--fasthenry", required=True)
    ap.add_argument("--fasterhenry", required=True)
    ap.add_argument("--json")
    ap.add_argument("--timeout", type=float, default=1800)
    ap.add_argument("--single-thread", action="store_true")
    ap.add_argument("decks", nargs="+")
    args = ap.parse_args()
    rows = []
    for deck in args.decks:
        print(f"... {os.path.basename(deck)}", file=sys.stderr, flush=True)
        rows.append(one_deck(deck, args))
    if args.json:
        with open(args.json, "w") as f:
            json.dump(rows, f, indent=2)
    print(table(rows))


if __name__ == "__main__":
    main()
