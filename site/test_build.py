"""Tests for the markdown renderer in build.py. Run: python3 -m unittest discover -s site"""
import re, subprocess, sys, unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import build, tablehtml

SRC = "docs/validation.md"


def md(text):
    return build.render_markdown(text, SRC)


class Render(unittest.TestCase):
    def test_heading_paragraph_inline(self):
        h = md("# Title `x`\n\nOne **bold** and *em*\nwrapped <b> & `a<b`.\n")
        self.assertIn('<h1 id="title-x">Title <code>x</code></h1>', h)
        self.assertIn("<p>One <strong>bold</strong> and <em>em</em> wrapped &lt;b&gt; &amp; <code>a&lt;b</code>.</p>", h)

    def test_fenced_code_is_escaped(self):
        h = md("```rust\nlet a = 1 < 2;\n```\n")
        self.assertIn('<pre><code class="language-rust">let a = 1 &lt; 2;\n</code></pre>', h)

    def test_table_escaped_pipe_and_code_pipe(self):
        h = md("| a | b |\n|---|--:|\n| `x\\|y` | `|t|` |\n")
        self.assertIn("<th>a</th>", h)
        self.assertIn('<th style="text-align:right">b</th>', h)
        self.assertIn("<td><code><span class=\"tok\">x|y</span></code></td><td style=\"text-align:right\"><code><span class=\"tok\">|t|</span></code></td>", h)

    def test_tables_use_one_wrapper_and_classify_columns(self):
        h = md("| k | n | note |\n|---|---|---|\n| a | 1 000 | " + "x" * 90 + " |\n| b | 2.5 ms | y |\n| c | — | y |\n")
        self.assertIn('<div class="tablewrap" role="region" tabindex="0" aria-label="Table: k, n, note">', h)
        self.assertIn('<th class="num">n</th>', h)
        self.assertIn('<td class="num">1 000</td>', h)
        self.assertIn('<td class="num na">—</td>', h)
        self.assertIn('<th class="prose">note</th>', h)
        self.assertNotIn("table-wrap", h)

    def test_wide_only_without_prose_and_six_columns(self):
        six = "| a | b | c | d | e | f |\n|---|---|---|---|---|---|\n| 1 | 2 | 3 | 4 | 5 | 6 |\n"
        self.assertIn('class="tablewrap wide"', md(six))
        self.assertNotIn(" wide", md("| a | b |\n|---|---|\n| 1 | 2 |\n"))

    def test_lists_with_continuation_and_nested_code(self):
        h = md("- one\n  more\n- two\n\n1. a\n\n   ```\n   x\n   ```\n2. b\n")
        self.assertIn("<ul>\n<li>one more</li>\n<li>two</li>\n</ul>", h)
        self.assertIn("<ol>", h)
        self.assertIn("<li><p>a</p>\n<pre><code>x\n</code></pre></li>", h)
        self.assertIn("<li>b</li>", h)

    def test_relative_links_become_github_urls(self):
        h = md("[o](output-formats.md) [t](../tools/fasthenry_compare.py#L1) [u](https://example.com/a.md) [a](#frag)")
        self.assertIn('href="{{repo}}/blob/main/docs/output-formats.md"', h)
        self.assertIn('href="{{repo}}/blob/main/tools/fasthenry_compare.py#L1"', h)
        self.assertIn('href="https://example.com/a.md"', h)
        self.assertIn('href="#frag"', h)

    def test_directory_link_uses_tree(self):
        self.assertIn('href="{{repo}}/tree/main/docs"', md("[d](.)"))

    def test_broken_relative_link_fails_the_build(self):
        with self.assertRaises(SystemExit):
            md("[x](no-such-file.md)")


class Classify(unittest.TestCase):
    def test_numeric_forms(self):
        for c in ("0.63746 nH", "19\u202f600", "2 497 MB", "<0.001 %", "−0.46 / +0.39", "~250", "1.7e-2", "0.5 pp (~10 %)", "**0.84**", "—"):
            self.assertTrue(tablehtml.is_numeric(c) or tablehtml.is_empty(c), c)
        for c in ("spiral", "Supported", "t/δ = 0.3", "graded(36, 10, 1.2)"):
            self.assertFalse(tablehtml.is_numeric(c), c)


class Build(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        subprocess.run([sys.executable, str(build.SITE / "build.py")], check=True, capture_output=True)

    def test_sources_render_into_pages_without_md_hrefs(self):
        subprocess.run([sys.executable, str(build.SITE / "build.py")], check=True, capture_output=True)
        for slug in ("validation", "fasthenry-users"):
            html = (build.DIST / slug / "index.html").read_text()
            self.assertIn("<table>", html)
            self.assertNotRegex(html, r'href="(?!https?://)[^"]*\.md')
            self.assertNotIn("{{", html)
        home = (build.DIST / "index.html").read_text()
        self.assertNotRegex(home, r"(Validation|FastHenry users) \(soon\)")


    def test_doc_pages_lead_with_the_doc_h1_then_a_provenance_line(self):
        for slug in ("validation", "fasthenry-users"):
            html = (build.DIST / slug / "index.html").read_text()
            main = html[html.index("<main"):]
            self.assertEqual(main.count("<h1"), 1)
            self.assertLess(main.index("<h1"), main.index('class="provenance"'))
            self.assertNotIn('class="lede"', main)
            self.assertLess(main.index('class="provenance"'), main.index("<h2"))

    def test_footer_and_header_links(self):
        html = (build.DIST / "about" / "index.html").read_text()
        for s in ("docs.rs/fasterhenry", "crates.io/crates/fasterhenry", build.crate_version(), "MIT license"):
            self.assertIn(s, html.split("<footer>")[1])
        self.assertIn('class="ghlink"', html)


class Benchmarks(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import render
        cls.render = render
        cls.doc = (build.SITE.parent / "docs/benchmarks.md").read_text()
        cls.html, cls.svg = render.benchmarks_body(cls.doc)

    def test_single_asterisk_renders_as_emphasis(self):
        self.assertEqual(self.render.inline("so *both* arms and **bold**"), "so <em>both</em> arms and <strong>bold</strong>")
        self.assertEqual(self.render.inline("2*3*4 stays"), "2*3*4 stays")
        self.assertNotRegex(self.html, r"(?<![*\w])\*[A-Za-z]")

    def test_section_order(self):
        h = self.html
        order = ["Takeaway", "Scaling: dense vs pFFT", "Speed against FastHenry",
                 "Accuracy against FastHenry", "How this was measured"]
        pos = [h.index(f"<h2>{t}") for t in order]
        self.assertEqual(pos, sorted(pos))
        self.assertLess(h.index("<figure"), h.index("<table>"))
        # the per-thread caveat sits right under the speed table
        self.assertRegex(h, r"</table></div>\n<p><strong>Per thread")

    def test_accuracy_table_is_rendered_from_the_doc(self):
        self.assertIn("relative difference", self.html)
        self.assertIn("7.3e-4", self.html)

    def test_empty_criterion_block_is_omitted(self):
        self.assertNotIn("Not yet regenerated", self.html)
        self.assertNotIn("Criterion bench suite", self.html)

    def test_criterion_section_appears_with_a_table(self):
        doc = self.doc.replace("_Not yet regenerated", "| n | t |\n|---|---|\n| 48 | 1 ms |\n\n_x")
        html, _ = self.render.benchmarks_body(doc)
        self.assertIn("<h2>Criterion bench suite</h2>", html)
        self.assertIn("1 ms", html)

    def test_chart_ticks_are_compact_and_inside_the_canvas(self):
        for t in ("1k", "10k", "100k"):
            self.assertIn(f">{t}</text>", self.svg)
        self.assertNotIn("100,000", self.svg)
        self.assertEqual(self.render.compact(100000), "100k")
        self.assertEqual(self.render.compact(10), "10")
        W = int(re.search(r'viewBox="0 0 (\d+) (\d+)"', self.svg).group(1))
        H = int(re.search(r'viewBox="0 0 (\d+) (\d+)"', self.svg).group(2))
        for x in re.findall(r'<text class="tx" x="([\d.]+)" y="[\d.]+" text-anchor="middle">\d+k?</text>', self.svg):
            self.assertLess(float(x) + 20, W)
        axis = re.search(r'<text class="tx" x="[\d.]+" y="([\d.]+)" text-anchor="middle">filaments', self.svg)
        self.assertLess(float(axis.group(1)), H)


class Validation(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import render
        cls.render = render
        cls.doc = (build.SITE.parent / "docs/validation.md").read_text()
        cls.bench = (build.SITE.parent / "docs/benchmarks.md").read_text()
        cls.html = render.validation_body(cls.doc, cls.bench, build.render_markdown)

    def test_summary_comes_first_with_values_from_the_docs(self):
        h = self.html
        self.assertLess(h.index("What was checked"), h.index("<h2 id="))
        rows = self.render.validation_summary(self.doc, self.bench)
        self.assertGreaterEqual(len(rows), 5)
        for r in rows:
            self.assertTrue(all(r), r)
        self.assertIn("4.97 %", h)
        self.assertIn("5.5 %", h)

    def test_maintainer_material_is_not_rendered(self):
        for s in ("<pre", "How CI asserts", "Margin is thin", "36162393794", "venv", "Regenerate everything"):
            self.assertNotIn(s, self.html)

    def test_issue_tags_are_dropped_from_headings_and_refs_are_links(self):
        for hd in re.findall(r"<h[23][^>]*>.*?</h[23]>", self.html):
            self.assertNotIn("issue #", hd)
        self.assertNotRegex(re.sub(r"<a [^>]*>.*?</a>|<code>.*?</code>", "", self.html), r"#\d+")

    def test_missing_summary_value_fails_the_build(self):
        with self.assertRaises(KeyError):
            self.render.validation_summary(self.doc.replace("| L vs Greenhouse |", "| L vs Gone |"), self.bench)
        with self.assertRaises(KeyError):
            self.render.validation_summary(self.doc, self.bench.replace("| spiral |", "| coil |"))

    def test_dropped_block_that_moved_fails_the_build(self):
        with self.assertRaises(KeyError):
            self.render.validation_body(self.doc.replace("**Margin is thin", "**Margin"), self.bench, build.render_markdown)


if __name__ == "__main__":
    unittest.main()


class Charts(unittest.TestCase):
    """The shared chart module and the inline figures (issue #307)."""

    @classmethod
    def setUpClass(cls):
        import charts, gen_hero
        cls.charts, cls.gen_hero = charts, gen_hero
        cls.hero = __import__("json").loads((build.SITE / "assets/hero-spiral.json").read_text())

    def test_series_colours_meet_3_to_1_on_the_background_in_both_themes(self):
        P = self.charts.PALETTE
        for i in (0, 1):
            for k in ("s1", "s2"):
                self.assertGreaterEqual(self.charts.contrast(P[k][i], P["bg"][i]), 3.0, (k, i))

    def test_site_css_tokens_match_the_palette(self):
        css = (build.SITE / "site.css").read_text()
        P = self.charts.PALETTE
        css = css.replace("--bg:#fff;", "--bg:#ffffff;")   # the palette spells it out
        light, dark = css.split("@media (prefers-color-scheme:dark){:root{")
        for key, var in (("fg", "fg"), ("muted", "muted"), ("frame", "frame"), ("grid", "line"),
                         ("s1", "copper"), ("s2", "link"), ("bg", "bg")):
            self.assertIn(f"--{var}:{P[key][0]};", light, key)
            self.assertIn(f"--{var}:{P[key][1]};", dark, key)

    def test_inline_has_no_background_or_style_and_standalone_has_both_schemes(self):
        inline = self.gen_hero.figure(self.hero)
        self.assertNotIn("<rect width=", inline)
        self.assertNotIn("<style", inline)
        self.assertNotIn("#ffffff", inline)
        alone = self.gen_hero.figure(self.hero, standalone=True)
        self.assertIn("prefers-color-scheme:dark", alone)
        self.assertNotIn("<rect width=", alone)   # transparent: no white box on a dark page

    def test_ids_are_unique_across_a_page_and_charts_are_named(self):
        for page in ("index", "gallery", "benchmarks"):
            html = (build.DIST / ("index.html" if page == "index" else f"{page}/index.html")).read_text()
            ids = re.findall(r'\bid="([^"]+)"', html)
            self.assertEqual(len(ids), len(set(ids)), page)
            for svg in re.findall(r"<svg[^>]*class=\"chart[^>]*>", html):
                self.assertIn("aria-labelledby=", svg)
                self.assertIn("aria-describedby=", svg)

    def test_units_are_in_panel_titles_not_a_rotated_label(self):
        svg = self.gen_hero.figure(self.hero)
        self.assertIn(">R, mΩ<", svg)
        self.assertIn(">L, nH<", svg)
        self.assertNotIn("rotate(", svg)

    def test_compact_text_is_at_least_11px_at_phone_width(self):
        # a 412 px phone gives a content column of about 330 px >= the 300 unit viewBox
        self.assertGreaterEqual(self.charts.COMPACT["fs"] * 330 / self.charts.COMPACT["W"], 11)
        svg = self.gen_hero.figure(self.hero, self.charts.COMPACT)
        self.assertIn('font-size="11"', svg)
        self.assertNotRegex(svg, r"font-size=\"(?:[0-9]|10)\"")

    def test_compact_text_stays_inside_the_canvas(self):
        svg = self.gen_hero.figure(self.hero, self.charts.COMPACT)
        for x, anchor in re.findall(r'<text class="tx" x="([\d.]+)" y="[\d.]+" text-anchor="(\w+)"', svg):
            self.assertLess(float(x), 300 - 18 + 1)
