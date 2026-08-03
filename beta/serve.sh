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
echo "  WASM channel  http://127.0.0.1:$port/wasm/"
echo

exec python3 "$(dirname "${BASH_SOURCE[0]}")/serve.py" "$root" "$port"
