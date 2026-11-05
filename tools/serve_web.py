"""Serves the browser build (web/) on 127.0.0.1 for testing: `python -I tools/serve_web.py [--port 8047] [--install DIR]`.

With `--install`, the player's own install is also served, read-only and on this machine only, under `/game/` (its
file list at `/game/index.json`, as the web image serves it: ADR 0081); the page then offers to download it, and
`http://127.0.0.1:8047/?dev-install` copies it into the page's storage without asking (ADR 0047 part 3). Never bind
this to another address.
"""
import argparse
import functools
import http.server
import json
import os
import pathlib
import urllib.parse

ROOT = pathlib.Path(__file__).resolve().parent.parent
PREFIX = "/game/"


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm", ".js": "text/javascript"}

    def __init__(self, *args, install=None, **kwargs):
        self.install = install
        super().__init__(*args, **kwargs)

    def do_GET(self):
        path = urllib.parse.unquote(urllib.parse.urlsplit(self.path).path)
        if self.install and path == PREFIX + "index.json":
            files = [[p.relative_to(self.install).as_posix(), p.stat().st_size] for p in sorted(self.install.rglob("*")) if p.is_file()]
            body = json.dumps(files).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        super().do_GET()

    def end_headers(self):
        # Every build is fresh: a cached module from an older build would run instead.
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def translate_path(self, path):
        rel = urllib.parse.unquote(urllib.parse.urlsplit(path).path)
        if self.install and rel.startswith(PREFIX):
            target = (self.install / rel[len(PREFIX):]).resolve()
            # Nothing outside the install.
            return str(target) if target.is_relative_to(self.install) else str(self.install / "\0")
        return super().translate_path(path)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=8047)
    ap.add_argument("--install", type=pathlib.Path, default=os.environ.get("IG2_INSTALL_DIR"))
    a = ap.parse_args()
    install = a.install.resolve() if a.install else None
    handler = functools.partial(Handler, directory=str(ROOT / "web"), install=install)
    with http.server.ThreadingHTTPServer(("127.0.0.1", a.port), handler) as srv:
        print(f"http://127.0.0.1:{a.port}/" + ("?dev-install" if install else ""), flush=True)
        srv.serve_forever()


if __name__ == "__main__":
    main()
