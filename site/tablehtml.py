"""One table emitter for the whole site (stdlib only).

build.py (markdown sources) and render.py (benchmarks page) both call
render_table(), so every table has the same wrapper and the same classes.

The cells are classified from their text, not hand-marked:

  .num    numeric column (>= 70% of body cells are a number with an optional
          sign, comparator and unit, or an empty value): right-aligned,
          tabular figures, never wrapped
  .prose  a cell in a column holding text over PROSE_CHARS characters: wraps
          at spaces and gets a readable minimum width
  .na     an empty-value cell ("—", "not attempted"): muted

A table with WIDE_COLUMNS or more columns and no prose column also gets
`.wide` on its wrapper and may use the wide container (site.css).
"""
import re
from html import escape

PROSE_CHARS = 80
WIDE_COLUMNS = 6
NUMERIC_SHARE = 0.7

_GROUP = "[    ]"  # thin / no-break / plain space between digit groups
_NUM = rf"[<>≤≥~≈±+\-−]?\d+(?:{_GROUP}\d{{3}})*(?:[.,]\d+)?(?:e[+\-−]?\d+)?"
_UNIT = r"(?:\s?(?:[%×]|[A-Za-zµμΩ]{1,5}(?:/[A-Za-zµμΩ]{1,5})?))?"
_NUMERIC = re.compile(rf"{_NUM}{_UNIT}(?:\s*/\s*{_NUM}{_UNIT})*(?:\s*\([^)]*\))?")
_EMPTY = re.compile(r"(?:—|–|-|n/a|not attempted)?", re.I)


_CODE = re.compile(r"<code>([^<]*)</code>")


def _nobreak(m):
    """Keep each space-free token of a code span on one line (no break at a
    hyphen in `--fasthenry-compat`); the span itself still wraps at spaces."""
    return "<code>" + re.sub(r"\S+", lambda w: f'<span class="tok">{w.group(0)}</span>', m.group(1)) + "</code>"


def plain(cell):
    """Cell text with the markdown emphasis and code marks removed."""
    return re.sub(r"\\(.)", r"\1", cell.replace("`", "").replace("*", "")).strip()


def is_empty(cell):
    return _EMPTY.fullmatch(plain(cell)) is not None


def is_numeric(cell):
    return _NUMERIC.fullmatch(plain(cell)) is not None


def classify(head, rows):
    """Per column: 'num', 'prose' or '' (plain text)."""
    kinds = []
    for k in range(len(head)):
        body = [r[k] for r in rows if k < len(r)]
        nums = sum(1 for c in body if not is_empty(c) and is_numeric(c))
        empties = sum(1 for c in body if is_empty(c))
        if nums and (nums + empties) >= NUMERIC_SHARE * len(body):
            kinds.append("num")
        elif any(len(plain(c)) > PROSE_CHARS for c in body):
            kinds.append("prose")
        else:
            kinds.append("")
    return kinds


def render_table(head, rows, inline, aligns=()):
    """HTML for a table. `head` and `rows` hold raw cell text, `inline` turns
    one cell into HTML, `aligns[k]` is an explicit markdown alignment."""
    kinds = classify(head, rows)

    def row(cells, tag):
        out = []
        for k, c in enumerate(cells):
            cls = [kinds[k]] if k < len(kinds) and kinds[k] else []
            if tag == "td" and is_empty(c):
                cls.append("na")
            a = aligns[k] if k < len(aligns) else ""
            attrs = (f' class="{" ".join(cls)}"' if cls else "") + (f' style="text-align:{a}"' if a else "")
            html = _CODE.sub(_nobreak, inline(c))
            out.append(f"<{tag}{attrs}>{html}</{tag}>")
        return "<tr>" + "".join(out) + "</tr>"

    wide = len(head) >= WIDE_COLUMNS and "prose" not in kinds
    names = [plain(h) for h in head if plain(h)]
    label = "Table: " + ", ".join(names[:3]) + (", …" if len(names) > 3 else "")
    return (f'<div class="tablewrap{" wide" if wide else ""}" role="region" tabindex="0" '
            f'aria-label="{escape(label, quote=True)}"><table>\n<thead>{row(head, "th")}</thead>\n<tbody>\n'
            + "\n".join(row(r, "td") for r in rows) + "\n</tbody></table></div>")
