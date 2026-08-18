#!/usr/bin/env bash
#
# Assemble a published build of CIVVIS for civvis.ai.
#
# The output is one complete LANE of the site — the viewer at its root with
# the engine compiled to WebAssembly beside it, the landing page at /home and
# the downloads page at /download — a plain directory of static files with no
# host-specific anything beyond the two Cloudflare Pages control files, which
# any other host simply ignores. Every link between the pages is
# lane-relative, so the same tree serves unchanged at the site root (the
# stable lane) and under /test (the head lane); the deploy workflow builds one
# lane per revision and stacks them.
#
#   ./beta/publish.sh                  build from the working tree
#   ./beta/publish.sh --commit <ref>   build from a pinned revision
#
# Nothing in `web/` or `src/` is edited. The viewer is *copied* and the copy is
# adjusted for living in a lane; if any of those adjustments stops matching,
# this script fails rather than publishing a page with dead links.

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
commit_time="$(git -C "$source_tree" show -s --format=%cI "$commit")"
built_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

echo "==> building the engine for wasm32 from $short"
(
  cd "$source_tree"
  CIVVIS_COMMIT="$commit" CIVVIS_COMMIT_TIME="$commit_time" CIVVIS_BUILT_AT="$built_at" \
    cargo rustc --lib \
    --target wasm32-unknown-unknown --release --crate-type cdylib
)

raw_wasm="$source_tree/target/wasm32-unknown-unknown/release/civvis.wasm"
[ -f "$raw_wasm" ] || raw_wasm="${CARGO_TARGET_DIR:-}/wasm32-unknown-unknown/release/civvis.wasm"
[ -f "$raw_wasm" ] || { echo "the wasm build produced nothing at $raw_wasm" >&2; exit 1; }

rm -rf "$out"
mkdir -p "$out"

if command -v wasm-opt >/dev/null; then
  echo "==> shrinking the module"
  # -O3 keeps the optimiser aimed at speed: this module simulates whole games,
  # and -Oz costs more turn time than it saves bytes over the wire.
  wasm-opt -O3 "$raw_wasm" -o "$out/civvis.wasm"
else
  echo "==> wasm-opt is not installed; publishing the unshrunk module" >&2
  cp "$raw_wasm" "$out/civvis.wasm"
fi

echo "==> assembling the pages"
cp "$repo_root/beta/shim.js" "$repo_root/beta/worker.js" "$out/"
cp -R "$source_tree/web/assets" "$out/assets"
# Every page of the site exists once per lane, built from the lane's own
# revision: the promoted stable lane shows the promoted landing and download
# pages, and /test shows head's. A pinned revision that predates one of the
# pages falls back to head's copy, so any commit stays publishable.
landing_page="$source_tree/beta/landing.html"
[ -f "$landing_page" ] || landing_page="$repo_root/beta/landing.html"
landing_assets="$source_tree/beta/assets"
[ -d "$landing_assets" ] || landing_assets="$repo_root/beta/assets"
download_page="$source_tree/beta/download.html"
[ -f "$download_page" ] || download_page="$repo_root/beta/download.html"
mkdir -p "$out/home"
cp "$landing_page" "$out/home/index.html"
cp -R "$landing_assets" "$out/home/assets"
# `/download/` rather than `/download.html`: the page outlives any one release
# and gets linked around, so it should have the tidier address.
mkdir -p "$out/download"
cp "$download_page" "$out/download/index.html"
# The gate travels *inside* the deployed directory. See beta/_worker.js for
# why this is not a `functions/` directory. Unlike the pages, these two are
# deployment-global rather than lane content — always head's copy, and the
# deploy workflow keeps only the root copy when it stacks two lanes.
cp "$repo_root/beta/_worker.js" "$out/_worker.js"
# Its very existence switches Pages from single-page-app fallback (unknown
# path -> the front page, 200) to honest 404s. See the comment inside it.
cp "$repo_root/beta/404.html" "$out/404.html"

# The viewer, copied and then made lane-relative. Each substitution is
# checked, because a silently unmatched one publishes a page whose strategic
# map sprites are simply missing.
python3 - "$source_tree/web/index.html" "$out/index.html" "$out/assets" <<'PY'
import re, sys, pathlib

source, target = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
assets = pathlib.Path(sys.argv[3])
page = source.read_text(encoding="utf-8")

# The viewer's scripts live beside the atlases in `assets/` (the renderer was a
# 1.35 MB inline block; splitting it out is what stopped `web/index.html`
# growing the pack by a megabyte per edit). Their copies arrived with the
# `cp -R web/assets` above, and they hold most of the root-absolute asset
# references now, so every rewrite below runs over the page and all of them.
#
# ⚠⚠ EVERY SCRIPT, NOT `app.js`. This named one file, and the day the renderer
# was carved further that became a silent deployment break: `app_palette.js`
# carries `"/assets/feature-atlas.png"` and two more like it, and a script this
# loop does not visit keeps them ROOT-absolute. Published in a lane served at
# /test they resolve to /assets/... on the site root, 404, and the terrain
# atlases simply do not load — "a sprite that silently does not load", exactly
# as the note below this one warns. `verify.py` would not have caught it
# either: it checks index.html for surviving `"/assets/` and nothing else.
#
# A revision from before the script moved out of the page (#1289) has no
# scripts here at all — its viewer is still inline in index.html — so the map
# is simply empty and every rewrite runs over the page alone. The stable lane
# is routinely pinned to such a revision.
scripts = {path: path.read_text(encoding="utf-8")
           for path in sorted(assets.glob("*.js"))}

# The page asks for its assets from the site root, which is where a desktop
# build serves them. Published in a lane they sit beside the page instead —
# and lane-relative is what lets the identical tree serve at / and at /test.
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
    if needle not in page and not any(needle in t for t in scripts.values()):
        raise SystemExit(
            f"the viewer no longer contains {needle!r}. web/index.html has changed "
            "shape; beta/publish.sh needs updating before this build can be published."
        )
    page = page.replace(needle, replacement)
    for path in scripts:
        scripts[path] = scripts[path].replace(needle, replacement)

# Links between the site's pages must stay inside the lane too: a test-lane
# viewer linking `/home` would silently drop the visitor back into prod.
# Tolerated as already absent (a revision may predate the link); the guard
# below is what is enforced.
for needle, replacement in [('href="/home"', 'href="home/"')]:
    page = page.replace(needle, replacement)
    for path in scripts:
        scripts[path] = scripts[path].replace(needle, replacement)

all_script_text = "".join(scripts.values())
for stranded in ('"/assets/',):
    if stranded in page or stranded in all_script_text:
        raise SystemExit(f"{stranded!r} survived the rewrite; the published page would 404 on it")
leaked = sorted(set(re.findall(r'(?:href|src)="(/[^"]*)"', page + all_script_text)))
if leaked:
    raise SystemExit(
        f"the viewer still links the site root ({', '.join(leaked)}); "
        "a lane page must link relatively so /test stays /test"
    )

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
        for path in scripts:
            scripts[path] = scripts[path].replace(
                f'"assets/{png.name}"', f'"assets/{webp.name}"')
    if before:
        print(f"   atlases {before:,} -> {after:,} bytes as lossless WebP")

# Every atlas the page asks for has to be one this build actually ships. This
# is what catches a rewrite that half-happened: a reference the loop above
# missed still names a `.png` that is no longer on disk. The script's copy is
# written back only now, after every rewrite above has run over it.
for path, text in scripts.items():
    path.write_text(text, encoding="utf-8")
wanted = set(re.findall(r'"assets/([^"]+)"', page + "".join(scripts.values())))
have = {p.name for p in assets.iterdir()} if assets.is_dir() else set()
missing = sorted(wanted - have)
if missing:
    raise SystemExit(f"the viewer asks for assets this build does not ship: {', '.join(missing)}")

# Start the largest critical request while the viewer is still parsing. The
# shim starts the worker immediately afterwards, and its `fetch()` shares this
# CORS-enabled preload instead of racing the sprite atlases for a connection.
# The interception still has to be installed before the page's own first script
# runs.
head = "<head>"
if page.count(head) != 1:
    raise SystemExit("the viewer does not have exactly one <head>")
page = page.replace(
    head,
    head + '\n<link rel="preload" href="civvis.wasm" as="fetch" '
    'type="application/wasm" crossorigin fetchpriority="high">'
    '\n<script src="shim.js"></script>',
    1,
)

target.write_text(page, encoding="utf-8")
print(f"   viewer written to {target} ({len(page):,} bytes)")
PY

# The landing and download pages are lane content too, so the same rule
# holds: every link between pages stays inside the lane. Head's sources are
# authored lane-relative already; these needles translate OLDER pinned
# revisions, whose pages linked the site by absolute path, and are therefore
# tolerated as absent. What is enforced is the guard: no root-absolute
# href/src, and no script-built root link, may survive into any lane page.
python3 - "$out" <<'PY'
import re, sys, pathlib

out = pathlib.Path(sys.argv[1])

edits = {
    out / "home" / "index.html": [
        # the photographs, with whatever hand-written ?v= they carried
        ('src="/home/assets/', 'src="assets/'),
        # links into the viewer, one directory up
        ('href="/?', 'href="../?'),
        ('href="/"', 'href="../"'),
        # the same viewer links built by the page's own script
        ('"/?map=', '"../?map='),
        ('`/?map=', '`../?map='),
    ],
    out / "download" / "index.html": [
        ('href="/test/"', 'href="../"'),  # the lane's own viewer
        ('href="/home"', 'href="../home/"'),
        ('href="/"', 'href="../"'),
    ],
}

for path, rules in edits.items():
    text = path.read_text(encoding="utf-8")
    for needle, replacement in rules:
        text = text.replace(needle, replacement)
    leaked = sorted(set(re.findall(r'(?:href|src)="(/[^"]*)"', text)))
    if leaked:
        raise SystemExit(
            f"{path.name} still links the site root ({', '.join(leaked)}); "
            "a lane page must link relatively so /test stays /test"
        )
    if '"/?map=' in text or '`/?map=' in text:
        raise SystemExit(f"{path.name} still builds root-absolute viewer links in its script")
    path.write_text(text, encoding="utf-8")
PY

# Browser caches cannot be purged by a deployment. Give every JS, WASM, and
# atlas dependency a URL derived from its actual bytes, so fresh HTML always
# asks for fresh content while unchanged files remain reusable across builds.
python3 "$repo_root/beta/cache_bust.py" "$out"

wasm_bytes=$(wc -c < "$out/civvis.wasm" | tr -d ' ')
bundle_bytes=$(find "$out" -type f -exec wc -c {} + | tail -1 | awk '{print $1}')
cat > "$out/build.json" <<JSON
{
  "commit": "$commit",
  "short": "$short",
  "commit_time": "$commit_time",
  "built_at": "$built_at",
  "wasm_bytes": $wasm_bytes,
  "bundle_bytes": $bundle_bytes
}
JSON

echo
echo "published build $short -> $out"
echo "  engine      $(printf "%'d" "$wasm_bytes") bytes"
if command -v brotli >/dev/null; then
  echo "  over a wire $(printf "%'d" "$(brotli -q 11 -c "$out/civvis.wasm" | wc -c | tr -d ' ')") bytes brotli"
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
