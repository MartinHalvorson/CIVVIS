#!/usr/bin/env bash
#
# Assemble a published build of CIVVIS for civvis.ai.
#
# The output is a plain directory of static files — no server, no runtime, no
# host-specific anything beyond the two Cloudflare Pages control files, which
# any other host simply ignores. Everything under `beta/` is the same viewer
# the desktop build serves, with the engine compiled to WebAssembly beside it.
#
#   ./beta/publish.sh                  build from the working tree
#   ./beta/publish.sh --commit <ref>   build from a pinned revision
#
# Nothing in `web/` or `src/` is edited. The viewer is *copied* and the copy is
# adjusted for living in a subdirectory; if any of those adjustments stops
# matching, this script fails rather than publishing a page with dead links.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="$repo_root/beta/dist"
commit_ref=""

while [ $# -gt 0 ]; do
  case "$1" in
    --commit) commit_ref="${2:?--commit needs a revision}"; shift 2 ;;
    --out) out="${2:?--out needs a directory}"; shift 2 ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
done

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"

command -v cargo >/dev/null || { echo "cargo is not on PATH" >&2; exit 1; }
rustup target list --installed | grep -qx wasm32-unknown-unknown \
  || rustup target add wasm32-unknown-unknown

source_tree="$repo_root"
scratch=""
if [ -n "$commit_ref" ]; then
  # A published build names one revision and is built from exactly it, not
  # from whatever happened to be in the tree at the time.
  scratch="$(mktemp -d)"
  trap 'rm -rf "$scratch"' EXIT
  git -C "$repo_root" worktree add --detach "$scratch/src" "$commit_ref" >/dev/null
  source_tree="$scratch/src"
  trap 'git -C "$repo_root" worktree remove --force "$scratch/src" >/dev/null 2>&1 || true; rm -rf "$scratch"' EXIT
fi

commit="$(git -C "$source_tree" rev-parse HEAD)"
short="$(git -C "$source_tree" rev-parse --short HEAD)"
built_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

echo "==> building the engine for wasm32 from $short"
(
  cd "$source_tree"
  CIVVIS_COMMIT="$short" cargo rustc --lib \
    --target wasm32-unknown-unknown --release --crate-type cdylib
)

raw_wasm="$source_tree/target/wasm32-unknown-unknown/release/civvis.wasm"
[ -f "$raw_wasm" ] || raw_wasm="${CARGO_TARGET_DIR:-}/wasm32-unknown-unknown/release/civvis.wasm"
[ -f "$raw_wasm" ] || { echo "the wasm build produced nothing at $raw_wasm" >&2; exit 1; }

rm -rf "$out"
mkdir -p "$out/beta"

if command -v wasm-opt >/dev/null; then
  echo "==> shrinking the module"
  # -O3 keeps the optimiser aimed at speed: this module simulates whole games,
  # and -Oz costs more turn time than it saves bytes over the wire.
  wasm-opt -O3 "$raw_wasm" -o "$out/beta/civvis.wasm"
else
  echo "==> wasm-opt is not installed; publishing the unshrunk module" >&2
  cp "$raw_wasm" "$out/beta/civvis.wasm"
fi

echo "==> assembling the page"
cp "$repo_root/beta/shim.js" "$repo_root/beta/worker.js" "$out/beta/"
cp "$source_tree/web/cinematic3d.js" "$out/beta/"
cp -R "$source_tree/web/assets" "$out/beta/assets"
cp "$repo_root/beta/landing.html" "$out/index.html"
# The gate travels *inside* the deployed directory. See beta/_worker.js for
# why this is not a `functions/` directory.
cp "$repo_root/beta/_worker.js" "$out/_worker.js"

# The viewer, copied and then made to work one directory down. Each
# substitution is checked, because a silently unmatched one publishes a page
# whose sprites and 3D view are simply missing.
python3 - "$source_tree/web/index.html" "$out/beta/index.html" <<'PY'
import sys, pathlib

source, target = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
page = source.read_text(encoding="utf-8")

# The page asks for its assets from the site root, which is where a desktop
# build serves them. Published under /beta/ they sit beside the page instead.
edits = [
    ('src="/cinematic3d.js"', 'src="cinematic3d.js"', 1),
    ('"/assets/', '"assets/', 7),
]
for needle, replacement, expected in edits:
    found = page.count(needle)
    if found != expected:
        raise SystemExit(
            f"expected {expected} occurrence(s) of {needle!r} in the viewer, found {found}. "
            "web/index.html has changed shape; beta/publish.sh needs updating before "
            "this build can be published."
        )
    page = page.replace(needle, replacement)

# The interception has to be installed before the page's own first script runs.
head = "<head>"
if page.count(head) != 1:
    raise SystemExit("the viewer does not have exactly one <head>")
page = page.replace(head, head + '\n<script src="shim.js"></script>', 1)

target.write_text(page, encoding="utf-8")
print(f"   viewer written to {target} ({len(page):,} bytes)")
PY

wasm_bytes=$(wc -c < "$out/beta/civvis.wasm" | tr -d ' ')
cat > "$out/beta/build.json" <<JSON
{
  "commit": "$commit",
  "short": "$short",
  "built_at": "$built_at",
  "wasm_bytes": $wasm_bytes
}
JSON

echo
echo "published build $short -> $out"
echo "  engine      $(printf "%'d" "$wasm_bytes") bytes"
if command -v brotli >/dev/null; then
  echo "  over a wire $(printf "%'d" "$(brotli -q 11 -c "$out/beta/civvis.wasm" | wc -c | tr -d ' ')") bytes brotli"
fi
echo
echo "check it locally with:  ./beta/serve.sh"
echo "deploy it with:         npx wrangler pages deploy $out --project-name civvis"
