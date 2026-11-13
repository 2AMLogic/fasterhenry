"""Page bodies for the Gallery and Benchmarks pages (stdlib only).

Imported by build.py. Nothing here is hand-typed data:

  * the gallery comes from site/assets/gallery/gallery.json (tools/gallery_figures.py)
  * the benchmarks page is rendered from docs/benchmarks.md, and its scaling
    plot is drawn from that file's own "Scaling: dense vs pFFT + GMRES" table

Inline markdown, table-cell splitting and issue links come from
site/mdinline.py, the module build.py renders its own markdown sources with, so
a link or a code span holding a `|` renders the same here as it does there.
"""
import json, math, re
from html import escape
from pathlib import Path

import charts
import mdinline
import tablehtml

REPO = "https://github.com/2AMLogic/fasterhenry"
BLOB = f"{REPO}/blob/main"
# Every markdown span this module renders comes from one of these two docs.
BENCH_SRC = "docs/benchmarks.md"
VALIDATION_SRC = "docs/validation.md"


# ---- gallery ---------------------------------------------------------------

def pct(a, b):
    """Signed relative change a -> b, e.g. '+20%', '-1.4%', '+0.25%'."""
    p = (b - a) / a * 100
    return f"{p:+.0f}%" if abs(p) >= 10 else f"{p:+.1f}%" if abs(p) >= 1 else f"{p:+.2f}%"


def result_line(e):
    """R and L at the two ends of the sweep, with their relative change, from the series."""
    s = e["series"]
    r0, r1, l0, l1 = s[0]["r_ohm"] * 1e3, s[-1]["r_ohm"] * 1e3, s[0]["l_h"] * 1e9, s[-1]["l_h"] * 1e9
    f = lambda hz: f'{hz / 1e6:g}&nbsp;MHz' if hz < 1e9 else f'{hz / 1e9:g}&nbsp;GHz'
    return (f'R {r0:.4g} to {r1:.4g}&nbsp;m&Omega; ({pct(r0, r1)}), L {l0:.4g} to {l1:.4g}&nbsp;nH ({pct(l0, l1)}), '
            f'from {f(s[0]["f_hz"])} to {f(s[-1]["f_hz"])}')


def _gf():
    """tools/gallery_figures.py, which owns the gallery drawings and their text."""
    import sys
    tools = str(Path(__file__).resolve().parent.parent / "tools")
    if tools not in sys.path:
        sys.path.insert(0, tools)
    import gallery_figures
    return gallery_figures


def rl_inline(e):
    """Compact R/L drawing (cards are narrow) inlined, ids unique per card."""
    gf = _gf()
    return gf.svg(e, charts.COMPACT, False, f'g-{e["id"]}')


def geometry_inline(e):
    gf = _gf()
    text = (Path(__file__).resolve().parent.parent / e["deck"]).read_text()
    return gf.geometry_svg(gf.parse_geometry(text), e["title"], f'g-{e["id"]}-geo', False)


def gallery_body(data):
    m = data["machine"]
    entries = data["entries"]
    variants = {}
    for e in entries:
        if e.get("variant_of"):
            variants.setdefault(e["variant_of"], []).append(e)
    out = ['<p class="lede">Each card is a deck committed in this repository, run through fasterhenry, '
           'with its conductors drawn from the deck itself.</p>',
           '<ul class="cards">']
    for e in (e for e in entries if not e.get("variant_of")):
        group = [e] + variants.get(e["id"], [])
        out.append(f'<li class="card" id="{escape(e["id"])}">\n<h2>{escape(e["title"])}</h2>')
        if e.get("summary"):
            out.append(f'<p>{escape(e["summary"])}</p>')
        out.append(f'<div class="fig">{geometry_inline(e)}</div>')
        if e["status"] != "ok":
            out.append(f'<p><strong>This deck did not run.</strong> The CLI reported:</p><pre><code>{escape(e["error"])}</code></pre>')
        else:
            c = e["counts"]
            out.append(f'<p class="result">{result_line(e)}</p>')
            out.append(f'<div class="fig">{rl_inline(e)}</div>')
            out.append(f'<p class="stats">{c["filaments"]} filaments &middot; {e["ports"]} port(s) &middot; '
                       f'wall time {e["wall_time_s"] * 1e3:.1f}&nbsp;ms</p>')
        out.append('<details><summary>Deck and command</summary>')
        if e["status"] == "ok":
            out.append(f'<p>Standalone figures: <a href="/assets/gallery/{escape(e["id"])}.svg">R and L</a>, '
                       f'<a href="/assets/gallery/{escape(e["geometry"])}">geometry</a> (SVG, light or dark to match your system).</p>')
        if len(group) > 1:
            out.append(f'<p>This plane is committed in {len(group)} deck variants (two contact-refinement regions, '
                       f'each in two syntaxes); the two syntaxes of a region give the same result.</p>')
        for g in group:
            out.append('<p>' + (f'{escape(g["variant"])}<br>' if g.get("variant") else "") +
                       (f'{result_line(g)}<br>' if g["status"] == "ok" and len(group) > 1 else "") +
                       f'Deck: <a href="{BLOB}/{escape(g["deck"])}"><code>{escape(g["deck"])}</code></a><br>'
                       f'Command: <code>{escape(g["command"])}</code></p>')
        out.append('</details>\n</li>')
    out.append('</ul>')
    fil = [e["counts"]["filaments"] for e in entries if e["status"] == "ok"]
    size = f' (between {min(fil)} and {max(fil)} filaments)' if fil else ""
    out.append(f'<p>These are small fixtures{size}, chosen to be readable rather than large. '
               'For how run time scales to bigger problems, see <a href="/benchmarks/">Benchmarks</a>.</p>')
    out.append('<h2 id="about">About these figures</h2>')
    out.append(f'<p>Every figure is the output of <code>tools/gallery_figures.py</code> run on a deck committed in '
               f'<a href="{BLOB}/fasterhenry-cli/tests/data">fasterhenry-cli/tests/data</a>. Plots show port 1&rsquo;s '
               f'self-impedance, 1&nbsp;MHz to 1&nbsp;GHz, 10 points per decade. The vertical axes span at least 20% of '
               f'the plotted value, so a change of a fraction of a percent is not drawn as a large one; the change is '
               f'also stated in the line above each plot. No FastHenry curve is overlaid on any entry: '
               f'this repository does not commit FastHenry output.</p>')
    out.append(f'<p>Wall time is the median process wall clock of {m["runs_per_deck"]} runs on the machine that generated the '
               f'figures ({m["cpu_count"]} logical CPUs, {escape(m["platform"])}). It is a per-run timing, not a benchmark; see '
               f'<a href="/benchmarks/">Benchmarks</a>.</p>')
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
            cur.append(mdinline.cells(ln))
        elif cur:
            if len(cur) >= 2 and all(re.fullmatch(r":?-+:?", c) for c in cur[1]):
                out.append((cur[0], cur[2:]))
            cur = []
    return out


def table_html(header, rows, src=BENCH_SRC):
    return tablehtml.render_table(header, rows, lambda c: mdinline.inline(c, src))


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
    return charts.compact_count(n)


SCALING_TITLE = "Dense versus pFFT + GMRES wall clock against filament count"
SCALING_DESC = ("Log-log plot from the scaling table in docs/benchmarks.md. The dense path grows faster than "
                "the iterative path and stops where it does not fit in memory.")


def scaling_svg(points, markers, layout=charts.WIDE, standalone=True, uid="scaling"):
    """Log-log wall clock vs filaments; `markers` = [(x, label)] vertical guides.
    Default is the standalone wide file build.py writes to assets/benchmarks/."""
    return charts.scaling_figure(points, markers, SCALING_TITLE, SCALING_DESC, uid, layout, standalone)


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
         f'<blockquote><p>{mdinline.inline(agree, BENCH_SRC)}</p><p>{mdinline.inline(advantage, BENCH_SRC)}</p></blockquote>',
         f'<p>Quoted from {doc}, which is where every number on this page comes from.</p>']

    o.append('<h2>Scaling: dense vs pFFT + GMRES</h2>')
    o.append('<figure class="chartfig">' + charts.both_layouts(
                 lambda layout, uid: scaling_svg(pts, markers, layout, False, uid), "scaling") +
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
    o.append(f'<p>{mdinline.inline(caveat, BENCH_SRC)}</p>')
    o.append(f'<p>Operator-run, on one machine; method and caveats are in <a href="{BLOB}/docs/benchmarks.md#speed">docs/benchmarks.md</a>. '
             'CI never runs FastHenry.</p>')

    o.append('<h2>Accuracy against FastHenry</h2>')
    lead = re.search(r"^Same decks.*?(?=\n\s*\n)", acc, re.M | re.S)
    if lead:
        o.append(f'<p>{mdinline.inline(" ".join(lead.group(0).split()), BENCH_SRC)}</p>')
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
        o.append(f'<li><strong>FastHenry comparison and the criterion suite:</strong> {mdinline.inline(hw, BENCH_SRC)}</li>')
    if hw2:
        o.append(f'<li><strong>Scaling run:</strong> {mdinline.inline(hw2, BENCH_SRC)}</li>')
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


def validation_body(md, bench_md, render_markdown):
    """Summary table, then the allowlisted sections of docs/validation.md.
    `render_markdown(text, src)` is build.render_markdown (passed in: build imports this module)."""
    rows = validation_summary(md, bench_md)
    o = ['<section id="what-was-checked">', '<h2>What was checked</h2>',
         table_html(["check", "independent reference", "measured agreement", "bound"],
                    [list(r) for r in rows], VALIDATION_SRC),
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
    o.append(mdinline.link_issues(render_markdown(text, VALIDATION_SRC)))
    o.append('<details><summary>Reproduce</summary>'
             f'<p>The commands to regenerate every reference, the CI gate markers and the maintainer notes are in '
             f'<a href="{BLOB}/docs/validation.md"><code>docs/validation.md</code></a> on GitHub. '
             f'The FastHenry oracle comparison is <a href="{BLOB}/tools/fasthenry_compare.py"><code>tools/fasthenry_compare.py</code></a>, '
             f'run by <a href="{BLOB}/.github/workflows/fasthenry-oracle.yml"><code>fasthenry-oracle.yml</code></a>.</p></details>')
    return "\n".join(o)
