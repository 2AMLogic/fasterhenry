"""Page bodies for the Gallery and Benchmarks pages (stdlib only).

Imported by build.py. Nothing here is hand-typed data:

  * the gallery comes from site/assets/gallery/gallery.json (tools/gallery_figures.py)
  * the benchmarks page is rendered from docs/benchmarks.md, and its scaling
    plot is drawn from that file's own "Scaling: dense vs pFFT + GMRES" table
"""
import json, math, re
from html import escape
from pathlib import Path

import tablehtml

REPO = "https://github.com/2AMLogic/fasterhenry"
BLOB = f"{REPO}/blob/main"


def inline(md):
    """Escape, then the four inline forms docs/benchmarks.md uses."""
    s = escape(md, quote=False)
    s = re.sub(r"`([^`]+)`", r"<code>\1</code>", s)
    s = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", s)
    s = re.sub(r"(?<![\w*])\*(?=[^\s*])([^*]+?)(?<=[^\s*])\*(?![\w*])", r"<em>\1</em>", s)
    return re.sub(r"(?<![\w*])_([^_]+)_(?![\w*])", r"<em>\1</em>", s)


# ---- gallery ---------------------------------------------------------------

def gallery_body(data):
    m = data["machine"]
    out = [f'<p>Every figure below is the output of <code>tools/gallery_figures.py</code> run on a deck committed in '
           f'<a href="{BLOB}/fasterhenry-cli/tests/data">fasterhenry-cli/tests/data</a>. Plots show port 1&rsquo;s '
           f'self-impedance, 1&nbsp;MHz to 1&nbsp;GHz, 10 points per decade. No FastHenry curve is overlaid on any entry: '
           f'this repository does not commit FastHenry output.</p>',
           f'<p>Wall time is the median process wall clock of {m["runs_per_deck"]} runs on the machine that generated the '
           f'figures ({m["cpu_count"]} logical CPUs, {escape(m["platform"])}). It is a per-run timing, not a benchmark; see '
           f'<a href="/benchmarks/">Benchmarks</a>.</p>']
    for e in data["entries"]:
        out.append(f'<section id="{escape(e["id"])}">\n<h2>{escape(e["title"])}</h2>')
        if e["description"]:
            out.append(f'<p>{inline(e["description"])}</p>')
        if e["status"] != "ok":
            out.append(f'<p><strong>This deck did not run.</strong> The CLI reported:</p><pre><code>{escape(e["error"])}</code></pre>')
        else:
            c = e["counts"]
            out.append(f'<figure><img src="/assets/gallery/{escape(e["id"])}.svg" width="720" height="470" '
                       f'alt="R and L versus frequency for {escape(e["title"])}">'
                       f'<figcaption>R(f) and L(f) of port 1 · {c["filaments"]} filaments, {e["ports"]} port(s) · '
                       f'wall time {e["wall_time_s"] * 1e3:.1f} ms · fasterhenry {escape(e["fasterhenry_version"])}</figcaption></figure>')
        out.append(f'<p>Deck: <a href="{BLOB}/{escape(e["deck"])}"><code>{escape(e["deck"])}</code></a><br>'
                   f'Command: <code>{escape(e["command"])}</code></p>\n</section>')
    return "\n".join(out)


# ---- docs/benchmarks.md ----------------------------------------------------

def section(md, heading, level=2):
    """Text under the first heading starting with `heading`, up to the next heading of <= level."""
    hashes = "#" * level
    m = re.search(rf"^{hashes} {re.escape(heading)}.*$", md, re.M)
    if not m:
        raise KeyError(heading)
    rest = md[m.end():]
    n = re.search(rf"^#{{1,{level}}} ", rest, re.M)
    return rest[: n.start()] if n else rest


def tables(text):
    """[(header_cells, [row_cells...])] for each pipe table in `text`."""
    out, cur = [], []
    for ln in text.splitlines() + [""]:
        if ln.startswith("|"):
            cur.append([c.strip() for c in ln.strip().strip("|").split("|")])
        elif cur:
            if len(cur) >= 2 and all(re.fullmatch(r":?-+:?", c) for c in cur[1]):
                out.append((cur[0], cur[2:]))
            cur = []
    return out


def table_html(header, rows):
    return tablehtml.render_table(header, rows, inline)


def number(cell):
    """'**0.84**' -> 0.84, '2 400' -> 2400.0, 'not attempted' -> None."""
    s = cell.replace("*", "").replace(" ", "").replace(" ", "").replace(",", "")
    return float(s) if re.fullmatch(r"\d+(\.\d+)?", s) else None


def bullet(text, label):
    """Body of the `- **label**: ...` bullet (continuation lines joined)."""
    m = re.search(rf"^- \*\*{re.escape(label)}\*\*: (.*?)(?=^- \*\*|^\S|\Z)", text, re.M | re.S)
    return " ".join(m.group(1).split()) if m else None


def scaling_points(scaling_text):
    for header, rows in tables(scaling_text):
        if header[0] == "filaments":
            i_d, i_i = header.index("dense s"), header.index("iterative s")
            return [(number(r[0]), number(r[i_d]), number(r[i_i])) for r in rows]
    raise KeyError("scaling table")


def compact(n):
    """1000 -> '1k', 100000 -> '100k', 10 -> '10'."""
    return f"{n // 1000}k" if n >= 1000 and n % 1000 == 0 else f"{n:g}"


def scaling_svg(points, markers):
    """Log-log wall clock vs filaments; `markers` = [(x, label)] vertical guides."""
    W, H, x0, y0, w, h = 720, 410, 76, 24, 590, 340
    xs = [p[0] for p in points]
    ys = [v for p in points for v in p[1:] if v]
    lx0, lx1 = math.floor(math.log10(min(xs))), math.ceil(math.log10(max(xs)))
    ly0, ly1 = math.floor(math.log10(min(ys))), math.ceil(math.log10(max(ys)))
    px = lambda x: x0 + (math.log10(x) - lx0) / (lx1 - lx0) * w
    py = lambda y: y0 + h - (math.log10(y) - ly0) / (ly1 - ly0) * h
    o = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" role="img" aria-labelledby="t d" font-family="system-ui,sans-serif" font-size="13" fill="#2b313b">',
         '<title id="t">Dense versus pFFT + GMRES wall clock against filament count</title>',
         '<desc id="d">Log-log plot from the scaling table in docs/benchmarks.md. The dense path grows faster than the iterative path and stops where it does not fit in memory.</desc>',
         f'<rect width="{W}" height="{H}" fill="#ffffff"/>',
         f'<rect x="{x0}" y="{y0}" width="{w}" height="{h}" fill="none" stroke="#c9ced6"/>']
    for d in range(lx0, lx1 + 1):
        o.append(f'<line x1="{px(10**d):.1f}" x2="{px(10**d):.1f}" y1="{y0}" y2="{y0+h}" stroke="#e6e9ee"/>')
        o.append(f'<text x="{px(10**d):.1f}" y="{y0+h+18}" text-anchor="middle">{compact(10**d)}</text>')
    for d in range(ly0, ly1 + 1):
        o.append(f'<line x1="{x0}" x2="{x0+w}" y1="{py(10**d):.1f}" y2="{py(10**d):.1f}" stroke="#e6e9ee"/>')
        o.append(f'<text x="{x0-8}" y="{py(10**d)+4:.1f}" text-anchor="end">{10**d:g} s</text>')
    # Guide labels sit at the top: the left-most guide's label ends at its line,
    # the next one starts at its line, so no label crosses another guide.
    ms = sorted(m for m in markers if min(xs) <= m[0] <= max(xs))
    for i, (mx, lab) in enumerate(ms):
        o.append(f'<line x1="{px(mx):.1f}" x2="{px(mx):.1f}" y1="{y0}" y2="{y0+h}" stroke="#6b7480" stroke-dasharray="5 4"/>')
        left = i % 2 == 0 and len(ms) > 1
        o.append(f'<text x="{px(mx)+(-5 if left else 5):.1f}" y="{y0+16}" text-anchor="{"end" if left else "start"}" fill="#4a5360">{escape(lab)}</text>')
    # Series names live in a legend in the empty top-left corner, not on the lines.
    for k, color, name in ((1, "#b4400a", "dense"), (2, "#0b5cad", "pFFT + GMRES")):
        pts = [(p[0], p[k]) for p in points if p[k]]
        o.append('<polyline points="' + " ".join(f"{px(x):.1f},{py(y):.1f}" for x, y in pts) +
                 f'" fill="none" stroke="{color}" stroke-width="2.5" stroke-linejoin="round"/>')
        o += [f'<circle cx="{px(x):.1f}" cy="{py(y):.1f}" r="4" fill="{color}"/>' for x, y in pts]
        ly = y0 + 18 + 20 * (k - 1)
        o.append(f'<line x1="{x0+12}" x2="{x0+36}" y1="{ly-4}" y2="{ly-4}" stroke="{color}" stroke-width="2.5"/>')
        o.append(f'<text x="{x0+42}" y="{ly}" fill="{color}" font-weight="600">{name}</text>')
    o.append(f'<text x="{x0+w/2}" y="{y0+h+40}" text-anchor="middle">filaments</text></svg>')
    return "\n".join(o)


def note_html(text):
    """One paragraph; a whole-paragraph `_..._` wrapper becomes <em> around the
    rest. inline() alone cannot close it when a code span holds an underscore."""
    t = " ".join(text.split())
    if len(t) > 2 and t[0] == "_" and t[-1] == "_":
        return f"<em>{inline(t[1:-1])}</em>"
    return inline(t)


def quote_after(text, prefix):
    """The paragraph or bullet of `text` that starts with `prefix`, verbatim (whitespace joined)."""
    m = re.search(rf"^(?:- )?({re.escape(prefix)}.*?)(?=\n\s*\n|^- |\Z)", text, re.M | re.S)
    if not m:
        raise KeyError(prefix)
    return " ".join(m.group(1).split())


def sentence(text, prefix):
    """The sentence of `text` that starts with `prefix`, verbatim."""
    t = " ".join(text.split())
    m = re.search(rf"{re.escape(prefix)}.*?[.](?=\s|$)", t)
    if not m:
        raise KeyError(prefix)
    return m.group(0)


def benchmarks_body(md):
    setup = section(md, "Setup")
    speed = section(md, "Speed")
    acc = section(md, "Accuracy")
    crit = section(md, "Committed criterion bench suite")
    scaling = section(md, "Scaling: dense vs pFFT")
    hw = bullet(setup, "Hardware")
    hw2 = bullet(scaling, "Hardware")
    block = re.search(r"<!-- bench-table:begin -->\n(.*?)\n<!-- bench-table:end -->", crit, re.S)
    ctab = tables(block.group(1)) if block else []

    pts = scaling_points(scaling)
    markers = []
    m = re.search(r"DENSE_PATH_MAX_FILAMENTS`\s+is\s+\*\*([\d  ]+)\*\*", md)
    if m:
        markers.append((number(m.group(1)), "default handover"))
    m = re.search(r"wall-clock crossover is near ([\d  ]+) filaments", md)
    if m:
        markers.insert(0, (number(m.group(1)), "wall-clock crossover"))

    doc = f'<a href="{BLOB}/docs/benchmarks.md"><code>docs/benchmarks.md</code></a>'
    # Takeaway: two sentences quoted verbatim from the doc, nothing newly written.
    agree = quote_after(acc, "Sub-0.1 %")
    caveat = quote_after(speed, "**Per thread")
    advantage = sentence(caveat, "Our dense wall-clock advantage")
    o = ['<h2>Takeaway</h2>',
         f'<blockquote><p>{inline(agree)}</p><p>{inline(advantage)}</p></blockquote>',
         f'<p>Quoted from {doc}, which is where every number on this page comes from.</p>']

    o.append('<h2>Scaling: dense vs pFFT + GMRES</h2>')
    o.append(f'<figure><img src="/assets/benchmarks/scaling.svg" width="720" height="410" '
             'alt="Log-log plot of wall clock against filament count for the dense and the pFFT plus GMRES paths">'
             '<figcaption>Wall clock of <code>assemble</code> + one-frequency <code>sweep</code>, single thread. '
             'The dense arm stops where its model memory no longer fits; the table is the data.</figcaption></figure>')
    o.append('<details><summary>Scaling table</summary>')
    for hd, rows in tables(scaling):
        if hd[0] == "filaments":
            o.append(table_html(hd, rows))
    o.append('</details>')
    o.append(f'<p>Reproduce: <code>cargo bench -p fasterhenry --bench scaling -- __report_only__</code>. '
             f'CI does not run this suite; it gates the headline with <code>iterative_path_holds_its_scaling_headline</code> in '
             f'<a href="{BLOB}/fasterhenry/tests/perf_smoke.rs"><code>perf_smoke.rs</code></a>.</p>')

    o.append('<h2>Speed against FastHenry</h2>')
    for hd, rows in tables(speed):
        o.append(table_html(hd, rows))
    o.append(f'<p>{inline(caveat)}</p>')
    o.append(f'<p>Operator-run, on one machine; method and caveats are in <a href="{BLOB}/docs/benchmarks.md#speed">docs/benchmarks.md</a>. '
             'CI never runs FastHenry.</p>')

    o.append('<h2>Accuracy against FastHenry</h2>')
    lead = re.search(r"^Same decks.*?(?=\n\s*\n)", acc, re.M | re.S)
    if lead:
        o.append(f'<p>{inline(" ".join(lead.group(0).split()))}</p>')
    for hd, rows in tables(acc):
        o.append(table_html(hd, rows))

    if ctab:
        o.append('<h2>Criterion bench suite</h2>')
        for hd, rows in ctab:
            o.append(table_html(hd, rows))
        o.append(f'<p>Regenerated by <a href="{BLOB}/.github/workflows/bench.yml"><code>.github/workflows/bench.yml</code></a> '
                 'via <code>tools/bench_table.py</code>, which parses criterion&rsquo;s own JSON output. '
                 'Locally: <code>cargo bench -p fasterhenry --bench assembly --bench solve &amp;&amp; python3 tools/bench_table.py</code>.</p>')

    o.append('<h2>How this was measured</h2><ul>')
    if hw:
        o.append(f'<li><strong>FastHenry comparison and the criterion suite:</strong> {inline(hw)}</li>')
    if hw2:
        o.append(f'<li><strong>Scaling run:</strong> {inline(hw2)}</li>')
    o.append('<li><strong>SIMD:</strong> the partial-inductance kernels use 4-wide <code>f64</code> lanes (<code>wide::f64x4</code>), '
             f'as in <a href="{BLOB}/fasterhenry/src/inductance/neumann.rs"><code>neumann.rs</code></a>.</li></ul>')
    return "\n".join(o), scaling_svg(pts, markers)


# ---- docs/validation.md ----------------------------------------------------

# Level-2 sections rendered below the summary (everything else in the file,
# including the runbook preamble, stays on GitHub).
VALIDATION_SECTIONS = ["Fixture", "Measured", "Oracles", "Ground planes",
                       "Surface-graded filaments", "Width-graded filaments", "Filament currents"]

# Maintainer-only blocks dropped from the rendered sections: a paragraph (or the
# table) is dropped when its text starts with one of these. Each must be found,
# so a doc edit that moves one fails the build rather than leaking it onto the page.
VALIDATION_DROPPED = [
    "**Margin is thin", "| check | measured | bound | headroom |",
    "Source: CI run", "If the gate fails", "The gate is the most expensive",
    "The `--fixture plane`", "Both contact gates are", "To regenerate it", "**CI.**",
]

_ISSUE_TAG = re.compile(r"\s*\((?:issue #\d+[^)]*)\)")
_ISSUE_REF = re.compile(r"(?<![\w&/])(?:issue )?#(\d+)\b")


def plain_cell(cell):
    return tablehtml.plain(cell)


def find_row(rows, prefix):
    for r in rows:
        if plain_cell(r[0]).startswith(prefix):
            return r
    raise KeyError(prefix)


def find_table(text, first_header):
    for hd, rows in tables(text):
        if hd[0] == first_header:
            return hd, rows
    raise KeyError(first_header)


def capture(pattern, text, group=1):
    m = re.search(pattern, text)
    if not m:
        raise KeyError(pattern)
    return " ".join(m.group(group).split())


def validation_summary(md, bench_md):
    """[(check, reference, measured, bound)], every value read from the docs."""
    measured = section(md, "Measured")
    comp = find_table(measured, "comparison")[1]
    planes = section(md, "Ground planes")
    slot = find_row(find_table(planes, "quantity")[1], "ΔL (slot)")
    slot_rel = plain_cell(slot[3])
    contact_mesh = find_table(planes, "mesh")[1]
    contact_sep = find_row([t for t in tables(planes) if t[0][0] == "quantity"][1][1], "ΔL (separation)")
    sep_rel = plain_cell(contact_sep[3])
    width = section(md, "Width-graded filaments")
    skin = re.search(r"`R'` is within ([\d.]+ %) \(measured ≤ ([\d.]+ %)\)", width)
    if not skin:
        raise KeyError("R' is within")
    fh = find_row(find_table(section(bench_md, "Accuracy"), "deck")[1], "spiral")
    bounds = capture(r"Bounds: ([\d.]+ %)", planes)

    def bound_of(cell):
        return capture(r"bound ([^)]+)\)", cell)

    def l_row(prefix):
        r = find_row(comp, prefix)
        return plain_cell(r[1]), plain_cell(r[2])

    pb, pm = l_row("L vs PyPEEC")
    gb, gm = l_row("L vs Greenhouse")
    ab, am = l_row("R vs analytic")
    graded = find_row(contact_mesh, "graded")
    return [
        ("Spiral inductance", "PyPEEC 5.8.0 voxel solve", pm, pb),
        ("Spiral inductance", "Greenhouse segment summation", gm, gb),
        ("Spiral DC resistance", "analytic series sum", am, ab),
        ("Slot ΔL in a ground plane", "PyPEEC 5 µm voxels", capture(r"^([\d.]+ %)", slot_rel), bound_of(slot_rel)),
        ("Graded contact mesh, L", "uniform 25 µm plane", plain_cell(graded[4]), bounds),
        ("Contact separation ΔL", "PyPEEC 5 µm voxels", capture(r"^([\d.]+ %)", sep_rel), bound_of(sep_rel)),
        ("Width-graded skin-effect R′", "2-D cross-section solve", "≤ " + skin.group(2), skin.group(1)),
        ("Spiral impedance at 1 MHz", "FastHenry (operator-run)", plain_cell(fh[2]) + " relative", "not gated in CI"),
    ]


def _drop_blocks(text):
    """Remove fenced code blocks and the VALIDATION_DROPPED paragraphs/tables."""
    text = re.sub(r"^```.*?^```[ \t]*\n?", "", text, flags=re.M | re.S)
    blocks, found = re.split(r"\n\s*\n", text), set()
    keep = []
    for b in blocks:
        hit = next((p for p in VALIDATION_DROPPED if b.lstrip().startswith(p)), None)
        if hit:
            found.add(hit)
        else:
            keep.append(b)
    missing = [p for p in VALIDATION_DROPPED if p not in found]
    if missing:
        raise KeyError(f"validation.md blocks expected to be dropped were not found: {missing}")
    return "\n\n".join(keep)


def _link_issues(html):
    """Turn remaining `#NN` / `issue #NN` in text (not code, not links) into issue links."""
    out, in_a = [], 0
    for part in re.split(r"(<code>.*?</code>|<pre>.*?</pre>|<[^>]+>)", html, flags=re.S):
        if part.startswith("<"):
            if part.startswith("<a ") or part.startswith("<a>"):
                in_a += 1
            elif part == "</a>":
                in_a -= 1
            out.append(part)
        else:
            out.append(part if in_a else _ISSUE_REF.sub(
                lambda m: f'<a href="{REPO}/issues/{m.group(1)}">{m.group(0)}</a>', part))
    return "".join(out)


def validation_body(md, bench_md, render_markdown):
    """Summary table, then the allowlisted sections of docs/validation.md.
    `render_markdown(text, src)` is build.render_markdown (passed in: build imports this module)."""
    rows = validation_summary(md, bench_md)
    o = ['<section id="what-was-checked">', '<h2>What was checked</h2>',
         table_html(["check", "independent reference", "measured agreement", "bound"], [list(r) for r in rows]),
         f'<p>Values are read from <a href="{BLOB}/docs/validation.md"><code>docs/validation.md</code></a> and '
         f'<a href="{BLOB}/docs/benchmarks.md"><code>docs/benchmarks.md</code></a> when the site is built; '
         'the build fails if one is missing.</p></section>']
    parts = []
    for h in VALIDATION_SECTIONS:
        m = re.search(rf"^## {re.escape(h)}.*$", md, re.M)
        if not m:
            raise KeyError(h)
        parts.append(m.group(0) + "\n" + section(md, h))
    text = _drop_blocks("\n\n".join(parts))
    text = "\n".join(_ISSUE_TAG.sub("", ln) if ln.startswith("#") else ln for ln in text.split("\n"))
    o.append(_link_issues(render_markdown(text, "docs/validation.md")))
    o.append('<details><summary>Reproduce</summary>'
             f'<p>The commands to regenerate every reference, the CI gate markers and the maintainer notes are in '
             f'<a href="{BLOB}/docs/validation.md"><code>docs/validation.md</code></a> on GitHub. '
             f'The FastHenry oracle comparison is <a href="{BLOB}/tools/fasthenry_compare.py"><code>tools/fasthenry_compare.py</code></a>, '
             f'run by <a href="{BLOB}/.github/workflows/fasthenry-oracle.yml"><code>fasthenry-oracle.yml</code></a>.</p></details>')
    return "\n".join(o)
