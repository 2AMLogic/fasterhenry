#!/usr/bin/env python3
"""Regenerate the landing-page hero figure from a committed deck.

Command (run from the repository root, after `cargo build --release -p fasterhenry-cli`):

    python3 site/gen_hero.py [--bin target/release/fasterhenry]

It runs the engine on `fasterhenry-cli/tests/data/spiral.inp` (the committed
2-turn square spiral: 8 segments of 0.2 mm x 35 um copper) with the sweep
overridden to 1 MHz - 1 GHz at 10 points per decade, then writes

    site/assets/hero-spiral.json   the extracted R(f), L(f) series
    site/assets/hero-spiral.svg    the standalone figure (stdlib only, no plotting library;
                                   drawn by site/charts.py, the same code build.py inlines)

Both outputs are committed so `site/build.py` needs neither Rust nor Python
packages. No FastHenry output is overlaid: the repository commits agreement
numbers against FastHenry (docs/benchmarks.md) but not FastHenry's curves.
"""
import argparse, json, math, subprocess, sys, tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import charts

ROOT = Path(__file__).resolve().parent.parent
DECK = "fasterhenry-cli/tests/data/spiral.inp"
CMD = "fasterhenry {deck} --freq 1e6 1e9 10 --json hero.json"


def run_engine(binary):
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "hero.json"
        subprocess.run(
            [binary, str(ROOT / DECK), "--freq", "1e6", "1e9", "10", "--json", str(out), "--zc-mat", str(Path(tmp) / "Zc.mat")],
            check=True, cwd=tmp, stdout=subprocess.DEVNULL)
        res = json.loads(out.read_text())
    series = []
    for f, mat in zip(res["frequencies_hz"], res["impedance_ohm"]):
        z = mat[0][0]
        series.append({"f_hz": f, "r_ohm": z["re"], "l_h": z["im"] / (2 * math.pi * f)})
    return {"deck": DECK, "command": CMD.format(deck=DECK),
            "fasterhenry_version": res["provenance"]["fasterhenry_version"],
            "filaments": res["provenance"]["counts"]["filaments"], "series": series}


# Fixed axis ranges (mOhm, nH) so the small L change is legible; ticks are round.
R_AXIS = (45, 65, [45, 50, 55, 60, 65])
L_AXIS = (14.2, 14.8, [14.2, 14.4, 14.6, 14.8])
UID = "hero"


def figure(data, layout=charts.WIDE, standalone=False, uid=UID):
    """The hero figure for `data` (the JSON this script commits). Also used by
    build.py to inline it, so the page and the standalone file cannot differ."""
    s = data["series"]
    r, l = s[0]["r_ohm"] * 1e3, s[0]["l_h"] * 1e9
    r1, l1 = s[-1]["r_ohm"] * 1e3, s[-1]["l_h"] * 1e9
    return charts.rl_figure(
        s, "Resistance and inductance of a 2-turn square spiral, 1 MHz to 1 GHz",
        f"Computed by fasterhenry. Resistance rises from {r:.1f} to {r1:.1f} milliohms; "
        f"inductance falls from {l:.1f} to {l1:.1f} nanohenries.",
        uid, layout, standalone, R_AXIS, L_AXIS)


def inline_figure(data):
    """Wide and compact drawings for the landing page (CSS shows one)."""
    return charts.both_layouts(lambda layout, uid: figure(data, layout, False, uid), UID)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bin", default="target/release/fasterhenry")
    args = ap.parse_args()
    data = run_engine(str(Path(args.bin)))
    (ROOT / "site/assets/hero-spiral.json").write_text(json.dumps(data, indent=2) + "\n")
    (ROOT / "site/assets/hero-spiral.svg").write_text(figure(data, standalone=True) + "\n")
    print("wrote site/assets/hero-spiral.{json,svg}", file=sys.stderr)


if __name__ == "__main__":
    main()
