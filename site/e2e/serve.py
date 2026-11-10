#!/usr/bin/env python3
"""Serve site/dist for the browser tests, the way Cloudflare Pages does.

Like `python3 -m http.server`, with one difference that matters to the tests:
an unknown path answers with /404.html and status 404.

Usage: serve.py [port]   (default 4173; binds 127.0.0.1 only)
"""
import sys
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

DIST = Path(__file__).resolve().parent.parent / "dist"


class Handler(SimpleHTTPRequestHandler):
    def send_error(self, code, message=None, explain=None):
        page = DIST / "404.html"
        if code != 404 or not page.is_file():
            return super().send_error(code, message, explain)
        body = page.read_bytes()
        self.send_response(404)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(body)

    def log_message(self, *args):
        pass


if __name__ == "__main__":
    if not (DIST / "index.html").is_file():
        sys.exit("site/dist is not built; run scripts/deploy-site.sh --no-deploy first")
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 4173
    ThreadingHTTPServer(("127.0.0.1", port), partial(Handler, directory=str(DIST))).serve_forever()
