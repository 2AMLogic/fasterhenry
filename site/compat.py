"""The FastHenry-users page, built from docs/fasthenry-compat.md (stdlib only).

Nothing here is hand-typed content. The audit's Field / Status / Notes tables
are turned into entry lists, each status is classified from the Status column
by its leading words, and the "at a glance" tally is counted from those same
rows. A status this module does not know fails the build, so a row can never
silently drop out of the count.

Entry ids and the bare-`#NN` issue links come from site/mdinline.py, shared
with build.py and render.py.
"""
import re
from html import escape

import mdinline

# kind -> label used in the tally ("12 supported · 3 differ · 1 not supported")
KINDS = [
    ("supported", "supported"), ("extended", "extended"), ("differs", "differ"),
    ("compat", "compat only"), ("error", "error"), ("rejected", "rejected"),
    ("not-supported", "not supported"), ("deferred", "deferred"),
]
LABEL = dict(KINDS)


def classify_status(status):
    """Status cell (markdown) -> (badge word, kind, qualifier markdown).

    The badge keeps the status word; whatever follows it in the cell
    ("differs (opt-in)", "extended", "compat only") is returned as the
    qualifier, verbatim, to be rendered as plain text beside the badge."""
    s = " ".join(status.split())
    low = s.lower()
    if low.startswith("native: not supported"):
        return "Not supported", "not-supported", s.split(",", 1)[1].strip() if "," in s else ""
    if low == "supported":
        return "Supported", "supported", ""
    if low.startswith("supported, extended"):
        return "Supported", "extended", s[len("Supported, "):]
    if low.startswith("supported, differs"):
        return "Supported", "differs", s[len("Supported, "):]
    if low.startswith("supported (compat only)"):
        return "Supported", "compat", s[len("Supported "):].strip("()")
    if low.startswith("not supported"):
        return "Not supported", "not-supported", s[len("Not supported"):].lstrip(", ")
    if low.startswith("error"):
        return "Error", "error", s[len("Error"):].lstrip(", ")
    if low.startswith("rejected"):
        return "Rejected", "rejected", s[len("Rejected"):].lstrip(", ")
    if low.startswith("deferred"):
        return "Deferred", "deferred", s[len("Deferred"):].lstrip(", ")
    raise SystemExit(f"docs/fasthenry-compat.md: unrecognised status {status!r}; teach site/compat.py classify_status")


def is_audit_table(head):
    h = [c.strip().lower() for c in head]
    return (len(h) == 3 and h[1:] == ["status", "notes"]) or h == ["status", "notes"]


_ABBREV = re.compile(r"(?:e\.g|i\.e|etc|vs|cf|approx|no)\.$", re.I)


def split_note(md, lead=60, min_total=200, min_rest=60):
    """(lead, rest) at a sentence end outside code spans, links and bold; rest is '' for a short note."""
    if len(md) < min_total:
        return md, ""
    spans = [(m.start(), m.end()) for m in re.finditer(r"(`+).+?\1", md)]
    for m in re.finditer(r"(?<=[.!?;:])\s+(?=\S)|\s+(?=—\s)", md):
        i = m.start()
        if i < lead or any(a <= i < b for a, b in spans) or _ABBREV.search(md[:i]):
            continue
        head = md[:i]
        if head.count("[") != head.count("]") or head.count("**") % 2 or head.count("(") != head.count(")"):
            continue
        if len(md) - i < min_rest:
            break
        return head, md[m.end():]
    return md, ""


def entries_html(head, rows, inline, family, seen):
    """Entry list (<dl>) for one audit table; returns (html, [kind per row])."""
    two = len(head) == 2
    kinds, out = [], ['<dl class="entries">']
    for r in rows:
        field, status, note = (family, r[0], r[1]) if two else (r[0], r[1], r[2] if len(r) > 2 else "")
        word, kind, qual = classify_status(status)
        kinds.append(kind)
        eid = mdinline.slug(f"{family} {field}", seen)
        plain = re.sub(r"[`*\\]", "", field)
        fhtml = inline(field)
        if "<a " not in fhtml:
            fhtml = f'<a class="field" href="#{eid}">{fhtml}</a>'
        lead, rest = split_note(note)
        body = f"<p>{inline(lead)}</p>" if lead else ""
        if rest:
            body += (f'<details class="more"><summary aria-label="Read the rest of the note on {escape(plain, quote=True)}">Read the rest</summary>'
                     f"<p>{inline(rest)}</p></details>")
        q = f' <span class="qual">{inline(qual)}</span>' if qual else ""
        out.append(f'<div class="entry" id="{eid}"><dt><span class="field">{fhtml}</span> '
                   f'<span class="badge s-{kind}">{word}</span>{q}</dt><dd>{body}</dd></div>')
    out.append("</dl>")
    return "\n".join(out), kinds


def tally(kinds):
    """{kind: count} in KINDS order, zero counts omitted."""
    return {k: kinds.count(k) for k, _ in KINDS if kinds.count(k)}


def tally_html(counts):
    return " ".join(f'<span class="badge s-{k}"><b>{n}</b> {LABEL[k]}</span>' for k, n in counts.items())


def glance_html(families):
    """families: [(title_html, id, [kinds])] -> one row per directive family, plus the total."""
    total = [k for _, _, ks in families for k in ks]
    rows = [f'<li><a href="#{fid}">{title}</a><span class="tally">{tally_html(tally(ks))}</span></li>'
            for title, fid, ks in families]
    return (f'<p class="glance-total">{len(total)} audited entries: {tally_html(tally(total))}</p>\n'
            '<ul class="glance">\n' + "\n".join(rows) + "\n</ul>")
