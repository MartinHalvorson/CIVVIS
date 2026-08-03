#!/usr/bin/env python3
"""Serve a published CIVVIS bundle with its stable local WASM channel."""

import functools
import http.server
import socketserver
import sys
import urllib.parse


def channel_path(path):
    """Map the local /wasm channel onto the published /beta bundle."""
    parsed = urllib.parse.urlsplit(path)
    if parsed.path == "/wasm":
        mapped = "/beta/"
    elif parsed.path.startswith("/wasm/"):
        mapped = "/beta/" + parsed.path.removeprefix("/wasm/")
    else:
        return path
    return urllib.parse.urlunsplit(("", "", mapped, parsed.query, parsed.fragment))


class Handler(http.server.SimpleHTTPRequestHandler):
    # Python's table has no entry for wasm, and `instantiateStreaming` refuses
    # anything that is not application/wasm.
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
        ".json": "application/json",
        ".webp": "image/webp",
    }

    def translate_path(self, path):
        return super().translate_path(channel_path(path))

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def log_message(self, fmt, *args):
        sys.stderr.write("  %s\n" % (fmt % args))


class Server(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


def main():
    if len(sys.argv) != 3:
        raise SystemExit("usage: serve.py SITE_ROOT PORT")
    root, port = sys.argv[1], int(sys.argv[2])
    handler = functools.partial(Handler, directory=root)
    with Server(("127.0.0.1", port), handler) as server:
        server.serve_forever()


if __name__ == "__main__":
    main()
