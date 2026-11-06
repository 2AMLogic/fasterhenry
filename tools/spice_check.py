#!/usr/bin/env python3
"""Check an exported SPICE subcircuit against the JSON impedance in ngspice.

Usage: spice_check.py DECK.json DECK.cir FREQ_HZ [--expect-lines REGEX]...

Instantiates the subcircuit from the `.cir`, ties every `n<i>` to ground,
drives `p<j>` with a 1 A AC current source (one run per port, the others
open), runs a one-point `.ac` in `ngspice -b`, and compares `V(p<i>)` with
`Z_ij` from the JSON (`impedance_ohm[0][i][j]`) in both real and imaginary
parts.  Driving j = 1 alone cannot see the H_i_j (j > 1) sources.  Prints
`ngspice stamp vs JSON: N ports checked` only after every port has matched.
"""
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

RTOL = 1e-6


def main() -> int:
    args = sys.argv[1:]
    json_path, cir_path, freq = Path(args[0]), Path(args[1]), float(args[2])
    patterns = [args[i + 1] for i, a in enumerate(args) if a == "--expect-lines"]
    cir = cir_path.read_text()
    for pat in patterns:
        if not re.search(pat, cir, re.M):
            print(f"::error::{cir_path}: no line matching {pat!r}")
            return 1

    header = re.search(r"^\.subckt\s+(\S+)\s+(.*)$", cir, re.M)
    name, nodes = header.group(1), header.group(2).split()
    ports = [n for n in nodes if n.startswith("p")]
    assert nodes == [x for i in range(len(ports)) for x in (f"p{i + 1}", f"n{i + 1}")]

    z = json.loads(json_path.read_text())["impedance_ohm"][0]
    n = len(ports)
    for j in range(n):  # drive port j (column j of Z); the others stay open
        deck = [f"* stamp check {name}, drive p{j + 1}", cir, f"X1 {' '.join(nodes)} {name}"]
        deck.append(f"I1 0 p{j + 1} DC 0 AC 1")  # 1 A flowing into p<j>
        deck += [f"Vg{i + 1} n{i + 1} 0 0" for i in range(n)]  # n<i> to ground
        deck += [".control", f"ac lin 1 {freq!r} {freq!r}"]
        deck += [f"print v(p{i + 1})" for i in range(n)]
        deck += ["quit", ".endc", ".end"]
        with tempfile.TemporaryDirectory() as tmp:
            tb = Path(tmp) / "tb.cir"
            tb.write_text("\n".join(deck) + "\n")
            out = subprocess.run(["ngspice", "-b", str(tb)], capture_output=True, text=True)
        log = out.stdout + out.stderr
        for i in range(n):
            m = re.search(rf"v\(p{i + 1}\)\s*=\s*([-+0-9.eE]+),\s*([-+0-9.eE]+)", log, re.I)
            if not m:
                print(log)
                print(f"::error::ngspice printed no v(p{i + 1})")
                return 1
            got = complex(float(m.group(1)), float(m.group(2)))
            want = complex(z[i][j]["re"], z[i][j]["im"])
            for part, g, w in (("re", got.real, want.real), ("im", got.imag, want.imag)):
                if abs(g - w) > RTOL * abs(w):
                    print(f"::error::{name}: V(p{i + 1}) {part} {g!r} != Z_{i + 1}{j + 1} {w!r}")
                    return 1
    print(f"ngspice stamp vs JSON: {n} ports checked ({n * n} entries)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
