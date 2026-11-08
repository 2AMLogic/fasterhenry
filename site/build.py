#!/usr/bin/env python3
"""Build the fasterhenry.org static site into site/dist (stdlib only).

    python3 site/build.py

Pages are site/pages/*.html fragments carrying `<!-- title: ... -->` and
`<!-- description: ... -->` headers; layout.html wraps them. `index.html`
becomes /index.html, any other `name.html` becomes /name/index.html. The hero
figure under site/assets/ is committed (regenerate: site/gen_hero.py).

NAV is the whole navigation. Pages that sibling issues will add are listed
with ready=False: they render as plain text ("soon") until their fragment
exists, then turn into links automatically.

Copy rule: public wording follows the 2AMLogic/marketing POSITIONING.md (see
the About quote was checked against marketing commit d3061f8, blob 7da7ca3e).
Do not copy or paraphrase that file into this repository.
"""
import json, re, shutil, sys
from html import escape
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import render

SITE = Path(__file__).resolve().parent
DIST = SITE / "dist"
REPO = "https://github.com/2AMLogic/fasterhenry"
NAV = [  # (label, slug)
    ("Gallery", "gallery"), ("Benchmarks", "benchmarks"), ("Validation", "validation"),
    ("FastHenry users", "fasthenry-users"), ("About", "about"),
]


def slug_url(slug):
    return "/" if slug == "" else f"/{slug}/"


def nav_html(current, built):
    items = []
    for label, slug in NAV:
        if slug not in built:
            items.append(f'<li><span class="soon">{escape(label)} (soon)</span></li>')
        elif slug == current:
            items.append(f'<li><a href="{slug_url(slug)}" aria-current="page">{escape(label)}</a></li>')
        else:
            items.append(f'<li><a href="{slug_url(slug)}">{escape(label)}</a></li>')
    return "\n".join(items)


def parse(path):
    text = path.read_text()
    meta = dict(re.findall(r"<!--\s*(title|description):\s*(.*?)\s*-->", text))
    body = re.sub(r"<!--\s*(?:title|description):.*?-->\n?", "", text)
    for k in ("title", "description"):
        if k not in meta:
            sys.exit(f"{path}: missing <!-- {k}: ... --> header")
    return meta, body


def main():
    hero = json.loads((SITE / "assets/hero-spiral.json").read_text())
    subs = {"repo": REPO, "hero_version": hero["fasterhenry_version"],
            "hero_filaments": str(hero["filaments"]), "hero_command": escape(hero["command"])}
    # Gallery: committed output of tools/gallery_figures.py. Benchmarks: rendered
    # from docs/benchmarks.md, scaling plot drawn from its own table.
    subs["gallery_body"] = render.gallery_body(json.loads((SITE / "assets/gallery/gallery.json").read_text()))
    subs["benchmarks_body"], scaling_svg = render.benchmarks_body((SITE.parent / "docs/benchmarks.md").read_text())
    pages = {("" if p.stem == "index" else p.stem): p for p in sorted((SITE / "pages").glob("*.html"))}
    layout = (SITE / "layout.html").read_text()
    if DIST.exists():
        shutil.rmtree(DIST)
    DIST.mkdir()
    for slug, path in pages.items():
        meta, body = parse(path)
        out = layout
        fill = {**subs, "title": escape(meta["title"]), "description": escape(meta["description"], quote=True),
                "nav": nav_html(slug, set(pages)), "body": body}
        # body first so its own {{repo}} etc. are expanded by the second pass
        for _ in range(2):
            out = re.sub(r"\{\{(\w+)\}\}", lambda m: fill[m.group(1)], out)
        target = DIST / slug / "index.html" if slug else DIST / "index.html"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(out)
    shutil.copy(SITE / "site.css", DIST / "site.css")
    shutil.copytree(SITE / "assets", DIST / "assets")
    (DIST / "assets/benchmarks").mkdir()
    (DIST / "assets/benchmarks/scaling.svg").write_text(scaling_svg + "\n")
    (DIST / "_headers").write_text("/*\n  X-Content-Type-Options: nosniff\n  Referrer-Policy: strict-origin-when-cross-origin\n/assets/*\n  Cache-Control: public, max-age=3600\n")
    left = [m for f in DIST.rglob("*.html") for m in re.findall(r"\{\{\w+\}\}", f.read_text())]
    if left:
        sys.exit(f"unexpanded placeholders: {sorted(set(left))}")
    print(f"built {len(pages)} page(s) to {DIST}", file=sys.stderr)


if __name__ == "__main__":
    main()
