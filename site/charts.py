"""Shared chart chrome for the site's figures (stdlib only).

One module draws every chart: the hero R/L figure (site/gen_hero.py), the
gallery R/L figures and top-view sketches (tools/gallery_figures.py) and the
benchmarks scaling plot (site/render.py). Importers put `site/` on sys.path.

Colours are CSS classes, never attributes, so one drawing serves two uses:

  * inline in a page (`standalone=False`): no <style>, no background; the
    classes are bound to the site's tokens in site.css (`svg.chart ...`), so the
    chart follows the page theme;
  * a standalone .svg file (`standalone=True`): the same markup plus an
    embedded <style> with literal colours and a `prefers-color-scheme` block,
    since a file opened on its own cannot see the page's CSS.

`PALETTE` is the single list of literal colours; site.css repeats the same
values as custom properties (checked by site/test_build.py).

Two layouts exist because a 720 px-wide drawing scaled to a 412 px phone
shrinks its 13 px text to 7 px: WIDE (viewBox 720 wide) and COMPACT (viewBox
300 wide, drawn at about 1:1 on a phone, 11 px text). Pages inline both for
the full-width figures and let CSS show one.
"""
import math
from html import escape

# (light, dark) literal values for the standalone files. The inline charts use
# the site tokens in site.css whose values are the same pairs.
PALETTE = {
    "fg":     ("#171c24", "#e6e9ee"),   # --fg: tick and title text
    "muted":  ("#525c69", "#a3acb9"),   # --muted: guides, secondary text
    "frame":  ("#6f7885", "#7c8697"),   # plot frame
    "grid":   ("#d9dde3", "#2a313b"),   # --line: grid lines
    "s1":     ("#a8400c", "#f0935a"),   # --copper: first series (R, dense)
    "s2":     ("#0b5cad", "#7db8ff"),   # --link: second series (L, iterative)
    "bg":     ("#ffffff", "#0f1318"),   # --bg: what the series must contrast with
}

CLASSES = {   # class -> (property, palette key)
    "tx": ("fill", "fg"), "mu": ("fill", "muted"), "fr": ("stroke", "frame"), "gr": ("stroke", "grid"),
    "s1": ("stroke", "s1"), "s2": ("stroke", "s2"), "f1": ("fill", "s1"), "f2": ("fill", "s2"),
    "gd": ("stroke", "muted"),
    # top-view sketches: copper trace / plane, dashed contact region, port dot, scale bar
    "cu": ("stroke", "s1"), "pl": ("stroke", "s1"), "ct": ("stroke", "muted"), "pt": ("fill", "fg"),
    "sb": ("stroke", "muted"),
}


def _rules(i):
    return "".join(f".{c}{{{prop}:{PALETTE[key][i]}}}" for c, (prop, key) in CLASSES.items())


STANDALONE_STYLE = (
    "<style>.cu,.ct,.sb{fill:none}.cu{stroke-linecap:square}.pl{fill:#a8400c;fill-opacity:.2;fill-rule:evenodd}"
    ".ct{stroke-dasharray:4 3}.fr,.gr,.s1,.s2,.gd,.ln{fill:none}.fr,.gr,.gd{stroke-width:1}.s1,.s2,.ln{stroke-width:2.5;stroke-linejoin:round}"
    ".dt{stroke:none}.gd{stroke-dasharray:5 4}.b{font-weight:600}"
    + _rules(0) + "@media (prefers-color-scheme:dark){.pl{fill:#f0935a}" + _rules(1) + "}</style>")

# ---- sizes ------------------------------------------------------------------

# name -> (viewBox width, font size). COMPACT text is 11 px at 1:1, which is
# what a 412 px phone gives (content column about 330 px).
WIDE = {"name": "wide", "W": 720, "fs": 13, "left": 84, "right": 36, "ph": 170}
COMPACT = {"name": "compact", "W": 300, "fs": 11, "left": 46, "right": 18, "ph": 104}


def uid_ids(uid):
    return f"{uid}-t", f"{uid}-d"


def open_svg(W, H, uid, title, desc, fs, cls="chart", standalone=False, extra=""):
    """Opening tag, <title>, <desc> (ids unique per figure) and, for a standalone
    file, the embedded style. The caller appends the body and calls close_svg."""
    t, d = uid_ids(uid)
    o = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" class="{cls}" role="img" '
         f'aria-labelledby="{t}" aria-describedby="{d}" font-family="system-ui,sans-serif" font-size="{fs}"{extra}>',
         f'<title id="{t}">{escape(title, quote=False)}</title>',
         f'<desc id="{d}">{escape(desc, quote=False)}</desc>']
    if standalone:
        o.append(STANDALONE_STYLE)
    return o


def close_svg(o):
    o.append("</svg>")
    return "\n".join(o)


# ---- axes ---------------------------------------------------------------------

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
    """Short tick text: at most 4 significant digits, no trailing zeros."""
    return f"{v:.4g}"


FREQ_LABELS = {6: "1 MHz", 7: "10 MHz", 8: "100 MHz", 9: "1 GHz"}


def freq_panel(x0, y0, w, h, fs, vals, color, title, ylo, yhi, yticks, fsz):
    """One R(f) or L(f) panel: frame, grid, y ticks, the series polyline and its
    title, which carries the unit ("R, mΩ"), so there is no rotated axis label.
    Returns (svg lines, x-mapping)."""
    lo, hi = math.log10(fs[0]), math.log10(fs[-1])
    px = lambda f: x0 + (math.log10(f) - lo) / (hi - lo) * w
    py = lambda v: y0 + h - (v - ylo) / (yhi - ylo) * h
    o = [f'<text class="tx b" x="{x0}" y="{y0-8}">{escape(title)}</text>',
         f'<rect class="fr" x="{x0}" y="{y0}" width="{w}" height="{h}"/>']
    for t in yticks:
        o.append(f'<line class="gr" x1="{x0}" x2="{x0+w}" y1="{py(t):.1f}" y2="{py(t):.1f}"/>')
        o.append(f'<text class="tx" x="{x0-6}" y="{py(t)+fsz*0.32:.1f}" text-anchor="end">{fmt_tick(t)}</text>')
    for d in range(math.ceil(lo), math.floor(hi) + 1):
        o.append(f'<line class="gr" x1="{px(10**d):.1f}" x2="{px(10**d):.1f}" y1="{y0}" y2="{y0+h}"/>')
    pts = " ".join(f"{px(f):.1f},{py(v):.1f}" for f, v in zip(fs, vals))
    o.append(f'<polyline class="{color}" points="{pts}"/>')
    return o, px


def rl_figure(series, title, desc, uid, layout=WIDE, standalone=False, r_axis=None, l_axis=None):
    """R(f) over L(f) of a series [{f_hz, r_ohm, l_h}] on a shared log-frequency
    axis. `r_axis` / `l_axis` = (lo, hi, ticks) pin a panel's range; otherwise
    it is axis_range(). Units are in the panel titles."""
    fs = [p["f_hz"] for p in series]
    r = [p["r_ohm"] * 1e3 for p in series]   # mOhm
    l = [p["l_h"] * 1e9 for p in series]     # nH
    W, fsz, x0, ph = layout["W"], layout["fs"], layout["left"], layout["ph"]
    w = W - x0 - layout["right"]
    y1 = 8 + fsz + 8
    y2 = y1 + ph + 14 + fsz + 8
    H = y2 + ph + fsz + 12 + 8
    o = open_svg(W, H, uid, title, desc, fsz, f'chart {layout["name"]}', standalone)

    def axis(vals, given):
        if given:
            return given
        lo, hi = axis_range(vals)
        return lo, hi, nice_ticks(lo, hi)

    a = axis(r, r_axis)
    b = axis(l, l_axis)
    pa, _ = freq_panel(x0, y1, w, ph, fs, r, "s1", "R, mΩ", a[0], a[1], a[2], fsz)
    pb, px = freq_panel(x0, y2, w, ph, fs, l, "s2", "L, nH", b[0], b[1], b[2], fsz)
    o += pa + pb
    lab_y = y2 + ph + fsz + 6
    for d in range(6, 10):
        o.append(f'<text class="tx" x="{px(10**d):.1f}" y="{lab_y}" text-anchor="middle">{FREQ_LABELS[d]}</text>')
    return close_svg(o)


def scaling_figure(points, markers, title, desc, uid, layout=WIDE, standalone=False):
    """Log-log wall clock (s) against filaments; `markers` = [(x, label)] vertical
    guides. The y unit is in the title, so ticks are bare numbers."""
    compact = layout["name"] == "compact"
    W, fsz = layout["W"], layout["fs"]
    x0, y0 = (46, 40) if compact else (62, 40)
    w = W - x0 - (20 if compact else 36)
    h = 200 if compact else 300
    xs = [p[0] for p in points]
    ys = [v for p in points for v in p[1:] if v]
    lx0, lx1 = math.floor(math.log10(min(xs))), math.ceil(math.log10(max(xs)))
    ly0, ly1 = math.floor(math.log10(min(ys))), math.ceil(math.log10(max(ys)))
    px = lambda x: x0 + (math.log10(x) - lx0) / (lx1 - lx0) * w
    py = lambda y: y0 + h - (math.log10(y) - ly0) / (ly1 - ly0) * h
    H = y0 + h + 24 + fsz + 14
    o = open_svg(W, H, uid, title, desc, fsz, f'chart {layout["name"]}', standalone)
    o.append(f'<text class="tx b" x="{x0}" y="{y0-24 if compact else y0-14}">wall clock, s</text>')
    o.append(f'<rect class="fr" x="{x0}" y="{y0}" width="{w}" height="{h}"/>')
    for d in range(lx0, lx1 + 1):
        o.append(f'<line class="gr" x1="{px(10**d):.1f}" x2="{px(10**d):.1f}" y1="{y0}" y2="{y0+h}"/>')
        o.append(f'<text class="tx" x="{px(10**d):.1f}" y="{y0+h+fsz+6}" text-anchor="middle">{compact_count(10**d)}</text>')
    for d in range(ly0, ly1 + 1):
        o.append(f'<line class="gr" x1="{x0}" x2="{x0+w}" y1="{py(10**d):.1f}" y2="{py(10**d):.1f}"/>')
        o.append(f'<text class="tx" x="{x0-6}" y="{py(10**d)+fsz*0.32:.1f}" text-anchor="end">{10**d:g}</text>')
    # Guide labels sit at the top; in the compact layout they stack on two lines
    # above the frame, since the plot is too narrow for them side by side.
    ms = sorted(m for m in markers if min(xs) <= m[0] <= max(xs))
    for i, (mx, lab) in enumerate(ms):
        o.append(f'<line class="gd" x1="{px(mx):.1f}" x2="{px(mx):.1f}" y1="{y0 - (22 - 14 * i if compact else 0)}" y2="{y0+h}"/>')
        if compact:
            o.append(f'<text class="mu" x="{px(mx)+4:.1f}" y="{y0 - (22 - 14 * i) + fsz - 2}">{escape(lab)}</text>')
        else:
            left = i % 2 == 0 and len(ms) > 1
            o.append(f'<text class="mu" x="{px(mx)+(-5 if left else 5):.1f}" y="{y0+16}" text-anchor="{"end" if left else "start"}">{escape(lab)}</text>')
    # Series names live in a legend in the empty top-left corner, not on the lines.
    for k, cls, name in ((1, "s1", "dense"), (2, "s2", "pFFT + GMRES")):
        pts = [(p[0], p[k]) for p in points if p[k]]
        o.append('<polyline class="' + cls + '" points="' + " ".join(f"{px(x):.1f},{py(y):.1f}" for x, y in pts) + '"/>')
        o += [f'<circle class="dt f{k}" cx="{px(x):.1f}" cy="{py(y):.1f}" r="4"/>' for x, y in pts]
        ly = y0 + 18 + 18 * (k - 1)
        o.append(f'<line class="{cls}" x1="{x0+10}" x2="{x0+32}" y1="{ly-4}" y2="{ly-4}"/>')
        o.append(f'<text class="tx b" x="{x0+38}" y="{ly}">{name}</text>')
    o.append(f'<text class="tx" x="{x0+w/2}" y="{y0+h+24+fsz}" text-anchor="middle">filaments</text>')
    return close_svg(o)


def compact_count(n):
    """1000 -> '1k', 100000 -> '100k', 10 -> '10'."""
    return f"{n // 1000}k" if n >= 1000 and n % 1000 == 0 else f"{n:g}"


# ---- inline pairs -------------------------------------------------------------

def both_layouts(draw, uid):
    """Markup for a page: the wide and the compact drawing of one figure, ids made
    unique with a suffix. CSS (site.css `.chart.wide` / `.chart.compact`) shows one."""
    return "\n".join(draw(layout, f"{uid}-{layout['name'][0]}") for layout in (WIDE, COMPACT))


def contrast(a, b):
    """WCAG contrast ratio of two #rrggbb colours."""
    def lum(c):
        ch = [int(c[i:i + 2], 16) / 255 for i in (1, 3, 5)]
        ch = [v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4 for v in ch]
        return 0.2126 * ch[0] + 0.7152 * ch[1] + 0.0722 * ch[2]
    la, lb = sorted((lum(a), lum(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)
