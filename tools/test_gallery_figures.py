#!/usr/bin/env python3
"""Unit tests for `tools/gallery_figures.py` and `site/render.py`.

Run with:
    cd tools && python3 -m unittest test_gallery_figures
"""

import json
import pathlib
import re
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "site"))

import gallery_figures
import render


class SeriesTests(unittest.TestCase):
    def test_l_is_imag_over_omega_and_port_selects_diagonal(self) -> None:
        z = lambda re, im: {"re": re, "im": im}
        res = {"frequencies_hz": [1e6],
               "impedance_ohm": [[[z(0.05, 2 * 3.14159265358979 * 1e6 * 1e-8), z(0, 0)], [z(0, 0), z(9, 9)]]]}
        s = gallery_figures.series_from_result(res)
        self.assertAlmostEqual(s[0]["l_h"], 1e-8, places=14)
        self.assertEqual(s[0]["r_ohm"], 0.05)


class ManifestFieldTests(unittest.TestCase):
    def test_every_deck_has_meta_and_titles_are_not_filenames(self) -> None:
        for name in gallery_figures.DECKS:
            d = gallery_figures.derived(name, "")
            self.assertTrue(d["title"].strip() and d["title"] != name and "_" not in d["title"], name)
            self.assertEqual(d["geometry"], f"{name}_geometry.svg")
        self.assertEqual(set(gallery_figures.META), set(gallery_figures.DECKS))

    def test_variants_point_at_a_real_entry_with_a_summary(self) -> None:
        for name, m in gallery_figures.META.items():
            if "variant_of" in m:
                self.assertIn(m["variant_of"], gallery_figures.DECKS)
                self.assertNotIn("variant_of", gallery_figures.META[m["variant_of"]])
                self.assertTrue(m["variant"])
            else:
                self.assertTrue(m["summary"].strip(), name)

    def test_committed_manifest_has_the_new_fields_and_no_deck_comments(self) -> None:
        data = json.loads((ROOT / "site/assets/gallery/gallery.json").read_text())
        for e in data["entries"]:
            self.assertNotIn("description", e)
            self.assertEqual(e["title"], gallery_figures.derived(e["id"], "")["title"])
            self.assertTrue((ROOT / "site/assets/gallery" / e["geometry"]).exists())
            if "variant_of" not in e:
                self.assertEqual(e["summary"], gallery_figures.META[e["id"]]["summary"])


class GeometryTests(unittest.TestCase):
    def deck(self, name):
        return gallery_figures.parse_geometry((ROOT / gallery_figures.DATA / f"{name}.inp").read_text())

    def test_spiral_nodes_in_mm(self) -> None:
        g = self.deck("spiral")
        self.assertEqual(len(g["elements"]), 8)
        self.assertEqual(g["nodes"]["n8"][:2], (-2.0, -2.0))
        self.assertAlmostEqual(g["elements"][0][2], 0.2)   # trace width, mm

    def test_plane_syntaxes_agree_on_geometry(self) -> None:
        a, b = self.deck("plane_fasthenry")["planes"][0], self.deck("plane_extension")["planes"][0]
        self.assertEqual(a["rect"], (0.0, 0.0, 10.0, 6.0))
        for key in ("rect", "holes", "contacts"):
            self.assertEqual(a[key], b[key])
        self.assertEqual(self.deck("plane_decay_rect")["planes"][0]["contacts"],
                         self.deck("plane_decay_extension")["planes"][0]["contacts"])

    def test_sketch_is_valid_looking_svg_with_ports(self) -> None:
        out = gallery_figures.geometry_svg(self.deck("plane_fasthenry"), "A & B")
        self.assertTrue(out.startswith("<svg") and "A &amp; B" in out and not re.search(r"\bnan\b", out))
        self.assertEqual(out.count("<circle"), 2)


class FigureTests(unittest.TestCase):
    def entry(self, rs):
        return {"title": "t & u", "series": [{"f_hz": 10 ** (6 + i / 10), "r_ohm": r, "l_h": 1e-8} for i, r in enumerate(rs)]}

    def test_flat_curve_does_not_divide_by_zero(self) -> None:
        out = gallery_figures.svg(self.entry([0.05] * 31))
        self.assertTrue(out.startswith("<svg") and not re.search(r"\bnan\b", out))

    def test_small_change_is_not_drawn_as_a_large_one(self) -> None:
        lo, hi = gallery_figures.axis_range([1.548, 1.552])
        self.assertGreaterEqual(hi - lo, gallery_figures.MIN_REL_SPAN * 1.552 - 1e-12)
        lo, hi = gallery_figures.axis_range([10.0, 20.0])
        self.assertLess(hi - lo, 0.2 * 20 * 3)   # a real transition still fills the panel

    def test_title_is_escaped(self) -> None:
        self.assertIn("t &amp; u", gallery_figures.svg(self.entry([0.05 + i * 1e-3 for i in range(31)])))


class GalleryPageTests(unittest.TestCase):
    machine = {"cpu_count": 2, "platform": "p", "runs_per_deck": 3}

    def ok(self, id="a", **kw):
        e = {"id": id, "title": "A", "summary": "About a.", "geometry": f"{id}_geometry.svg", "deck": "fasterhenry-cli/tests/data/spiral.inp",
             "command": f"fasterhenry d/{id}.inp", "status": "ok", "wall_time_s": 0.0123, "counts": {"filaments": 5},
             "ports": 1, "fasterhenry_version": "0.2.0", "overlay": None,
             "series": [{"f_hz": 1e6, "r_ohm": 0.05, "l_h": 1e-8}, {"f_hz": 1e9, "r_ohm": 0.06, "l_h": 9e-9}]}
        e.update(kw)
        return e

    def test_failed_deck_is_reported_and_has_no_plot(self) -> None:
        e = {"id": "bad", "title": "Bad", "summary": "", "geometry": "bad_geometry.svg", "deck": "fasterhenry-cli/tests/data/spiral.inp",
             "command": "c", "status": "failed", "error": "parse error <x>"}
        html = render.gallery_body({"machine": self.machine, "entries": [e]})
        self.assertIn("did not run", html)
        self.assertIn("parse error &lt;x&gt;", html)
        self.assertNotIn("/assets/gallery/bad.svg", html)

    def test_card_has_title_summary_result_plot_and_closed_details(self) -> None:
        html = render.gallery_body({"machine": self.machine, "entries": [self.ok()]})
        self.assertIn("/assets/gallery/a.svg", html)             # linked as a standalone download
        self.assertIn('<svg xmlns="http://www.w3.org/2000/svg" viewBox', html)   # and drawn inline
        self.assertNotIn("<img", html)
        self.assertIn('id="g-a-t"', html)
        self.assertIn("12.3&nbsp;ms", html)
        self.assertIn("About a.", html)
        self.assertIn("R 50 to 60&nbsp;m&Omega; (+20%)", html)    # computed from the series
        self.assertIn("L 10 to 9&nbsp;nH (-10%)", html)
        self.assertIn("<details><summary>", html)
        self.assertNotIn("<details open", html)
        self.assertIn("fasterhenry d/a.inp", html)
        self.assertIn("About these figures", html)
        self.assertIn("small fixtures", html)

    def test_variants_fold_into_one_card(self) -> None:
        v = self.ok("b", variant_of="a", variant="other syntax", title="A")
        html = render.gallery_body({"machine": self.machine, "entries": [self.ok(), v]})
        self.assertEqual(html.count('class="card"'), 1)
        self.assertIn("other syntax", html)
        self.assertIn("fasterhenry d/b.inp", html)

    def test_pct_precision(self) -> None:
        self.assertEqual(render.pct(1.548, 1.5519), "+0.25%")
        self.assertEqual(render.pct(10, 11.4), "+14%")

    def test_committed_gallery_json_is_consistent(self) -> None:
        data = json.loads((ROOT / "site/assets/gallery/gallery.json").read_text())
        for e in data["entries"]:
            if e["status"] == "ok":
                self.assertTrue((ROOT / "site/assets/gallery" / f'{e["id"]}.svg').exists())
            self.assertTrue((ROOT / e["deck"]).exists())
        html = render.gallery_body(data)
        self.assertEqual(html.count('class="card"'), sum("variant_of" not in e for e in data["entries"]))


class BenchmarkPageTests(unittest.TestCase):
    md = """## Setup

- **Hardware**: A box,
  8 cores.
- **Other**: x

## Speed

| deck | t |
|---|---|
| a | **1 ms** |

- **Per thread, FastHenry is single-threaded.** Our dense wall-clock advantage is 3x on this fixture. More text.

## Accuracy (the replacement claim)

Same decks, both engines.

| deck | delta |
|---|---|
| a | 0.01 % |

Sub-0.1 % engine agreement on every deck.

## Committed criterion bench suite

<!-- bench-table:begin -->
_Not yet regenerated by CI._
<!-- bench-table:end -->

## Scaling: dense vs pFFT + GMRES

- **Hardware**: Cloud, 1 thread.

| filaments | loops | dense s | iterative s |
|---|---|---|---|
| 960 | 720 | **0.84** | 1.71 |
| 29 928 | 22 446 | not attempted | **60.9** |

`DENSE_PATH_MAX_FILAMENTS` is **10 000**: and the wall-clock crossover is near 3 000 filaments.
"""

    def test_section_stops_at_next_h2(self) -> None:
        self.assertNotIn("Scaling", render.section(self.md, "Speed"))

    def test_number_parsing(self) -> None:
        self.assertEqual(render.number("**0.84**"), 0.84)
        self.assertEqual(render.number("29 928"), 29928.0)
        self.assertIsNone(render.number("not attempted"))

    def test_scaling_points_skip_unattempted_dense(self) -> None:
        pts = render.scaling_points(render.section(self.md, "Scaling"))
        self.assertEqual(pts, [(960.0, 0.84, 1.71), (29928.0, None, 60.9)])

    def test_page_renders_placeholder_honestly_and_no_invented_table(self) -> None:
        body, svg = render.benchmarks_body(self.md)
        # With no generated table the renderer omits the criterion section rather than showing the placeholder.
        self.assertNotIn("Not yet regenerated by CI", body)
        self.assertIn("<strong>1 ms</strong>", body)
        self.assertIn("A box, 8 cores.", body)
        self.assertEqual(body.count("<table>"), 3)  # accuracy + speed + scaling; none invented for criterion
        self.assertIn("Sub-0.1 % engine agreement on every deck.", body)
        self.assertIn("default handover", svg)

    def test_real_docs_render(self) -> None:
        body, svg = render.benchmarks_body((ROOT / "docs/benchmarks.md").read_text())
        self.assertIn("<table>", body)
        self.assertIn("<polyline", svg)


if __name__ == "__main__":
    unittest.main()
