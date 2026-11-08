"""Tests for the markdown renderer in build.py. Run: python3 -m unittest discover -s site"""
import re, subprocess, sys, unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import build

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
        self.assertIn("<td><code>x|y</code></td><td style=\"text-align:right\"><code>|t|</code></td>", h)

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


class Build(unittest.TestCase):
    def test_sources_render_into_pages_without_md_hrefs(self):
        subprocess.run([sys.executable, str(build.SITE / "build.py")], check=True, capture_output=True)
        for slug in ("validation", "fasthenry-users"):
            html = (build.DIST / slug / "index.html").read_text()
            self.assertIn("<table>", html)
            self.assertNotRegex(html, r'href="(?!https?://)[^"]*\.md')
            self.assertNotIn("{{", html)
        home = (build.DIST / "index.html").read_text()
        self.assertNotRegex(home, r"(Validation|FastHenry users) \(soon\)")


if __name__ == "__main__":
    unittest.main()
