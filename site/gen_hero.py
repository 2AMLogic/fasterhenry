#!/usr/bin/env python3
"""Regenerate the landing-page hero figure from a committed deck.

Command (run from the repository root, after `cargo build --release -p fasterhenry-cli`):

    python3 site/gen_hero.py [--bin target/release/fasterhenry]

It runs the engine on `fasterhenry-cli/tests/data/spiral.inp` (the committed
2-turn square spiral: 8 segments of 0.2 mm x 35 um copper) with the sweep
overridden to 1 MHz - 1 GHz at 10 points per decade, then writes

    site/assets/hero-spiral.json   the extracted R(f), L(f) series
    site/assets/hero-spiral.svg    the figure (stdlib only, no plotting library)

Both outputs are committed so `site/build.py` needs neither Rust nor Python
packages. No FastHenry output is overlaid: the repository commits agreement
numbers against FastHenry (docs/benchmarks.md) but not FastHenry's curves.
"""
import argparse, json, math, subprocess, sys, tempfile
from pathlib import Path

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


def panel(x0, y0, w, h, xs, ys, ylo, yhi, yticks, ylabel, fmt, color):
    lo, hi = math.log10(xs[0]), math.log10(xs[-1])
    px = lambda f: x0 + (math.log10(f) - lo) / (hi - lo) * w
    py = lambda v: y0 + h - (v - ylo) / (yhi - ylo) * h
    o = [f'<rect x="{x0}" y="{y0}" width="{w}" height="{h}" fill="none" stroke="#c9ced6"/>']
    for t in yticks:
        o.append(f'<line x1="{x0}" x2="{x0+w}" y1="{py(t):.1f}" y2="{py(t):.1f}" stroke="#e6e9ee"/>')
        o.append(f'<text x="{x0-8}" y="{py(t)+4:.1f}" text-anchor="end">{fmt(t)}</text>')
    for d in range(6, 10):
        o.append(f'<line x1="{px(10**d):.1f}" x2="{px(10**d):.1f}" y1="{y0}" y2="{y0+h}" stroke="#e6e9ee"/>')
    pts = " ".join(f"{px(x):.1f},{py(y):.1f}" for x, y in zip(xs, ys))
    o.append(f'<polyline points="{pts}" fill="none" stroke="{color}" stroke-width="2.5" stroke-linejoin="round"/>')
    o.append(f'<text x="{x0-52}" y="{y0+h/2}" text-anchor="middle" transform="rotate(-90 {x0-52} {y0+h/2})">{ylabel}</text>')
    return o, px


def svg(data):
    s = data["series"]
    f = [p["f_hz"] for p in s]
    r = [p["r_ohm"] * 1e3 for p in s]   # mOhm
    l = [p["l_h"] * 1e9 for p in s]     # nH
    W, H, x0, w = 720, 470, 84, 600
    rl, rh = 45, 65
    ll, lh = 14.2, 14.8
    o = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" role="img" aria-labelledby="t d" font-family="system-ui,sans-serif" font-size="13" fill="#2b313b">',
         '<title id="t">Resistance and inductance of a 2-turn square spiral, 1 MHz to 1 GHz</title>',
         f'<desc id="d">Computed by fasterhenry. Resistance rises from {r[0]:.1f} to {r[-1]:.1f} milliohms; inductance falls from {l[0]:.1f} to {l[-1]:.1f} nanohenries.</desc>',
         f'<rect width="{W}" height="{H}" fill="#ffffff"/>']
    a, _ = panel(x0, 34, w, 170, f, r, rl, rh, [45, 50, 55, 60, 65], "R (mΩ)", lambda v: f"{v:g}", "#b4400a")
    b, px = panel(x0, 244, w, 170, f, l, ll, lh, [14.2, 14.4, 14.6, 14.8], "L (nH)", lambda v: f"{v:g}", "#0b5cad")
    o += a + b
    o.append('<text x="84" y="22" font-weight="600">R(f)</text><text x="84" y="232" font-weight="600">L(f)</text>')
    for d, lab in zip(range(6, 10), ["1 MHz", "10 MHz", "100 MHz", "1 GHz"]):
        o.append(f'<text x="{px(10**d):.1f}" y="434" text-anchor="middle">{lab}</text>')
    o.append(f'<text x="{x0+w/2}" y="460" text-anchor="middle">frequency</text></svg>')
    return "\n".join(o)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bin", default="target/release/fasterhenry")
    args = ap.parse_args()
    data = run_engine(str(Path(args.bin)))
    (ROOT / "site/assets/hero-spiral.json").write_text(json.dumps(data, indent=2) + "\n")
    (ROOT / "site/assets/hero-spiral.svg").write_text(svg(data) + "\n")
    print("wrote site/assets/hero-spiral.{json,svg}", file=sys.stderr)


if __name__ == "__main__":
    main()
