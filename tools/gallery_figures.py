#!/usr/bin/env python3
"""Regenerate the site's Gallery data and figures from the committed decks.

Command (repository root, after `cargo build --release -p fasterhenry-cli`):

    python3 tools/gallery_figures.py [--bin target/release/fasterhenry]

For every deck in DECKS it runs the engine with the sweep overridden to
1 MHz - 1 GHz at 10 points per decade, times the process (median wall clock of
RUNS runs), and writes under site/assets/gallery/

    gallery.json    one entry per deck: title, summary, command, wall time, counts,
                    R(f)/L(f) of port 1's self-impedance, or the error if it failed
    <id>.svg        the R/L vs frequency figure (stdlib only, no plotting lib)
    <id>_geometry.svg
                    top-view sketch drawn from the deck's own node coordinates

`--no-run` skips the engine: it keeps the series and timings already in
gallery.json and regenerates only the derived fields and every SVG.

A deck that does not run is recorded with status "failed" and its error text
(the page shows it) and the script exits non-zero; it is never dropped
silently. No FastHenry curve is overlaid: the repository commits agreement
numbers against FastHenry (docs/benchmarks.md), not FastHenry's curves.

Outputs are committed so `site/build.py` needs neither Rust nor Python
packages. Wall times are of the machine that ran this script, recorded in
gallery.json under "machine".
"""
import argparse, json, math, os, platform, re, statistics, subprocess, sys, tempfile, time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DATA = "fasterhenry-cli/tests/data"
DECKS = ["spiral", "twobar", "plane_fasthenry", "plane_extension", "plane_decay_rect", "plane_decay_extension"]
OUT = ROOT / "site/assets/gallery"
FREQ = ("1e6", "1e9", "10")
RUNS = 3
CMD = "fasterhenry {deck} --freq 1e6 1e9 10 --json out.json"


# Human titles and one-sentence summaries, written about the structure each deck
# describes (not the test it serves). `variant_of` folds a deck into another
# entry's card; `variant` says how that deck differs. Numbers shown on the page
# (filaments, times, R and L) come from gallery.json, never from this text.
META = {
    "spiral": {"title": "Two-turn square spiral",
               "summary": "A copper trace wound into a two-turn square spiral, driven between its centre and its outer end."},
    "twobar": {"title": "Two parallel bars",
               "summary": "Two straight copper bars side by side, each with its own port, so the result includes the coupling between them."},
    "plane_fasthenry": {"title": "Ground plane with a hole",
                        "summary": "A thin copper ground plane with a rectangular hole and a locally refined contact region, driven between two vias that land on it.",
                        "variant": "FastHenry corner-point syntax, contact rect"},
    "plane_extension": {"variant_of": "plane_fasthenry", "variant": "fasterhenry G / .hole / .contact syntax, same contact region"},
    "plane_decay_rect": {"variant_of": "plane_fasthenry", "variant": "FastHenry corner-point syntax, contact decay_rect (wider refined region)"},
    "plane_decay_extension": {"variant_of": "plane_fasthenry", "variant": "fasterhenry .contact syntax with nx, ny and ratio (wider refined region)"},
}


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
    e = {"id": name, "deck": f"{DATA}/{name}.inp", "command": CMD.format(deck=f"{DATA}/{name}.inp"),
         "overlay": None}
    e.update(derived(name, text))
    res, err, times = run_deck(binary, name)
    if res is None:
        e.update(status="failed", error=err)
        return e
    prov = res["provenance"]
    e.update(status="ok", wall_time_s=statistics.median(times), wall_time_runs=RUNS,
             fasterhenry_version=prov["fasterhenry_version"], counts=prov["counts"],
             ports=len(res["ports"]), series=series_from_result(res))
    return e


def derived(name, text):
    """Fields that depend only on META and the deck text, not on running the engine."""
    m = META[name]
    d = {"title": m.get("title") or META[m["variant_of"]]["title"], "summary": m.get("summary", "")}
    if "variant_of" in m:
        d["variant_of"] = m["variant_of"]
    if "variant" in m:
        d["variant"] = m["variant"]
    d["geometry"] = f"{name}_geometry.svg"
    return d


# ---- deck geometry (for the sketch) ----------------------------------------

UNITS_MM = {"m": 1e3, "cm": 10.0, "mm": 1.0, "um": 1e-3, "in": 25.4, "mils": 0.0254}
KV = re.compile(r"(\w+)=([-+0-9.eE]+)")
NUM = r"[-+0-9.eE]+"


def parse_geometry(text):
    """Nodes, elements, plane rectangles (with holes and refined contact
    regions) and ports of a deck, in millimetres. Only what the sketch draws."""
    lines = []
    for ln in text.splitlines():
        s = ln.strip().lower()
        if not s or s.startswith("*"):
            continue
        if s.startswith("+") and lines:
            lines[-1] += " " + s[1:].strip()
        else:
            lines.append(s)
    k = 1e3
    default, nodes, elems, planes, ports = {}, {}, [], {}, []
    for s in lines:
        tok = s.split()
        kv = {a: float(b) for a, b in KV.findall(s)}
        if tok[0] == ".units":
            k = UNITS_MM[tok[1]]
        elif tok[0] == ".default":
            default.update(kv)
        elif tok[0] == ".external":
            ports.append((tok[1], tok[2]))
        elif tok[0] in (".hole", ".contact"):
            v = [float(x) for x in tok[2:6]]
            planes[tok[1]]["holes" if tok[0] == ".hole" else "contacts"].append(
                (v[0] * k, v[1] * k, v[2] * k, v[3] * k))
        elif tok[0][0] == "n" and "x" in kv:
            nodes[tok[0]] = (kv["x"] * k, kv.get("y", 0.0) * k, kv.get("z", default.get("z", 0.0)) * k)
        elif tok[0][0] == "e" and len(tok) >= 3:
            elems.append((tok[1], tok[2], kv.get("w", default.get("w", 0.0)) * k))
        elif tok[0][0] == "g":
            if "x1" in kv:
                pts = [(kv["x1"], kv["y1"]), (kv["x2"], kv["y2"]), (kv["x3"], kv["y3"])]
                pts.append((pts[0][0] + pts[2][0] - pts[1][0], pts[0][1] + pts[2][1] - pts[1][1]))
                xs, ys = [p[0] for p in pts], [p[1] for p in pts]
                rect = (min(xs) * k, min(ys) * k, max(xs) * k, max(ys) * k)
            else:
                v = [float(x) for x in tok[1:7]]
                rect = (min(v[0], v[3]) * k, min(v[1], v[4]) * k, max(v[0], v[3]) * k, max(v[1], v[4]) * k)
            pl = {"rect": rect, "holes": [], "contacts": []}
            for kind, _, args in re.findall(r"(hole|contact)\s+(rect|decay_rect)\s*\(([^)]*)\)", s):
                v = [float(x) for x in args.split(",")]
                if len(v) == 6:
                    c = (v[0], v[1], v[3], v[4])
                else:   # centre, full widths about it
                    c = (v[0] - v[3] / 2, v[1] - v[4] / 2, v[0] + v[3] / 2, v[1] + v[4] / 2)
                pl["holes" if kind == "hole" else "contacts"].append(tuple(x * k for x in c))
            for nm, args in re.findall(r"\b(n\w+)\s*\(([^)]*)\)", s):   # landing nodes declared in the clause
                v = [float(x) for x in args.split(",")]
                nodes[nm] = (v[0] * k, v[1] * k, v[2] * k)
            planes[tok[0]] = pl
    return {"nodes": nodes, "elements": elems, "planes": list(planes.values()), "ports": ports}


def scale_bar_mm(span_mm):
    """Round length (1, 2 or 5 times a power of ten) near a third of `span_mm`."""
    mag = 10 ** math.floor(math.log10(span_mm / 3))
    return max((m * mag for m in (1, 2, 5, 10) if m * mag <= span_mm / 3), default=mag)


def geometry_svg(geom, title):
    """Top view (x-y) of the conductors; copper traces, plane outline with holes
    cut out, refined contact regions dashed, port nodes dotted. Light and dark
    colours switch inside the file, since an <img> cannot see the page's CSS."""
    W, HMAX, PAD = 360, 230, 16
    xs, ys = [], []
    for pl in geom["planes"]:
        xs += [pl["rect"][0], pl["rect"][2]]
        ys += [pl["rect"][1], pl["rect"][3]]
    for a, b, _ in geom["elements"]:
        for n in (a, b):
            xs.append(geom["nodes"][n][0])
            ys.append(geom["nodes"][n][1])
    x0, x1, y0, y1 = min(xs), max(xs), min(ys), max(ys)
    bw, bh = max(x1 - x0, 1e-9), max(y1 - y0, 1e-9)
    sc = min((W - 2 * PAD) / bw, (HMAX - 2 * PAD - 14) / bh)
    H = round(bh * sc + 2 * PAD + 14)
    px = lambda x: PAD + (x - x0) * sc + (W - 2 * PAD - bw * sc) / 2
    py = lambda y: PAD + (y1 - y) * sc
    rp = lambda r: f"M{px(r[0]):.1f} {py(r[1]):.1f}H{px(r[2]):.1f}V{py(r[3]):.1f}H{px(r[0]):.1f}Z"
    o = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" role="img" aria-labelledby="t" font-family="system-ui,sans-serif" font-size="11">',
         f'<title id="t">{title.replace("&", "&amp;")}: top view of the conductors</title>',
         '<style>.cu{stroke:#a8400c;fill:none;stroke-linecap:square}.pl{fill:#a8400c;fill-opacity:.2;stroke:#a8400c;fill-rule:evenodd}'
         '.ct{fill:none;stroke:#525c69;stroke-dasharray:4 3}.pt{fill:#171c24}.tx{fill:#525c69}.sb{stroke:#525c69;fill:none}'
         '@media (prefers-color-scheme:dark){.cu{stroke:#f0935a}.pl{fill:#f0935a;stroke:#f0935a}.ct,.sb{stroke:#a3acb9}.pt{fill:#e6e9ee}.tx{fill:#a3acb9}}</style>']
    for pl in geom["planes"]:
        o.append(f'<path class="pl" d="{rp(pl["rect"])}{"".join(rp(h) for h in pl["holes"])}"/>')
        o += [f'<path class="ct" d="{rp(c)}"/>' for c in pl["contacts"]]
    for a, b, w in geom["elements"]:
        (xa, ya, _), (xb, yb, _) = geom["nodes"][a], geom["nodes"][b]
        if (xa, ya) != (xb, yb):    # vias run along z and are a point in this view
            o.append(f'<line class="cu" x1="{px(xa):.1f}" y1="{py(ya):.1f}" x2="{px(xb):.1f}" y2="{py(yb):.1f}" stroke-width="{max(w * sc, 1.5):.1f}"/>')
    for n in sorted({n for p in geom["ports"] for n in p}):
        x, y, _ = geom["nodes"][n]
        o.append(f'<circle class="pt" cx="{px(x):.1f}" cy="{py(y):.1f}" r="3.5"/>')
    bar = scale_bar_mm(bw)
    sx, sy = PAD, H - 8
    o.append(f'<path class="sb" d="M{sx} {sy-4}V{sy}H{sx+bar*sc:.1f}V{sy-4}"/>'
             f'<text class="tx" x="{sx+bar*sc+6:.1f}" y="{sy}">{bar:g} mm · top view</text></svg>')
    return "\n".join(o)


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


MIN_REL_SPAN = 0.2   # the y-axis always spans at least 20% of the plotted value


def axis_range(vals):
    """Padded range of the data, widened to MIN_REL_SPAN of its magnitude so a
    change of a fraction of a percent reads as the small change it is."""
    lo, hi = min(vals), max(vals)
    mag = max(abs(lo), abs(hi))
    span = max((hi - lo) * 1.16, MIN_REL_SPAN * mag) or 1.0
    mid = (lo + hi) / 2
    return mid - span / 2, mid + span / 2


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
    ap.add_argument("--no-run", action="store_true",
                    help="reuse the series and timings in gallery.json; regenerate derived fields and SVGs only")
    args = ap.parse_args()
    binary = str(Path(args.bin))
    if args.no_run:
        old = json.loads((OUT / "gallery.json").read_text())
        entries = []
        for e in old["entries"]:
            e.pop("description", None)
            e.update(derived(e["id"], (ROOT / DATA / f'{e["id"]}.inp').read_text()))
            entries.append(e)
        machine = old["machine"]
    else:
        entries = [entry(binary, n) for n in DECKS]
        machine = {"cpu_count": os.cpu_count(), "platform": platform.platform(), "runs_per_deck": RUNS,
                   "note": "wall time is the median process wall clock on this machine, not a benchmark"}
    OUT.mkdir(parents=True, exist_ok=True)
    for old in OUT.glob("*.svg"):
        old.unlink()
    for e in entries:
        text = (ROOT / DATA / f'{e["id"]}.inp').read_text()
        (OUT / e["geometry"]).write_text(geometry_svg(parse_geometry(text), e["title"]) + "\n")
        if e["status"] == "ok":
            (OUT / f'{e["id"]}.svg').write_text(svg(e) + "\n")
        else:
            print(f'FAILED {e["id"]}: {e["error"]}', file=sys.stderr)
    (OUT / "gallery.json").write_text(json.dumps({"machine": machine, "entries": entries}, indent=2) + "\n")
    print(f"wrote {OUT.relative_to(ROOT)}/gallery.json and {sum(e['status']=='ok' for e in entries)} figure(s)", file=sys.stderr)
    sys.exit(1 if any(e["status"] != "ok" for e in entries) else 0)


if __name__ == "__main__":
    main()
