#!/usr/bin/env python3
"""Serve the local WASM channel."""

import functools
import http.server
import socketserver
import sys
import urllib.parse



def channel_path(path):
    """Map the local /wasm channel onto the published viewer.

    The viewer sits at the lane root now — publish.sh emits one complete
    lane per revision, viewer at its root with /home and /download beside it
    — so the channel maps straight onto the root rather than a test/
    subdirectory.
    """
    parsed = urllib.parse.urlsplit(path)
    if parsed.path == "/wasm":
        mapped = "/"
    elif parsed.path.startswith("/wasm/"):
        mapped = "/" + parsed.path.removeprefix("/wasm/")
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
    # The rating-host arguments are retired with the league (#2357); a
    # launcher that still passes them is served rather than refused, so an
    # older installed bundle keeps working across the transition.
    if len(sys.argv) not in (3, 5):
        raise SystemExit("usage: serve.py SITE_ROOT PORT")
    root, port = sys.argv[1], int(sys.argv[2])
    handler = functools.partial(Handler, directory=root)
    with Server(("127.0.0.1", port), handler) as server:
        server.serve_forever()


if __name__ == "__main__":
    main()
