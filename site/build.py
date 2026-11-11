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

A fragment whose body comes from a placeholder (benchmarks) names its document
with `<!-- origin: docs/x.md -->`; with `source` this yields the page's one
"Source: docs/x.md" footer line. Every page also ends with a "Next" link that
follows NAV (see page_sequence), and any page with TOC_MIN or more h2/h3
headings gets an "On this page" contents list (add_toc) built from the final
HTML, so placeholder-supplied bodies are covered too.

A fragment may also carry `<!-- source: docs/x.md -->`: that markdown file is
rendered (render_markdown, a small stdlib renderer for what the docs use:
headings, paragraphs, fenced code, tables, lists, inline code, links,
emphasis) and appended after the fragment's own HTML, so the docs stay the
single source of truth. Relative links in a source become
{{repo}}/blob/main/<path> URLs; a link to a file that does not exist fails the
build.

Copy rule: public wording follows the 2AMLogic/marketing POSITIONING.md (see
the About quote was checked against marketing commit d3061f8, blob 7da7ca3e).
Do not copy or paraphrase that file into this repository.
"""
import json, posixpath, re, shutil, sys
from html import escape, unescape
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import render
import tablehtml

SITE = Path(__file__).resolve().parent
ROOT = SITE.parent
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


TOC_MIN = 8  # h2/h3 headings at which a page gets an "On this page" list
_HEADING = re.compile(r"<h([23])((?:\s[^>]*)?)>(.*?)</h\1>", re.S)


def page_sequence(built):
    """The one path through the site: home, then NAV order, skipping unbuilt pages."""
    return [("Home", "")] + [(label, slug) for label, slug in NAV if slug in built]


def next_html(current, built):
    seq = page_sequence(built)
    slugs = [slug for _, slug in seq]
    if current not in slugs or slugs.index(current) == len(seq) - 1:
        return ""
    label, slug = seq[slugs.index(current) + 1]
    return f'<p class="next"><a href="{slug_url(slug)}" rel="next"><span>Next</span> {escape(label)}</a></p>\n'


def source_html(meta):
    src = meta.get("source") or meta.get("origin")
    if not src:
        return ""
    return f'<p class="source">Source: <a href="{{{{repo}}}}/blob/main/{escape(src, quote=True)}"><code>{escape(src)}</code></a></p>\n'


def page_list_html(built):
    """Every built page, for the 404 page (derived from NAV so it cannot drift)."""
    return "<ul class=\"links\">\n" + "\n".join(
        f'<li><a href="{slug_url(slug)}">{escape(label)}</a></li>' for label, slug in NAV if slug in built) + "\n</ul>"


def add_toc(body):
    """Give every h2/h3 an id and, at TOC_MIN or more, put an 'On this page' nav after the h1.

    Returns (body, has_toc). One <nav>: a closed <details> below the wide breakpoint,
    a plain list in a sticky rail above it (site.css picks one; no JavaScript)."""
    seen = set(re.findall(r'\bid="([^"]*)"', body))
    entries = []

    def tag(m):
        lvl, attrs, inner = m.group(1), m.group(2), m.group(3)
        text = re.sub(r"<[^>]+>", "", inner)
        idm = re.search(r'\bid="([^"]*)"', attrs)
        if idm:
            hid = idm.group(1)
        else:
            hid = _slug(unescape(text), seen)
            attrs += f' id="{hid}"'
        entries.append((lvl, hid, text))
        return f"<h{lvl}{attrs}>{inner}</h{lvl}>"

    body = _HEADING.sub(tag, body)
    if len(entries) < TOC_MIN or "</h1>" not in body:
        return body, False
    items = "\n".join(f'<li class="l{lvl}"><a href="#{hid}">{text}</a></li>' for lvl, hid, text in entries)
    toc = ('<nav class="toc" aria-label="On this page">\n'
           f'<details><summary>On this page</summary>\n<ol>\n{items}\n</ol>\n</details>\n'
           f'<div class="toc-rail"><p>On this page</p>\n<ol>\n{items}\n</ol></div>\n</nav>\n')
    head, _, rest = body.partition("</h1>")
    return head + "</h1>\n" + toc + rest.lstrip("\n"), True


def parse(path):
    text = path.read_text()
    meta = dict(re.findall(r"<!--\s*(title|description|source|origin):\s*(.*?)\s*-->", text))
    body = re.sub(r"<!--\s*(?:title|description|source|origin):.*?-->\n?", "", text)
    for k in ("title", "description"):
        if k not in meta:
            sys.exit(f"{path}: missing <!-- {k}: ... --> header")
    if "source" in meta:
        src = ROOT / meta["source"]
        if not src.is_file():
            sys.exit(f"{path}: source {meta['source']} does not exist")
        body = doc_page(body, render_markdown(src.read_text(), meta["source"]))
    return meta, body


def doc_page(note, doc_html):
    """Doc-derived page: the document's own H1 first, then the fragment's note as a
    small muted provenance line, then the rest of the document (one H1 per page)."""
    m = re.match(r"<h1 [^>]*>.*?</h1>\n?", doc_html, re.S)
    head, rest = (m.group(0), doc_html[m.end():]) if m else ("", doc_html)
    note = note.replace('<p class="lede">', '<p class="provenance">', 1)
    return head + note + rest


# ---- markdown rendering (stdlib only; covers what docs/*.md use) -----------

_SCHEME = re.compile(r"^[A-Za-z][A-Za-z0-9+.-]*:|^//")
_FENCE = re.compile(r"^(\s*)```(.*)$")
_ITEM = re.compile(r"^(\s*)([-*+]|\d+\.)\s+(.*)$")
_SEP = re.compile(r"^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$")


def rewrite_link(url, src):
    """Relative link in `src` (repo-relative path) -> GitHub URL under {{repo}}."""
    if url.startswith("#") or _SCHEME.match(url):
        return url
    target, _, frag = url.partition("#")
    rel = posixpath.normpath(posixpath.join(posixpath.dirname(src), target))
    if rel.startswith("..") or not (ROOT / rel).exists():
        sys.exit(f"{src}: relative link '{url}' does not resolve to a file in this repository")
    kind = "tree" if (ROOT / rel).is_dir() else "blob"
    return f"{{{{repo}}}}/{kind}/main/{rel}" + (f"#{frag}" if frag else "")


def inline(text, src):
    stash = []

    def keep(html):
        stash.append(html)
        return f"\x00{len(stash) - 1}\x00"

    text = re.sub(r"(`+)(.+?)\1", lambda m: keep(f"<code>{escape(m.group(2).strip())}</code>"), text)
    text = re.sub(r"\\([\\`*_{}\[\]()#+\-.!|<>])", lambda m: keep(escape(m.group(1))), text)
    text = escape(text, quote=False)
    text = re.sub(r"\[([^\]]+)\]\(([^)\s]+)\)",
                  lambda m: keep(f'<a href="{escape(rewrite_link(m.group(2), src), quote=True)}">') + m.group(1) + keep("</a>"),
                  text)
    text = re.sub(r"\*\*(?=\S)(.+?)(?<=\S)\*\*", r"<strong>\1</strong>", text)
    text = re.sub(r"(?<![\w*])\*(?=[^\s*])(.+?)(?<=[^\s*])\*(?![\w*])", r"<em>\1</em>", text)
    return re.sub(r"\x00(\d+)\x00", lambda m: stash[int(m.group(1))], text)


def _cells(row):
    """Split a table row on `|` outside code spans (the docs write `|tan θ|` unescaped)."""
    row = row.strip()
    cells, cur = [], []
    for m in re.finditer(r"(`+).+?\1|\\.|\||[^`\\|]+|.", row):
        tok = m.group(0)
        if tok == "|":
            cells.append("".join(cur))
            cur = []
        else:
            cur.append(tok)
    cells.append("".join(cur))
    if cells and cells[0].strip() == "":
        cells = cells[1:]
    if cells and cells[-1].strip() == "":
        cells = cells[:-1]
    return [c.strip().replace("\\|", "|") for c in cells]


def _slug(text, seen):
    base = re.sub(r"[^a-z0-9 _-]", "", re.sub(r"[`*]", "", text).lower()).strip().replace(" ", "-") or "section"
    slug, n = base, 1
    while slug in seen:
        n += 1
        slug = f"{base}-{n}"
    seen.add(slug)
    return slug


def _starts_block(line):
    m = _ITEM.match(line)
    return bool(_FENCE.match(line) or line.startswith("#") or (m and m.group(2) in ("-", "*", "+", "1.")))


def _blocks(lines, src, seen):
    out, i, n = [], 0, len(lines)
    while i < n:
        line = lines[i]
        if not line.strip():
            i += 1
            continue
        fm = _FENCE.match(line)
        if fm:
            code, i = [], i + 1
            while i < n and not _FENCE.match(lines[i]):
                code.append(lines[i][min(len(fm.group(1)), len(lines[i]) - len(lines[i].lstrip())):])
                i += 1
            i += 1
            lang = fm.group(2).strip().split(" ")[0]
            cls = f' class="language-{escape(lang)}"' if lang else ""
            out.append(f"<pre><code{cls}>{escape(chr(10).join(code))}\n</code></pre>")
            continue
        hm = re.match(r"^(#{1,6})\s+(.*?)\s*#*\s*$", line)
        if hm:
            lvl, title = len(hm.group(1)), hm.group(2)
            out.append(f'<h{lvl} id="{_slug(title, seen)}">{inline(title, src)}</h{lvl}>')
            i += 1
            continue
        if line.lstrip().startswith("|") and i + 1 < n and _SEP.match(lines[i + 1]) and "|" in lines[i + 1]:
            head, aligns = _cells(line), []
            for c in _cells(lines[i + 1]):
                aligns.append("center" if c.startswith(":") and c.endswith(":") else "right" if c.endswith(":") else "")
            i += 2
            rows = []
            while i < n and lines[i].lstrip().startswith("|"):
                rows.append(_cells(lines[i]))
                i += 1

            out.append(tablehtml.render_table(head, rows, lambda c: inline(c, src), aligns))
            continue
        im = _ITEM.match(line)
        if im:
            ordered = im.group(2)[0].isdigit()
            items = []
            while i < n:
                im = _ITEM.match(lines[i])
                if not im or im.group(1) or im.group(2)[0].isdigit() != ordered:
                    break
                width = len(lines[i]) - len(im.group(3))
                item, i = [im.group(3)], i + 1
                while i < n:
                    cur = lines[i]
                    if not cur.strip():
                        j = i
                        while j < n and not lines[j].strip():
                            j += 1
                        if j < n and (len(lines[j]) - len(lines[j].lstrip())) >= 2:
                            item.append("")
                            i += 1
                            continue
                        break
                    if cur.startswith("  "):
                        item.append(cur[min(width, len(cur) - len(cur.lstrip())):])
                    elif _ITEM.match(cur) or _FENCE.match(cur) or cur.startswith("#"):
                        break
                    else:  # lazy paragraph continuation
                        item.append(cur)
                    i += 1
                body = _blocks(item, src, seen)
                if len(body) >= 1 and body[0].startswith("<p>") and len(body) == 1:
                    body = [body[0][3:-4]]
                items.append("<li>" + "\n".join(body) + "</li>")
                nxt = _ITEM.match(lines[i + 1]) if i + 1 < n else None
                if i < n and not lines[i].strip() and nxt and not nxt.group(1) and nxt.group(2)[0].isdigit() == ordered:
                    i += 1
            tag = "ol" if ordered else "ul"
            out.append(f"<{tag}>\n" + "\n".join(items) + f"\n</{tag}>")
            continue
        para = [line.strip()]
        i += 1
        while i < n and lines[i].strip() and not _starts_block(lines[i]):
            para.append(lines[i].strip())
            i += 1
        out.append("<p>" + inline(" ".join(para), src) + "</p>")
    return out


def render_markdown(text, src):
    """Render markdown `text` read from repo path `src` to an HTML fragment."""
    return "\n".join(_blocks(text.expandtabs(4).split("\n"), src, set())) + "\n"


def crate_version():
    m = re.search(r'^version\s*=\s*"([^"]+)"', (ROOT / "Cargo.toml").read_text(), re.M)
    return m.group(1) if m else sys.exit("Cargo.toml: no version found")


def main():
    hero = json.loads((SITE / "assets/hero-spiral.json").read_text())
    subs = {"repo": REPO, "version": crate_version(), "hero_version": hero["fasterhenry_version"],
            "hero_filaments": str(hero["filaments"]), "hero_command": escape(hero["command"])}
    # Gallery: committed output of tools/gallery_figures.py. Benchmarks: rendered
    # from docs/benchmarks.md, scaling plot drawn from its own table.
    subs["gallery_body"] = render.gallery_body(json.loads((SITE / "assets/gallery/gallery.json").read_text()))
    subs["benchmarks_body"], scaling_svg = render.benchmarks_body((SITE.parent / "docs/benchmarks.md").read_text())
    pages = {("" if p.stem == "index" else p.stem): p for p in sorted((SITE / "pages").glob("*.html"))}
    layout = (SITE / "layout.html").read_text()
    subs["page_list"] = page_list_html(set(pages))
    if DIST.exists():
        shutil.rmtree(DIST)
    DIST.mkdir()
    for slug, path in pages.items():
        meta, body = parse(path)
        out = layout
        # {{body}} placeholders (gallery, benchmarks, page_list) are expanded first so
        # the contents list sees every heading on the final page
        body = re.sub(r"\{\{(page_list|gallery_body|benchmarks_body)\}\}", lambda m: subs[m.group(1)], body)
        body, has_toc = add_toc(body)
        after = "" if slug == "404" else source_html(meta) + next_html(slug, set(pages))
        fill = {**subs, "title": escape(meta["title"]), "description": escape(meta["description"], quote=True),
                "nav": nav_html(slug, set(pages)), "body": body, "after": after,
                "mainclass": ' class="has-toc"' if has_toc else ""}
        # body first so its own {{repo}} etc. are expanded by the second pass
        for _ in range(2):
            out = re.sub(r"\{\{(\w+)\}\}", lambda m: fill[m.group(1)], out)
        # pages/404.html becomes /404.html, which Cloudflare Pages serves with
        # status 404 for unknown paths. Without it Pages answers every unknown
        # path with the landing page and status 200.
        if slug == "404":
            target = DIST / "404.html"
        else:
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
