#!/usr/bin/env python3
"""Inline markdown helpers for the whole site build (stdlib only).

One copy of each helper the page builders need below the block level:

  * inline()      markdown spans -> HTML (code, escapes, links, emphasis)
  * rewrite_link() a relative link in a doc -> its {{repo}} URL
  * cells()       split a pipe-table row on the bars that separate cells
  * slug()        heading or entry text -> a unique id
  * link_issues() bare `#NN` in rendered HTML -> a link to that issue

build.py (the markdown sources), render.py (the benchmarks and validation
pages) and compat.py (the FastHenry-users page) all import this module, so the
same markdown cannot render one way on one page and another way on another --
the drift that issue #347 removed, where each of these existed twice with two
behaviours.

A leaf module by design: like tablehtml.py and charts.py it imports nothing
else from the site build, so build.py can keep importing render.py.
"""
import posixpath, re, sys
from html import escape
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

_SCHEME = re.compile(r"^[A-Za-z][A-Za-z0-9+.-]*:|^//")
# A bare issue reference: not an HTML entity (&#39;), not a URL path (/issues/5),
# not a `##` heading marker. `post-#18` is a reference, so `-` does not disqualify.
_ISSUE_REF = re.compile(r"(?<![\w&/#])#(\d+)\b")
# What link_issues() must not rewrite inside: a code block, an existing link, a
# code span, or any tag's own attributes.
_MARKUP = re.compile(r"(<pre\b.*?</pre>|<a\b.*?</a>|<code\b[^>]*>.*?</code>|<[^>]*>)", re.S)


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
    """Inline markdown in `text` -> HTML. `src` is the repo-relative path of the
    document it came from, used to resolve relative links (see rewrite_link)."""
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


def cells(row):
    """Split a table row on `|` outside code spans (the docs write `|tan θ|` unescaped)."""
    row = row.strip()
    out, cur = [], []
    for m in re.finditer(r"(`+).+?\1|\\.|\||[^`\\|]+|.", row):
        tok = m.group(0)
        if tok == "|":
            out.append("".join(cur))
            cur = []
        else:
            cur.append(tok)
    out.append("".join(cur))
    if out and out[0].strip() == "":
        out = out[1:]
    if out and out[-1].strip() == "":
        out = out[:-1]
    return [c.strip().replace("\\|", "|") for c in out]


def slug(text, seen):
    """`text` -> a lowercase dashed id not already in `seen` (which it adds to)."""
    base = re.sub(r"[^a-z0-9 _-]", "", re.sub(r"[`*]", "", text).lower()).strip().replace(" ", "-") or "section"
    out, n = base, 1
    while out in seen:
        n += 1
        out = f"{base}-{n}"
    seen.add(out)
    return out


def link_issues(html, repo="{{repo}}"):
    """Bare #NN in text becomes a link to that issue; text already in a link, a
    tag, a code span or a <pre> is left alone."""
    parts = _MARKUP.split(html)
    for i in range(0, len(parts), 2):
        parts[i] = _ISSUE_REF.sub(rf'<a href="{repo}/issues/\1">#\1</a>', parts[i])
    return "".join(parts)
