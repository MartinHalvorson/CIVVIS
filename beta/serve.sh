#!/usr/bin/env bash
#
# Serve a published build locally, the way a static host would.
#
# This is the check that a build is actually publishable: it is the same
# directory that gets deployed, served over HTTP with the same MIME types, so
# the page boots the same way it will on civvis.ai. The password gate is a
# Cloudflare Function and is not part of this — `npx wrangler pages dev
# beta/dist` runs that too, if it is the gate you want to look at.
#
#   ./beta/serve.sh [port]

set -euo pipefail

port="${1:-8790}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/dist"

[ -d "$root" ] || { echo "no build at $root — run ./beta/publish.sh first" >&2; exit 1; }

echo "serving $root"
echo "  landing page  http://127.0.0.1:$port/"
echo "  beta build    http://127.0.0.1:$port/beta/"
echo

exec python3 - "$root" "$port" <<'PY'
import functools, http.server, socketserver, sys

root, port = sys.argv[1], int(sys.argv[2])

class Handler(http.server.SimpleHTTPRequestHandler):
    # Python's table has no entry for wasm, and `instantiateStreaming` refuses
    # anything that is not application/wasm — which is exactly the failure a
    # local check exists to catch before a deploy does.
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
        ".json": "application/json",
    }

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def log_message(self, fmt, *args):
        sys.stderr.write("  %s\n" % (fmt % args))

socketserver.TCPServer.allow_reuse_address = True
with socketserver.TCPServer(("127.0.0.1", port),
                            functools.partial(Handler, directory=root)) as httpd:
    httpd.serve_forever()
PY
