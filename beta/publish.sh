#!/usr/bin/env bash
#
# Assemble a published build of CIVVIS for civvis.ai.
#
# The output is one complete LANE of the site — the viewer at its root with
# the engine compiled to WebAssembly beside it, the landing page at /home, the
# pages of Charts/ under /charts (AI Insights, Civ AI Interpretability, Civ
# Analytics, Win Analytics, ELO Rankings, CIVVIS Performance), the Experiments page at
# /experiments and the downloads page at /download — a
# plain directory of static files with no
# host-specific anything beyond the two Cloudflare Pages control files, which
# any other host simply ignores. Every link between the pages is
# lane-relative, so the same tree serves unchanged at the site root (the
# stable lane) and under /test (the head lane); the deploy workflow builds one
# lane per revision and stacks them.
#
# A LANE IS ONE REVISION'S SITE AND NOTHING ELSE. Everything a visitor is
# served comes from the revision the lane names: the engine, the viewer, its
# scripts (shim.js and worker.js among them), every page, the script that
# fills a page's figures and the data that script reads. The site is therefore
# two whole sets, one of the promoted revision at / and one of main at /test,
# and nothing reaches / except by a promotion. A page the revision does not
# have is a page the lane does not have. This script, which is the invoking
# tree's, only assembles; the two things it adds that are not the lane's are
# the serving shell (_worker.js and 404.html, one per deployment) and the run
# ledger, the games as played by the time of the build.
#
#   ./beta/publish.sh                  build from the working tree
#   ./beta/publish.sh --commit <ref>   build from a pinned revision
#   ./beta/publish.sh --symbols-out <dir>   retain a separate profiling sidecar
#
# Nothing in `web/` or `src/` is edited. The viewer is *copied* and the copy is
# adjusted for living in a lane; if any of those adjustments stops matching,
# this script fails rather than publishing a page with dead links.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="$repo_root/beta/dist"
commit_ref=""
symbols_out=""

while [ $# -gt 0 ]; do
  case "$1" in
    --commit) commit_ref="${2:?--commit needs a revision}"; shift 2 ;;
    --out) out="${2:?--out needs a directory}"; shift 2 ;;
    --symbols-out) symbols_out="${2:?--symbols-out needs a directory}"; shift 2 ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
done

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"

command -v cargo >/dev/null || { echo "cargo is not on PATH" >&2; exit 1; }
if [ -n "$symbols_out" ]; then
  command -v wasm-opt >/dev/null || { echo "function maps require wasm-opt" >&2; exit 1; }
  mkdir -p "$symbols_out"
  symbols_out="$(cd "$symbols_out" && pwd)"
fi
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

# The module is stamped with its revision and not with the hour. A deployment
# rebuilds the promoted lane from the same commit every time; with the hour
# inside it, two builds of one commit differed in five bytes of sixteen million
# and the module took a new content address on every deployment, so a
# returning visitor downloaded and compiled an engine they already had. The
# hour goes in build.json below, and shim.js hands it to the page from there.
#
# The engine is compiled from a copy in which every opt-in gene no browser
# seat can turn on reads as `false`, so the compiler drops its code. Of the
# registry's 1,061 genes civvis.ai seats about 250; the rest were a quarter of
# the module (measured 2026-11-18: 18.37 -> 13.95 MB, 3.97 -> 3.03 MB brotli,
# identical games, unchanged engine time). `tools/wasm_gene_prune.py` keeps any
# gene it cannot prove off, and on a source it cannot read it changes
# nothing. The copy is a scratch tree — `src/` here is never edited — and
# CIVVIS_PRUNE_GENES=0 builds the engine whole.
engine_tree="$source_tree"
if [ "${CIVVIS_PRUNE_GENES:-1}" != 0 ] && ! command -v rsync >/dev/null; then
  echo "   rsync is not installed; building the engine with every gene"
elif [ "${CIVVIS_PRUNE_GENES:-1}" != 0 ]; then
  if [ -z "$scratch" ]; then
    scratch="$(mktemp -d)"
    trap 'rm -rf "$scratch"' EXIT
  fi
  if [ "$source_tree" = "$repo_root" ]; then
    # A pinned revision is already a throwaway worktree; the working tree is
    # copied, uncommitted edits included, with the build cache kept shared.
    engine_tree="$scratch/engine"
    mkdir -p "$engine_tree"
    rsync -a --exclude /target --exclude /.git --exclude /docs --exclude /beta/dist \
      "$source_tree/" "$engine_tree/"
    export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo_root/target}"
  fi
  python3 "$repo_root/tools/wasm_gene_prune.py" "$engine_tree"
fi

echo "==> building the engine for wasm32 from $short"
(
  cd "$engine_tree"
  CIVVIS_COMMIT="$commit" CIVVIS_COMMIT_TIME="$commit_time" CIVVIS_BUILT_AT="" \
    cargo rustc --lib \
    --target wasm32-unknown-unknown --release --crate-type cdylib
)

raw_wasm="${CARGO_TARGET_DIR:-$engine_tree/target}/wasm32-unknown-unknown/release/civvis.wasm"
[ -f "$raw_wasm" ] || { echo "the wasm build produced nothing at $raw_wasm" >&2; exit 1; }

rm -rf "$out"
mkdir -p "$out"

if command -v wasm-opt >/dev/null; then
  echo "==> shrinking the module"
  # -O3 keeps the optimiser aimed at speed: this module simulates whole games,
  # and -Oz costs more turn time than it saves bytes over the wire.
  # Measured 2026-11-18 at 241674c, three paired full games a side with
  # identical trace and save hashes: rustc `opt-level = "s"` shrank the module
  # 18.37 -> 12.50 MB (3.97 -> 3.05 MB brotli) and cost about 23% engine time
  # (median 94.7 s against 76.9 s a game). wasm-opt alone barely matters:
  # -O3, -Os and -Oz on the same input land within 0.5% of each other.
  if [ -n "$symbols_out" ]; then
    # The printing pass leaves the optimized module byte-for-byte unchanged.
    # Its indexes name the final functions, after inlining and reordering.
    wasm-opt -O3 "$raw_wasm" --symbolmap -o "$out/civvis.wasm" > "$symbols_out/functions.map"
    python3 "$repo_root/tools/site_wasm_symbols.py" \
      "$out/civvis.wasm" "$symbols_out/functions.map" "$commit"
  else
    wasm-opt -O3 "$raw_wasm" -o "$out/civvis.wasm"
  fi
else
  echo "==> wasm-opt is not installed; publishing the unshrunk module" >&2
  cp "$raw_wasm" "$out/civvis.wasm"
fi

echo "==> assembling the pages"
# The viewer's two scripts of its own are the lane's like the rest of it: the
# shim speaks to the engine built above, so it has to be that revision's.
for script in shim.js worker.js; do
  [ -f "$source_tree/beta/$script" ] \
    || { echo "$short has no beta/$script; it cannot be published as a lane" >&2; exit 1; }
done
cp "$source_tree/beta/shim.js" "$source_tree/beta/worker.js" "$out/"
cp -R "$source_tree/web/assets" "$out/assets"
# Every page of the site exists once per lane, built from the lane's own
# revision and from nothing else: the promoted stable lane shows the promoted
# pages, and /test shows head's. A pinned revision that predates one of the
# pages does not have that page. (Until 2026-11-22 it was given head's copy,
# and a page nobody had promoted answered on the front page: the AI
# Interpretability page did, the day it was written.) `pages` is the list of
# what this lane carries, written into build.json for the checks to hold the
# bundle to.
landing_page="$source_tree/beta/landing.html"
landing_assets="$source_tree/beta/assets"
download_page="$source_tree/beta/download.html"
tournaments_page="$source_tree/beta/tournaments.html"
research_page="$source_tree/beta/research.html"
elo_page="$source_tree/beta/elo.html"
civs_page="$source_tree/beta/civs.html"
performance_page="$source_tree/beta/performance.html"
interpretability_page="$source_tree/beta/interpretability.html"
insights_page="$source_tree/beta/insights.html"
pages=()
without=()

# Where a page is served is the lane's own revision's to say, like the page
# itself: `address <page> <the address it had before pages said theirs>`.
#
# Since 2026-11-23 an address follows the page's name in the site list
# (operator: "clean up the URL please. standardized. please remove research
# from the url"): Win Analytics is at charts/win-analytics where it was at
# research, and Auto-Tourneys at auto-tourneys where it was at tournaments.
# On 2026-11-24 Auto-Tourneys took its first name back, Tournaments, and with
# it its first address (operator: "change auto-outnrye to Tournaments.
# Auto-tourney suggests some self imrpvoing tourney thing"). On 2026-11-25 it
# was named Experiments and went to experiments (operator: "rename
# tournaments to experiments"). The page's file and its record keep the
# names they were written under (beta/tournaments.html, data/tournaments.json).
# Every link inside a page is relative, so a page works at one depth only,
# and this script, which is the invoking tree's, lays out both lanes. A page
# of a pinned revision placed at main's address would be a page of dead
# links. So each page states its own address, in its `<link rel="canonical">`,
# and is put there; a page from before pages said so keeps the address it
# was written for.
address() {
  local page="$1" former="$2" stated=""
  if [ -f "$page" ]; then
    stated="$(grep -o -m 1 '<link rel="canonical" href="https://civvis\.ai/[a-z][a-z0-9/-]*/">' "$page" || true)"
  fi
  if [ -z "$stated" ]; then
    printf '%s' "$former"
    return 0
  fi
  stated="${stated#*civvis.ai/}"
  stated="${stated%/\">}"
  # One name, or a folder and a name: what `_worker.js` reads as a page.
  if ! [[ "$stated" =~ ^[a-z][a-z0-9-]*(/[a-z][a-z0-9-]*)?$ ]]; then
    echo "${page#"$source_tree"/} states the address '$stated', which is not a page address" >&2
    exit 1
  fi
  printf '%s' "$stated"
}
home_at="$(address "$landing_page" home)"
tournaments_at="$(address "$tournaments_page" tournaments)"
research_at="$(address "$research_page" research)"
elo_at="$(address "$elo_page" elo)"
civs_at="$(address "$civs_page" civs)"
performance_at="$(address "$performance_page" performance)"
interpretability_at="$(address "$interpretability_page" interpretability)"
# AI Insights was written after pages stated their addresses, so it has no
# former one: a revision without the page has no page to place.
insights_at="$(address "$insights_page" charts/ai-insights)"
download_at="$(address "$download_page" download)"

# A lane built from some other revision than the invoking tree's is pinned:
# the stable lane, on every deployment.
pinned_elsewhere=0
if [ "$commit" != "$(git -C "$repo_root" rev-parse HEAD)" ]; then pinned_elsewhere=1; fi

# Fill a page's figures with the lane's own script, read against the lane's
# own tree: `fill <script in beta/> <page>`. The page, the script that fills
# it and the snapshots under data/ are one revision's, so the record always
# has the shape the page reads. Until 2026-11-22 the script and most of the
# data were head's in both lanes, which moved the front page's figures with
# every merge and handed a promoted page a record written for a later one.
#
# What still moves between two builds of one revision is the run ledger
# (`results.jsonl` on the `ledger` branch): the games played are not a
# property of any revision, so each lane counts them as of the build, by its
# own script.
#
# A script that fails stops the publish when it is the invoking tree's own,
# because that is a fault main can mend. A pinned revision's script cannot be
# mended, only promoted past, and the ledger it reads goes on being written
# after it: a failure there costs its page the figures (the slot stays
# `null`, which every page reads as "no record"), never the deployment,
# which would hold /test back too.
fill() {
  local script="$source_tree/beta/$1" page="$2"
  if [ ! -f "$script" ]; then
    echo "   $short has no beta/$1; ${page#"$out"/} keeps the slot it came with"
    return 0
  fi
  if [ "$pinned_elsewhere" = 1 ]; then
    python3 "$script" "$source_tree" "$page" \
      || echo "::warning::$short: beta/$1 failed on ${page#"$out"/}; the page is published without its figures"
  else
    python3 "$script" "$source_tree" "$page"
  fi
}

if [ -f "$landing_page" ]; then
  mkdir -p "$out/$home_at"
  cp "$landing_page" "$out/$home_at/index.html"
  # The home page is six videos, and its fastest game is whichever record
  # leads the lane's data/performance/fastest_game_videos.json: it is written
  # into the page here rather than committed as a copy that could drift.
  # Over the videos stands the Firaxis Civ 6 AI benchmark, the win rates by
  # level that Win Analytics opens on, written into the page's own slot by
  # the same run from the run ledger. A
  # home page from before 2026-11-22 carries the two rankings instead (the
  # winningest civs of the verification games, the Civ AI Agents by Elo) and
  # is filled with those, the civs from the run ledger and the agents from
  # the lane's own revision; a page with neither slot is left as it is. A
  # home page from before the speed timeline went to CIVVIS Performance
  # (2026-11-24) still carries it, and its own revision's copy of the script
  # fills it; since then it is written with that page's record, below.
  fill home_rankings.py "$out/$home_at/index.html"
  pages+=("$home_at")
else
  without+=(home)
fi
# The Experiments page (running, scheduled and finished series of the
# verification games) is filled the same way: the definitions in the lane's
# data/tournaments.json, the games from the run ledger. An experiment
# scheduled on main is listed on /test at once and on the front page when it
# is promoted.
if [ -f "$tournaments_page" ]; then
  mkdir -p "$out/$tournaments_at"
  cp "$tournaments_page" "$out/$tournaments_at/index.html"
  fill tournaments_page.py "$out/$tournaments_at/index.html"
  pages+=("$tournaments_at")
else
  without+=(experiments)
fi
# AI Insights (what the Civ games show about AI at large, an insight at a
# time; the first is the games against Civilization VI's own AI ending sooner
# at each level from Lv 5 to Lv 8) is the first page under Charts/ since
# 2026-11-24 (operator: "the old ai interpretability should be in the process
# of being renamed civ ai interpretability. we want a new ai insights"), and
# so the page /charts opens: the worker opens a folder's first page in the
# lane's list. It is filled the same way: the games against the AI from the
# run ledger, counted by the lane's own Win Analytics script.
if [ -f "$insights_page" ]; then
  mkdir -p "$out/$insights_at"
  cp "$insights_page" "$out/$insights_at/index.html"
  fill insights_page.py "$out/$insights_at/index.html"
  pages+=("$insights_at")
else
  without+=(ai-insights)
fi
# The Civ AI Interpretability page (the steps of one turn in order, the plan the
# seats write down through a game, and every gene with what it is worth in
# wins) is the second page under Charts/, and was the first from 2026-11-24
# (operator: "move ai interpretablity to the top of the charts/ then civ
# analytics, win analytics. elo rankings.. civvis performanc") until AI
# Insights was put over it later that day. It is
# filled the same way, from the lane's source (the turn's steps, the gene
# ranking) and the lane's snapshot of the seats' plans in
# data/interpretability/: the agent as it is in the revision the lane plays.
if [ -f "$interpretability_page" ]; then
  mkdir -p "$out/$interpretability_at"
  cp "$interpretability_page" "$out/$interpretability_at/index.html"
  fill interpretability_page.py "$out/$interpretability_at/index.html"
  pages+=("$interpretability_at")
else
  without+=(interpretability)
fi
# Civ Analytics, the third under Charts/ (operator 2026-11-23: "add another
# page: Civ Analytics. First can be the civ records (pulled from win
# analytics), then early build order"): both off the run ledger, the tree of build
# orders as Win Analytics' script reads it.
if [ -f "$civs_page" ]; then
  mkdir -p "$out/$civs_at"
  cp "$civs_page" "$out/$civs_at/index.html"
  fill civs_page.py "$out/$civs_at/index.html"
  pages+=("$civs_at")
else
  without+=(civ-analytics)
fi
# Win Analytics, the fourth page under Charts/ and the first until 2026-11-24
# (when in the game each victory type lands, in self-play and against the
# Deity AI; `beta/research.html`, by the name the page had first), is filled the same way: the self-play games
# from the lane's snapshot in data/research/, the Deity games from the run
# ledger.
if [ -f "$research_page" ]; then
  mkdir -p "$out/$research_at"
  cp "$research_page" "$out/$research_at/index.html"
  fill research_page.py "$out/$research_at/index.html"
  # The two rankings stood on this page from 2026-11-22 until they moved to
  # ELO Rankings and Civ Analytics on 2026-11-23. A page of a revision that
  # still has their slot is filled as it was; one without it is left as it is.
  fill home_rankings.py "$out/$research_at/index.html"
  pages+=("$research_at")
else
  without+=(research)
fi
# ELO Rankings, the fifth page under Charts/ (operator 2026-11-23: "add
# that to a new page: Charts/ELO Rankings. this should be after win
# analytics in the ordering"): the Civ AI Agents by Elo and Civilization
# VI's own AI at Lv 4 to Lv 8, now and over time. Filled the same way: the
# agents from the lane's own data/strategy_elo.json and the history of its
# fits, the Firaxis levels from the benchmark games on the run ledger, so
# they move with every finished game.
if [ -f "$elo_page" ]; then
  mkdir -p "$out/$elo_at"
  cp "$elo_page" "$out/$elo_at/index.html"
  fill elo_page.py "$out/$elo_at/index.html"
  pages+=("$elo_at")
else
  without+=(elo-rankings)
fi
# The Performance page (how long a game takes with CIVVIS at the controls of
# Civilization VI, day by day; the fastest game on video; how long a
# self-play game takes at five percentiles, and game time batch by batch) is
# CIVVIS's own page in the site list. It is filled the same way: the computer
# control games from the run ledger, the rest from the lane's record videos
# and the lane's snapshot of the games' times in data/performance/.
if [ -f "$performance_page" ]; then
  mkdir -p "$out/$performance_at"
  cp "$performance_page" "$out/$performance_at/index.html"
  fill performance_page.py "$out/$performance_at/index.html"
  pages+=("$performance_at")
else
  without+=(performance)
fi
# The home page carries no art of its own any more, so there is usually no
# directory to ship; a pinned revision from when it had photographs still does.
if [ -f "$landing_page" ] && [ -d "$landing_assets" ]; then cp -R "$landing_assets" "$out/$home_at/assets"; fi
# `/download/` rather than `/download.html`: the page outlives any one release
# and gets linked around, so it should have the tidier address.
if [ -f "$download_page" ]; then
  mkdir -p "$out/$download_at"
  cp "$download_page" "$out/$download_at/index.html"
  pages+=("$download_at")
else
  without+=(download)
fi
# Wizard Chess is set aside and no lane carries it (operator 2026-11-24:
# "lets set aside wizard chess right now. please remove from civvis.ai"). It
# was published at /wizard-chess from 2026-11-23. The game stays in the tree
# (beta/chess.html, beta/chess/, docs/WIZARD_CHESS.md); nothing here copies it.
# Chaos Monkey likewise (operator 2026-11-26: "remove chaos monkey for now").
# It was published at /chaos-monkey on 2026-11-25. Its page is gone from the
# tree; the engine's dial, `civvis chaos-monkey`, tools/chaos_monkey.py and
# its record data/chaos_monkey.json stay, so putting it back is a revert.
if [ "${#without[@]}" -gt 0 ]; then
  echo "   $short does not have every page, and its lane is published without: ${without[*]}"
fi
# The gate travels *inside* the deployed directory. See beta/_worker.js for
# why this is not a `functions/` directory. These two are the serving shell
# and the one part of a deployment that is not a lane's: a deployment has a
# single request handler, which routes both lanes, so there cannot be one of
# it per lane. Always the invoking tree's copy, and the deploy workflow keeps
# only the root copy when it stacks two lanes.
cp "$repo_root/beta/_worker.js" "$out/_worker.js"
# Its very existence switches Pages from single-page-app fallback (unknown
# path -> the front page, 200) to honest 404s. See the comment inside it.
cp "$repo_root/beta/404.html" "$out/404.html"

# The viewer, copied and then made lane-relative. Each substitution is
# checked, because a silently unmatched one publishes a page whose strategic
# map sprites are simply missing.
python3 - "$source_tree/web/index.html" "$out/index.html" "$out/assets" ${pages[@]+"${pages[@]}"} <<'PY'
import re, sys, pathlib

source, target = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
assets = pathlib.Path(sys.argv[3])
# The addresses of the pages this lane carries, as its own revision states
# them (`address` above).
carried = sys.argv[4:]
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
#
# The addresses are the lane's own: a viewer links the pages of its own
# revision, which are where that revision says they are (`charts/win-analytics`
# on main, `research` in a revision from before 2026-11-23). The six names
# every page had before pages stated their addresses stay listed, for a
# revision whose viewer links a page its tree keeps somewhere this script does
# not look.
former = ["home", "research", "tournaments", "performance", "interpretability", "download"]
# A page set aside is carried by no lane, and a revision from the days it was
# on the site still links it from its viewer: Wizard Chess, at /wizard-chess
# from 2026-11-23 until it was set aside on 2026-11-24. That link is made
# lane-relative like every other, so it reaches the lane's own 404 until a
# promotion brings a viewer without it. Left as it was, it stopped every
# publish from the hour the page was set aside (#5793): the front page was
# pinned to such a revision, and the guard below stops a publish on any link
# to the site root. Nothing of the page is copied; this only names where an
# older viewer's link pointed. Chaos Monkey, at /chaos-monkey on 2026-11-25,
# was taken off the site on 2026-11-26 the same way.
set_aside = ["wizard-chess", "chaos-monkey"]
former += set_aside
for address in dict.fromkeys([*carried, *former]):
    needle, replacement = f'href="/{address}"', f'href="{address}/"'
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

# The sprite atlases, republished as WebP: lossless unless named below.
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
#
# Before any of that, the copy keeps only what the viewer asks for. `web/assets`
# is copied whole, and art the viewer stopped drawing stays there for the
# desktop build: on 2026-11-18 the two feature atlases #4865 retired were still
# 1.7 MB of every deployment, fetched by nobody. A reference is any quoted
# `assets/...` or CSS `url(assets/...)` in the page or a script; the check
# after the WebP pass still fails a build whose viewer names a file it lacks.
referenced = set(re.findall(
    r"""(?:["'`]|url\(\s*)assets/([^"'`)?#\s]+)""", page + "".join(scripts.values())))
unreferenced = sorted(p for p in assets.iterdir() if p.is_file() and p.name not in referenced)
for stale in unreferenced:
    scripts.pop(stale, None)
    stale.unlink()
if unreferenced:
    print(f"   left out {len(unreferenced)} asset(s) the viewer never asks for: "
          + ", ".join(p.name for p in unreferenced))

# Lossy where nobody can see the loss: the hidden-map marginalia are ink
# engravings painted under a multiply blend at a fraction of their 256 px
# cells. At quality 80 they are 195 KB instead of 881 KB lossless (measured
# 2026-11-18), with no visible difference even at full cell size.
LOSSY_QUALITY = {"hidden-map-monsters.png": 80}
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
        if png.name in LOSSY_QUALITY:
            image.save(webp, "WEBP", quality=LOSSY_QUALITY[png.name], method=6)
        else:
            image.save(webp, "WEBP", lossless=True, quality=100, method=5)
        before += png.stat().st_size
        after += webp.stat().st_size
        png.unlink()
        page = page.replace(f'"assets/{png.name}"', f'"assets/{webp.name}"')
        for path in scripts:
            scripts[path] = scripts[path].replace(
                f'"assets/{png.name}"', f'"assets/{webp.name}"')
    if before:
        print(f"   atlases {before:,} -> {after:,} bytes as WebP")

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

# The viewer's scripts, without their comments and indentation. The source is
# written to be read — app.js was 2.0 MB, about half of it prose — and none
# of that reaches a browser usefully: measured 2026-11-18, stripping it took
# app.js from 471 KB to 261 KB brotli. Whitespace only: no identifier is
# renamed and no expression rewritten, so the scripts' shared top-level names
# and every string the page or a check reads survive unchanged.
#
# Optional like Pillow: a machine without Node publishes the scripts as
# written. The version is pinned so every build strips the same way.
ESBUILD="esbuild@0.24.0"
if command -v npx >/dev/null && compgen -G "$out/assets/*.js" >/dev/null; then
  minified="$(mktemp -d)"
  scripts_before=$(cat "$out"/assets/*.js | wc -c | tr -d ' ')
  npx --yes "$ESBUILD" "$out"/assets/*.js --minify-whitespace --charset=utf8 \
    --legal-comments=none --log-level=warning --outdir="$minified"
  for script in "$out"/assets/*.js; do
    [ -s "$minified/$(basename "$script")" ] \
      || { echo "esbuild wrote nothing for $(basename "$script")" >&2; exit 1; }
    mv "$minified/$(basename "$script")" "$script"
  done
  rmdir "$minified"
  echo "   viewer scripts $(printf "%'d" "$scripts_before") -> $(printf "%'d" "$(cat "$out"/assets/*.js | wc -c | tr -d ' ')") bytes without comments"
else
  echo "   npx is not installed; publishing the viewer scripts as written"
fi

# The landing and download pages are lane content too, so the same rule
# holds: every link between pages stays inside the lane. Head's sources are
# authored lane-relative already; these needles translate OLDER pinned
# revisions, whose pages linked the site by absolute path, and are therefore
# tolerated as absent. What is enforced is the guard: no root-absolute
# href/src, and no script-built root link, may survive into any lane page.
#
# And, since 2026-11-23, that a relative link lands on something. The pages
# are no longer all one directory deep (the pages of Charts/ sit under
# charts/), so a link written for the wrong depth (`../home/` on a page two
# deep) stays inside the lane and reaches nothing. Every plain relative
# href/src of a page is therefore resolved from the page's own address, and
# has to come out at the viewer, at a page the lane carries, or at a file the
# lane ships. A dead one stops the publish when the revision is the invoking
# tree's own, which main can mend; a pinned revision's is reported and
# published as it is, like its failing script.
python3 - "$out" "$pinned_elsewhere" "$home_at" "$download_at" ${pages[@]+"${pages[@]}"} <<'PY'
import posixpath, re, sys, pathlib

out = pathlib.Path(sys.argv[1])
pinned_elsewhere = sys.argv[2] == "1"
home_at, download_at = sys.argv[3], sys.argv[4]
# Every page the lane carries, by its address: the guard holds for each one,
# and a page added to a lane is covered the day it is added.
carried = sys.argv[5:]

translations = {
    home_at: [
        # the photographs, with whatever hand-written ?v= they carried
        ('src="/home/assets/', 'src="assets/'),
        # links into the viewer, one directory up
        ('href="/?', 'href="../?'),
        ('href="/"', 'href="../"'),
        # the same viewer links built by the page's own script
        ('"/?map=', '"../?map='),
        ('`/?map=', '`../?map='),
    ],
    download_at: [
        ('href="/test/"', 'href="../"'),  # the lane's own viewer
        ('href="/home"', 'href="../home/"'),
        ('href="/"', 'href="../"'),
    ],
    # Every other page was authored lane-relative from its first day, so
    # there is nothing to translate.
}

# A link this check can read: a plain relative address. One with a scheme
# (https:, data:, mailto:) leaves the site, and one a script puts together
# (quotes, `${...}`) is not an address until the script has run.
PLAIN = re.compile(r"[A-Za-z0-9._~/?#=&%+-]+")


def lands(address, link):
    target = link.split("#", 1)[0].split("?", 1)[0]
    if not target:
        return True  # the page itself, at an anchor or with a query
    where = posixpath.normpath(posixpath.join(address, target))
    if where == ".":
        return True  # the lane's root: the viewer
    if where == ".." or where.startswith("../"):
        return False  # out of the lane
    return where in carried or (out / where).is_file()


for address in carried:
    path = out / address / "index.html"
    named = f"{address}/index.html"
    text = path.read_text(encoding="utf-8")
    for needle, replacement in translations.get(address, []):
        text = text.replace(needle, replacement)
    leaked = sorted(set(re.findall(r'(?:href|src)="(/[^"]*)"', text)))
    if leaked:
        raise SystemExit(
            f"{named} still links the site root ({', '.join(leaked)}); "
            "a lane page must link relatively so /test stays /test"
        )
    if '"/?map=' in text or '`/?map=' in text:
        raise SystemExit(f"{named} still builds root-absolute viewer links in its script")
    dead = sorted(
        link for link in set(re.findall(r'(?:href|src)="([^"]*)"', text))
        if PLAIN.fullmatch(link) and not lands(address, link)
    )
    if dead:
        said = (f"{named} links {', '.join(dead)}, which this lane does not have "
                f"(a relative link is read from the page's own address, {address}/)")
        if not pinned_elsewhere:
            raise SystemExit(said)
        print(f"::warning::{said}")
    path.write_text(text, encoding="utf-8")
PY

# Browser caches cannot be purged by a deployment. Give every JS, WASM, and
# atlas dependency a URL derived from its actual bytes, so fresh HTML always
# asks for fresh content while unchanged files remain reusable across builds.
python3 "$repo_root/beta/cache_bust.py" "$out"

# Every compressible file of the game page but the page itself, at brotli's
# slowest setting. `_worker.js` serves the `.br` beside a file when the
# browser accepts it; Cloudflare's own on-the-fly compression favours speed
# and comes out about 30% larger on the engine. They are alternate encodings
# of files already counted, so the on-disk budget below does not count them.
#
# HTML is the exception: Cloudflare re-encodes every text/html response at
# its edge, whatever the Worker sends. Measured on /test (2026-11-18): an
# `index.html.br` of 85,864 bytes was served as 109,582 bytes of brotli, the
# page byte-identical, and brotli quality 4 of that page is 109,317. So no
# copy is written for it, and the count below prices it at quality 4.
viewer_files() {
  { find "$out/civvis.wasm" "$out/index.html" "$out/shim.js" "$out/worker.js" "$out/assets" \
      -type f ! -name '*.br' 2>/dev/null || true; } | sort
}
if command -v brotli >/dev/null; then
  viewer_files | while read -r file; do
    case "$file" in
      *.wasm|*.js|*.json|*.css) brotli -q 11 --lgwin=24 -f -o "$file.br" "$file" ;;
    esac
  done
fi

# The Build size civvis.ai shows: what a browser downloads for the game page —
# each file's brotli copy where there is one, the page at the quality the edge
# re-encodes it with, the file itself otherwise (the images are already
# compressed). It counts the whole viewer, the map art a played game fetches
# later included, and nothing the landing or download pages carry.
download_bytes=0
while read -r file; do
  if [ -f "$file.br" ]; then
    bytes=$(wc -c < "$file.br")
  elif [[ "$file" == *.html ]] && command -v brotli >/dev/null; then
    bytes=$(brotli -q 4 -c "$file" | wc -c)
  else
    bytes=$(wc -c < "$file")
  fi
  download_bytes=$(( download_bytes + $(echo "$bytes" | tr -d ' ') ))
done < <(viewer_files)

wasm_bytes=$(wc -c < "$out/civvis.wasm" | tr -d ' ')
bundle_bytes=$(find "$out" -type f ! -name '*.br' -exec wc -c {} + | tail -1 | awk '{print $1}')
# `pages` is the lane's own list of what it carries beside the viewer: the
# checks (beta/verify.py) hold the bundle to exactly these, so a page from
# another revision cannot be slipped into a lane, and a lane cannot lose one.
pages_json=""
for page in ${pages[@]+"${pages[@]}"}; do pages_json="$pages_json${pages_json:+, }\"$page\""; done
cat > "$out/build.json" <<JSON
{
  "commit": "$commit",
  "short": "$short",
  "commit_time": "$commit_time",
  "built_at": "$built_at",
  "pages": [$pages_json],
  "wasm_bytes": $wasm_bytes,
  "download_bytes": $download_bytes,
  "bundle_bytes": $bundle_bytes
}
JSON

echo
echo "published build $short -> $out"
echo "  engine      $(printf "%'d" "$wasm_bytes") bytes"
if [ -f "$out/civvis.wasm.br" ]; then
  echo "  over a wire $(printf "%'d" "$(wc -c < "$out/civvis.wasm.br" | tr -d ' ')") bytes brotli"
fi
echo "  download    $(printf "%'d" "$download_bytes") bytes for the whole game page"
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
