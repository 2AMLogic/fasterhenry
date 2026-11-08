#!/usr/bin/env python3
"""Regenerate the site's Gallery data and figures from the committed decks.

Command (repository root, after `cargo build --release -p fasterhenry-cli`):

    python3 tools/gallery_figures.py [--bin target/release/fasterhenry]

For every deck in DECKS it runs the engine with the sweep overridden to
1 MHz - 1 GHz at 10 points per decade, times the process (median wall clock of
RUNS runs), and writes under site/assets/gallery/

    gallery.json    one entry per deck: command, wall time, counts, R(f)/L(f)
                    of port 1's self-impedance, or the error if the deck failed
    <id>.svg        the R/L vs frequency figure (stdlib only, no plotting lib)

A deck that does not run is recorded with status "failed" and its error text
(the page shows it) and the script exits non-zero; it is never dropped
silently. No FastHenry curve is overlaid: the repository commits agreement
numbers against FastHenry (docs/benchmarks.md), not FastHenry's curves.

Outputs are committed so `site/build.py` needs neither Rust nor Python
packages. Wall times are of the machine that ran this script, recorded in
gallery.json under "machine".
"""
import argparse, json, math, os, platform, statistics, subprocess, sys, tempfile, time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DATA = "fasterhenry-cli/tests/data"
DECKS = ["spiral", "twobar", "plane_fasthenry", "plane_extension", "plane_decay_rect", "plane_decay_extension"]
OUT = ROOT / "site/assets/gallery"
FREQ = ("1e6", "1e9", "10")
RUNS = 3
CMD = "fasterhenry {deck} --freq 1e6 1e9 10 --json out.json"


def describe(deck_text, limit=320):
    """Leading `*` comment block of a deck, joined, cut at a sentence end."""
    lines = []
    for ln in deck_text.splitlines():
        if not ln.startswith("*"):
            break
        lines.append(ln[1:].strip())
    text = " ".join(l for l in lines if l)
    if len(text) > limit:
        cut = text[:limit]
        end = max(cut.rfind(". "), cut.rfind(".)"))
        text = cut[: end + 1] if end > 40 else cut.rsplit(" ", 1)[0] + "…"
    return text


def deck_title(deck_text, fallback):
    for ln in deck_text.splitlines():
        if ln.lower().startswith(".title"):
            return ln[6:].strip() or fallback
    return fallback


def series_from_result(res, port=0):
    """R(f) [ohm] and L(f) [H] of Z[port][port]; L = Im(Z) / (2 pi f)."""
    out = []
    for f, mat in zip(res["frequencies_hz"], res["impedance_ohm"]):
        z = mat[port][port]
        out.append({"f_hz": f, "r_ohm": z["re"], "l_h": z["im"] / (2 * math.pi * f)})
    return out


def run_deck(binary, name):
    deck = ROOT / DATA / f"{name}.inp"
    times, res = [], None
    with tempfile.TemporaryDirectory() as tmp:
        for _ in range(RUNS):
            out = Path(tmp) / "out.json"
            t0 = time.perf_counter()
            proc = subprocess.run(
                [binary, str(deck), "--freq", *FREQ, "--json", str(out), "--zc-mat", str(Path(tmp) / "Zc.mat")],
                cwd=tmp, capture_output=True, text=True)
            times.append(time.perf_counter() - t0)
            if proc.returncode != 0:
                return None, (proc.stderr.strip() or f"exit status {proc.returncode}")[-600:], times
            res = json.loads(out.read_text())
    return res, None, times


def entry(binary, name):
    text = (ROOT / DATA / f"{name}.inp").read_text()
    e = {"id": name, "title": deck_title(text, name), "description": describe(text),
         "deck": f"{DATA}/{name}.inp", "command": CMD.format(deck=f"{DATA}/{name}.inp"),
         "overlay": None}
    res, err, times = run_deck(binary, name)
    if res is None:
        e.update(status="failed", error=err)
        return e
    prov = res["provenance"]
    e.update(status="ok", wall_time_s=statistics.median(times), wall_time_runs=RUNS,
             fasterhenry_version=prov["fasterhenry_version"], counts=prov["counts"],
             ports=len(res["ports"]), series=series_from_result(res))
    return e


def nice_ticks(lo, hi, n=4):
    """About n round tick values spanning [lo, hi] (hi > lo)."""
    raw = (hi - lo) / n
    mag = 10 ** math.floor(math.log10(raw))
    step = next(m * mag for m in (1, 2, 2.5, 5, 10) if m * mag >= raw)
    first = math.ceil(lo / step - 1e-9)
    ticks = []
    while first * step <= hi + step * 1e-9:
        ticks.append(round(first * step, 12))
        first += 1
    return ticks


def axis_range(vals):
    lo, hi = min(vals), max(vals)
    if hi - lo <= 1e-12 * max(abs(hi), 1e-300):   # flat curve: open a window around it
        pad = abs(hi) * 0.05 or 1.0
        return lo - pad, hi + pad
    pad = (hi - lo) * 0.08
    return lo - pad, hi + pad


def fmt_tick(v):
    return f"{v:.4g}"


def panel(x0, y0, w, h, fs, vals, label, color):
    lo, hi = math.log10(fs[0]), math.log10(fs[-1])
    ylo, yhi = axis_range(vals)
    px = lambda f: x0 + (math.log10(f) - lo) / (hi - lo) * w
    py = lambda v: y0 + h - (v - ylo) / (yhi - ylo) * h
    o = [f'<rect x="{x0}" y="{y0}" width="{w}" height="{h}" fill="none" stroke="#c9ced6"/>']
    for t in nice_ticks(ylo, yhi):
        o.append(f'<line x1="{x0}" x2="{x0+w}" y1="{py(t):.1f}" y2="{py(t):.1f}" stroke="#e6e9ee"/>')
        o.append(f'<text x="{x0-8}" y="{py(t)+4:.1f}" text-anchor="end">{fmt_tick(t)}</text>')
    for d in range(math.ceil(lo), math.floor(hi) + 1):
        o.append(f'<line x1="{px(10**d):.1f}" x2="{px(10**d):.1f}" y1="{y0}" y2="{y0+h}" stroke="#e6e9ee"/>')
    pts = " ".join(f"{px(f):.1f},{py(v):.1f}" for f, v in zip(fs, vals))
    o.append(f'<polyline points="{pts}" fill="none" stroke="{color}" stroke-width="2.5" stroke-linejoin="round"/>')
    o.append(f'<text x="{x0-58}" y="{y0+h/2}" text-anchor="middle" transform="rotate(-90 {x0-58} {y0+h/2})">{label}</text>')
    return o, px


def svg(e):
    s = e["series"]
    fs = [p["f_hz"] for p in s]
    r = [p["r_ohm"] * 1e3 for p in s]   # mOhm
    l = [p["l_h"] * 1e9 for p in s]     # nH
    W, H, x0, w = 720, 470, 90, 594
    title = f'{e["title"]}: port 1 self-impedance, 1 MHz to 1 GHz'
    o = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" role="img" aria-labelledby="t d" font-family="system-ui,sans-serif" font-size="13" fill="#2b313b">',
         f'<title id="t">{title}</title>'.replace("&", "&amp;"),
         f'<desc id="d">Computed by fasterhenry. Resistance goes from {r[0]:.4g} to {r[-1]:.4g} milliohms; inductance from {l[0]:.4g} to {l[-1]:.4g} nanohenries.</desc>',
         f'<rect width="{W}" height="{H}" fill="#ffffff"/>']
    a, _ = panel(x0, 34, w, 170, fs, r, "R (mΩ)", "#b4400a")
    b, px = panel(x0, 244, w, 170, fs, l, "L (nH)", "#0b5cad")
    o += a + b
    o.append(f'<text x="{x0}" y="22" font-weight="600">R(f)</text><text x="{x0}" y="232" font-weight="600">L(f)</text>')
    for d, lab in zip(range(6, 10), ["1 MHz", "10 MHz", "100 MHz", "1 GHz"]):
        o.append(f'<text x="{px(10**d):.1f}" y="434" text-anchor="middle">{lab}</text>')
    o.append(f'<text x="{x0+w/2}" y="460" text-anchor="middle">frequency</text></svg>')
    return "\n".join(o)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bin", default="target/release/fasterhenry")
    args = ap.parse_args()
    binary = str(Path(args.bin))
    entries = [entry(binary, n) for n in DECKS]
    OUT.mkdir(parents=True, exist_ok=True)
    for old in OUT.glob("*.svg"):
        old.unlink()
    for e in entries:
        if e["status"] == "ok":
            (OUT / f'{e["id"]}.svg').write_text(svg(e) + "\n")
        else:
            print(f'FAILED {e["id"]}: {e["error"]}', file=sys.stderr)
    machine = {"cpu_count": os.cpu_count(), "platform": platform.platform(), "runs_per_deck": RUNS,
               "note": "wall time is the median process wall clock on this machine, not a benchmark"}
    (OUT / "gallery.json").write_text(json.dumps({"machine": machine, "entries": entries}, indent=2) + "\n")
    print(f"wrote {OUT.relative_to(ROOT)}/gallery.json and {sum(e['status']=='ok' for e in entries)} svg(s)", file=sys.stderr)
    sys.exit(1 if any(e["status"] != "ok" for e in entries) else 0)


if __name__ == "__main__":
    main()
