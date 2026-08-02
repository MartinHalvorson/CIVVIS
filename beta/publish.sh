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
cp -R "$source_tree/web/assets" "$out/beta/assets"
cp "$repo_root/beta/landing.html" "$out/index.html"
# `/download/` rather than `/download.html`: the page outlives any one release
# and gets linked around, so it should have the tidier address.
mkdir -p "$out/download"
cp "$repo_root/beta/download.html" "$out/download/index.html"
# The gate travels *inside* the deployed directory. See beta/_worker.js for
# why this is not a `functions/` directory.
cp "$repo_root/beta/_worker.js" "$out/_worker.js"

# The viewer, copied and then made to work one directory down. Each
# substitution is checked, because a silently unmatched one publishes a page
# whose strategic map sprites are simply missing.
python3 - "$source_tree/web/index.html" "$out/beta/index.html" "$out/beta/assets" <<'PY'
import re, sys, pathlib

source, target = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
assets = pathlib.Path(sys.argv[3])
page = source.read_text(encoding="utf-8")

# The page asks for its assets from the site root, which is where a desktop
# build serves them. Published under /beta/ they sit beside the page instead.
#
# The check is "none left afterwards", not "exactly N rewritten". An exact
# count fails the build every time somebody adds a sprite atlas — which is
# normal work, happens often, and has nothing to do with whether the rewrite
# is still correct. What actually matters is that no root-absolute reference
# survives into the published page, because each one is a sprite that silently
# does not load.
edits = [
    ('"/assets/', '"assets/'),
]
for needle, replacement in edits:
    if needle not in page:
        raise SystemExit(
            f"the viewer no longer contains {needle!r}. web/index.html has changed "
            "shape; beta/publish.sh needs updating before this build can be published."
        )
    page = page.replace(needle, replacement)

for stranded in ('"/assets/',):
    if stranded in page:
        raise SystemExit(f"{stranded!r} survived the rewrite; the published page would 404 on it")

# The sprite atlases, republished as lossless WebP.
#
# They are the heaviest thing here — heavier than the engine — and they grow
# every time somebody draws something. Lossless WebP is *pixel-identical* to
# the PNG it replaces, so this is purely a better container: measured at
# publication, 12.46 MB of atlas became 8.87 MB, and nothing on screen
# changed. It happens to the *copy*: `web/assets` stays PNG, which is what the
# desktop build serves and what anyone editing the art works on.
#
# Optional, because a machine cutting a build should not need a wheel
# installed to do it. Without Pillow the atlases publish as they are and the
# only cost is the budget at the end of this script.
try:
    from PIL import Image
except ImportError:
    print("   Pillow is not installed; publishing the atlases as PNG")
else:
    before = after = 0
    for png in sorted(assets.glob("*.png")):
        webp = png.with_suffix(".webp")
        image = Image.open(png)
        image.load()
        image.save(webp, "WEBP", lossless=True, quality=100, method=5)
        before += png.stat().st_size
        after += webp.stat().st_size
        png.unlink()
        page = page.replace(f'"assets/{png.name}"', f'"assets/{webp.name}"')
    if before:
        print(f"   atlases {before:,} -> {after:,} bytes as lossless WebP")

# Every atlas the page asks for has to be one this build actually ships. This
# is what catches a rewrite that half-happened: a reference the loop above
# missed still names a `.png` that is no longer on disk.
wanted = set(re.findall(r'"assets/([^"]+)"', page))
have = {p.name for p in assets.iterdir()} if assets.is_dir() else set()
missing = sorted(wanted - have)
if missing:
    raise SystemExit(f"the viewer asks for assets this build does not ship: {', '.join(missing)}")

# The interception has to be installed before the page's own first script runs.
head = "<head>"
if page.count(head) != 1:
    raise SystemExit("the viewer does not have exactly one <head>")
page = page.replace(head, head + '\n<script src="shim.js"></script>', 1)

target.write_text(page, encoding="utf-8")
print(f"   viewer written to {target} ({len(page):,} bytes)")
PY

wasm_bytes=$(wc -c < "$out/beta/civvis.wasm" | tr -d ' ')
bundle_bytes=$(find "$out" -type f -exec wc -c {} + | tail -1 | awk '{print $1}')
cat > "$out/beta/build.json" <<JSON
{
  "commit": "$commit",
  "short": "$short",
  "built_at": "$built_at",
  "wasm_bytes": $wasm_bytes,
  "bundle_bytes": $bundle_bytes
}
JSON

echo
echo "published build $short -> $out"
echo "  engine      $(printf "%'d" "$wasm_bytes") bytes"
if command -v brotli >/dev/null; then
  echo "  over a wire $(printf "%'d" "$(brotli -q 11 -c "$out/beta/civvis.wasm" | wc -c | tr -d ' ')") bytes brotli"
fi
echo "  bundle      $(printf "%'d" "$bundle_bytes") bytes on disk"

# The whole point of publishing this way is that it stays something a person
# can be handed over an ordinary connection, and the engine grows every week.
# A budget nobody measures is a wish, so this is a build failure rather than a
# line in a document: raise BUNDLE_BUDGET_BYTES deliberately, or find the
# weight. (The assets are already compressed images; the module is not, and
# `wasm-opt` plus brotli is where the room is.)
budget=${BUNDLE_BUDGET_BYTES:-26214400}
if [ "$bundle_bytes" -gt "$budget" ]; then
  echo
  echo "the bundle is $(printf "%'d" "$bundle_bytes") bytes, over the $(printf "%'d" "$budget") byte budget" >&2
  exit 1
fi
echo "              $(( bundle_bytes * 100 / budget ))% of the $(( budget / 1048576 )) MiB budget"

echo
echo "check it locally with:  ./beta/serve.sh"
echo "deploy it with:         npx wrangler pages deploy $out --project-name civvis"
