# civvis.ai — the published build

CIVVIS runs as a desktop program: a Rust engine with a local HTTP server, and
`web/index.html` talking to it over `fetch`. This directory publishes that same
program to the web with **nothing running behind it** — the engine is compiled
to WebAssembly and the page answers its own requests.

That is what makes the hosting free and what makes it honest. There is no
server to pay for, no game state anywhere but the visitor's own tab, and the
viewer is not a reimplementation: it is `web/index.html`, copied byte for byte
apart from the asset paths a subdirectory forces.

```
civvis.ai                 the STABLE lane — the promoted build, moved by a person
civvis.ai/home            its landing page
civvis.ai/research        its Charts page (named Research, then Musings; the address stayed): charts read off the games, first when each victory type lands
civvis.ai/tournaments     its Auto-Tourneys page (named Tournaments until 2026-11-22; the address stayed): series of verification games, running, scheduled and finished
civvis.ai/performance     its Performance page, Charts' own: the fastest game on video and how long a self-play game takes
civvis.ai/download        its native-binaries page, from the latest GitHub release
civvis.ai/test            the HEAD lane — latest main, republished automatically
civvis.ai/test/home       head's landing page
civvis.ai/test/research   head's Charts page
civvis.ai/test/tournaments  head's Auto-Tourneys page
civvis.ai/test/performance  head's Performance page
civvis.ai/test/download   head's binaries page
```

**The site is the same set of pages twice, from two commits.** Each lane is a
complete tree — the viewer at its root, `/home`, `/research`, `/tournaments`, `/performance` and `/download` beside it —
and every link inside a lane is lane-relative, so reviewing `/test` means
clicking around `/test` and never landing in prod. `/test` follows `main` on
a schedule so the newest site is always reviewable without anyone deciding
anything; `/` is whatever the `site-stable` tag names and moves only when
somebody runs the **to-prod-manual-only** workflow, because whether a build is
worth being the front page is a judgement no gate can make. Each lane states
its commit in its own `build.json`. Both lanes ship in every deployment —
Pages deployments are immutable snapshots of a whole directory, so there is no
such thing as updating half a site; a promotion is nothing but moving the tag
and deploying again, which promotes every page at once.

Repeat visitors cannot be left on an earlier deployment. Each lane's HTML is a
moving pointer and revalidates on every visit. The publisher gives `shim.js`,
`worker.js`, `civvis.wasm`, and every referenced atlas a query version derived
from that file's bytes. A changed file therefore has a new URL and is fetched
after the fresh page arrives; an unchanged file keeps its URL and can be reused
from cache. Legacy unversioned URLs also revalidate. An already-open tab keeps
running its loaded build until it is reloaded.

(`/rust` and `/wasm`, the old build-channel redirects, were dropped in favour
of the real addresses above; they 404 like any other retired URL. The
installed desktop apps' local `/wasm/` channel is unrelated and remains.)

`/` serves the product directly. Setting `ROOT_REDIRECT` in the Pages
environment turns the root into a **302** to anywhere — the escape hatch that
once pointed the domain at the YouTube channel — and unsetting it (or `off`)
serves the site again; neither needs a deploy. The redirect is deliberately
temporary: browsers cache a 301 effectively for ever, and this pointer exists
to be movable.

## How it fits together

| Piece | What it does |
| --- | --- |
| `src/wasm.rs` | The engine's request router for the browser. A child module of `server`, `cfg`-gated to wasm, answering the same endpoints over the same JSON. |
| `beta/worker.js` | Runs the module off the main thread. A turn is not a quick call, and the viewer paints on `requestAnimationFrame`; on the page's own thread the engine would stall the frames it exists to produce. |
| `beta/shim.js` | Intercepts `fetch` before it reaches the network. Also owns the three things that genuinely became the page's job: the turn clock, the selected between-game finale countdown, and saved games in IndexedDB. |
| `beta/_worker.js` | The whole site's routing: the two lanes, the optional gates on `/` and `/test`, and the response headers. |
| `beta/landing.html` | `civvis.ai/home` (and `/test/home` from head's revision) — the landing page, one of the site's five pages: the list of all five beside its title (Home, in gold because it is this page; Charts/; Auto-Tourneys; CIVVIS, the simulation the AIs play; and AI Debugger, the same game with a seat of your own through `?mode=play`) with a Civ 6 link to the game's shop page under them, the project's links (GitHub and YouTube in their own colours, GitHub saying "Everything open source, free", and under YouTube the channel's two top playlists, Lv 8 Deity Wins and Lv 1 - 7 Archive, one to a line; the same on every page), one paragraph and four videos, two by two: the fastest game upper left (whichever record leads `data/performance/fastest_game_videos.json`, written into the page's `home-videos` slot by `home_rankings.py` at publish), a single player's view of a game upper right (from 6 seconds in, its `data-start`), the AI Debugger lower left and a Lv 8 Deity win lower right. A tile is the video's title and under it the video's picture, which gives its place to the YouTube player, playing without sound, once the tile is on screen (a visitor who asked for reduced motion keeps the pictures and presses one to play it). The two rankings that stood here moved to Win Analytics on 2026-11-22. The viewer's masthead carries the same six in the same order down its left side, with whichever of CIVVIS and AI Debugger it is showing in gold. The list is a tree that keeps its branches folded: a page's own pages are listed under it only while it is the page on screen. Charts has one, Performance (`beta/performance.html`), which Charts lists under itself and which lists itself there, in gold; the viewer (CIVVIS and AI Debugger, the two tools) lists no branch. |
| `beta/research.html` | `civvis.ai/research` (and `/test/research`) — Win Analytics, the first page under Charts/ and the one Charts/ opens (the page was named Research, then Musings, then Charts, until 2026-11-22: the address and these file names did not move). Charts/ stands between Home and Auto-Tourneys in every page's list, with its two pages, Win Analytics and CIVVIS Performance, under it on those two pages: charts read off the games the CIVVIS AI agents play. The page opens on a table of CIVVIS's win rate against the game's AI at each level from Lv 4 Prince to Lv 8 Deity, over the finished games of the tournaments Auto-Tourneys lists, with the games, wins and losses beside each rate, on the last 30 days (7 days and every game are its other two windows). Under it, in the same wide column, are the graphs, CIVVIS self-play's first and the games against Civilization VI's own AI under it, each game length split by victory type; to their right, in a narrower column, stand the two rankings that were the home page's until 2026-11-22, the Civ AI Agents by Elo over the winningest civs (the `rankings-data` slot, filled by `home_rankings.py`). (Until later on 2026-11-22 the page was two equal columns, the AI's games on the left and self-play on the right.) The first is when each victory type lands, by Online-speed turn, in CIVVIS against Civilization VI's own AI and in CIVVIS self-play (every seat an agent, in the simulator), six players on Continents to 250 turns. A slider over the games against the AI picks the AI's level, Lv 1 Settler to Lv 8 Deity (the page opens on Deity), and the line of counts under their name gives CIVVIS's win rate at it. The games against the AI are three charts of stacked columns per span of turns (every victory, the AI's wins, CIVVIS's wins) and self-play is one; then a small chart per victory with both kinds as lines, and the figures as tables. Each kind of game counts a window of its own, last 7 days, last 30 days or every game: self-play opens on 7 days and the games against the AI on 30; 5- or 10-turn spans. |
| `beta/research_page.py` | That page's figures. `publish.sh` runs it and writes the result into the page's `research-data` slot. Self-play comes from the snapshot `data/research/self_play_endings.json` (head's, in both lanes), which `research_page.py --self-play <state dir>/batches data/research/self_play_endings.json` rebuilds from the gene-screen batch rows on the machine that plays them; the Deity games come from `results.jsonl` on the `ledger` branch, each kept only when its run summary there says Online speed and a limit of at least 250 turns. The table of win rates (`tournament_rates`) is counted off the same result lines by the tournaments' definitions, `data/tournaments.json`, through `home_rankings.tournament_series`, so it holds the games the Auto-Tourneys page holds. What it cannot read costs the page its figures, never the publish. |
| `beta/performance.html` | `civvis.ai/performance` (and `/test/performance`) — CIVVIS Performance, the second page under Charts/ in the list: it stands under Charts/, set in, after Win Analytics, on that page and on itself and on no other page. How fast the simulator plays a complete game. First the fastest game on video: the record's picture, which plays the YouTube video in its place when pressed (nothing of YouTube's but that picture is asked for before then), its time as the one large figure, and the earlier records under it. Then how long a self-play game takes (every seat an agent, six players on Continents, Online speed, 250 turns): a tile each for the 1st, 10th, 50th, 90th and 99th percentile of the games' wall-clock seconds, and the games as columns per span of time with the five marked; latest batch, last 7 days or every game. Then game time batch by batch, the middle game as a line inside two bands (p10 to p90, p1 to p99), with every batch's figures as a table. |
| `beta/performance_page.py` | That page's record. `publish.sh` runs it and writes the result into the page's `performance-data` slot (head's, in both lanes). **A new record video is a line in `data/performance/fastest_game_videos.json`** (its YouTube link, the seconds it shows a game played in, that time as its title words it, the day it was posted, its title): the one with the fewest seconds is the page's video. The times come from the snapshot `data/performance/self_play_times.json`, which `performance_page.py --self-play <state dir>/batches data/performance/self_play_times.json` rebuilds from the gene-screen batch rows on the machine that plays them (a game per seed, its `secs`; finished batches only). The snapshot holds the percentiles and a count of games per span of time, taken on the games themselves, not the games. What it cannot read costs the page its figures, never the publish. |
| `beta/tournaments.html` | `civvis.ai/tournaments` (and `/test/tournaments`) — the Auto-Tourneys page (named Tournaments until 2026-11-22; the address and these file names did not move), between Charts and CIVVIS in every page's list. A tournament is one idea played out: a set number of verification games under fixed conditions. The page shows the one running (record, a square per game in the order played, wins by victory, the latest games), the ones scheduled after it, and the finished ones in the table that stood on the home page until 2026-11-21. Each tournament's title line is its number and its short description, "Tournament 4: 50 game series", the largest line it has; with the definition's name under it, that is the left third of every tournament in all three lists, and everything else (record, squares, dates, durations, conditions, notes, result video) is on the right two thirds. |
| `beta/tournaments_page.py` | That page's record. `publish.sh` runs it and writes the result into the page's `tournaments-data` slot. It reads the definitions in `data/tournaments.json` (head's, in both lanes) and counts the games off `results.jsonl` on the `ledger` branch with `home_rankings.tournament_series`, the same count the live mirror's Mode line uses. **Scheduling a tournament is adding its definition** (`name`, optional `idea`, `conditions`, `start`, `series_size`, `match`): a definition none of whose games has finished is listed as scheduled, in file order. **A result video is a line in the file's `videos`**, the tournament's number against its YouTube link: the page links it with that tournament's games (only a YouTube watch link is carried). **A tournament's short description is a line in the file's `descriptions`**, by number in the same way (or a `description` on its definition, used when the list has no line for its number): every tournament has one, so add it when a tournament is scheduled or begins. A page whose slot is still `null` says it has no record. |
| `beta/home_rankings.py` | The site's two rankings, on Win Analytics since 2026-11-22 (`beta/research.html`; the home page had them before, and a lane pinned to that page is still filled), and the home page's fastest game (`videos`: the record that leads `data/performance/fastest_game_videos.json`, written into the home page's `home-videos` slot). The rankings: the winningest civs of the verification games (our AI's win rate as each civ against Civilization VI's own AI, best first, over the Lv 8 games that began on or after 2026-11-16: `CIVS_LEVEL`, `CIVS_SINCE`), on the left, and the Civ AI Agents by published Elo (`data/strategy_elo.json`, handles from `strategies::ROSTER`) on the right. `publish.sh` runs it against the lane's own revision and writes the result into the page's `rankings-data` slot, so nothing is committed as a copy; a page whose slot is still `null` hides the lists. The civs are the one list that is not a property of the revision: they are read from `results.jsonl` on the `ledger` branch, where `tools/civ6_ladder.py publish-run` adds a line as each game finishes, so both lanes show the record as of the build (the publish workflow's checkout fetches every branch; a clone without the branch publishes without that list). `home_rankings_test.py` holds the parse, the count and the slot. |
| `beta/speedup_timeline.py` | The home page's speed timeline, over its videos: five ways one game of Civilization VI gets played (by hand, the real game under CIVVIS computer control, the Deity sim, self-play on one core, self-play on every core of an M5 Max), the time each takes and how many times faster than by hand that is, on a line of multipliers by tenfolds that ends at the goal, 1,000,000x. The page's one number is the fastest stop's multiplier. The readings are `data/performance/speedup_timeline.json`, dated and oldest first, and the stop that plays on every core follows the record of `data/performance/games_per_minute.json`; nothing derived is committed. `home_rankings.py` writes what this works out into the home page's `home-speedup` slot, and a page whose slot is still `null` shows no timeline. A new reading is a line in the ledger: `docs/SPEEDUP_TIMELINE.md` says how each stop is measured. Tests: `beta/speedup_timeline_test.py`. |
| `beta/download.html` | `civvis.ai/download` (and `/test/download`). Links `releases/latest/download/<asset>`, so it never needs republishing when a release is cut. |
| `.github/workflows/release.yml` | Builds those assets for Windows, macOS (both architectures) and Linux on a `v*` tag. |
| `.github/workflows/to-test-auto-30.yml` | Builds both lanes, checks them, and deploys — half-hourly behind the gate, and on demand. Without the Cloudflare secrets it degrades to a dry run, so the schedule is safe before the account exists. |
| `.github/workflows/to-prod-manual-only.yml` | Moves the `site-stable` tag to a chosen revision, then runs to-test-auto-30 — the only way `/` changes. |
| `beta/publish.sh` | Assembles one complete lane into `beta/dist/` from a named revision — viewer at the root, `/home` and `/download` beside it, every inter-page link lane-relative. |
| `beta/verify.py` | Opens the assembled bundle in a real browser, watches it play, and walks through the password door. |
| `beta/worker_test.py` | Calls `_worker.js` directly — the forward, the password, the headers — needing only Chrome. |
| `beta/debugger_test.py` | Drives the Sim debugger's bar and its split screen (`web/assets/app_debugger.js`, as it ships) against a toy engine: each control sends the operation it is for, and the two halves of a head to head are told, run, compared, copied and kept. Needs only Chrome; run on every pull request with the other `*_test.py` scripts here. |
| `beta/forks_test.py` | Forks an engine to parallel workers (`shim.js` and `worker.js`, as they ship) against a toy engine: a fork carries on from where its engine was, each plays its own history, and a request that names a fork reaches it. |
| `beta/serve.sh` | Serves `beta/dist/` locally the way a static host would. |

Nothing under `web/` is edited to suit the web build, and none of `src/wasm.rs`
compiles for a native target. The whole footprint on the desktop program is two
things in `src/server.rs`: the module declaration, and `process_identity()`.

That second one is not cosmetic. **`std::process::id()` panics outright on
`wasm32-unknown-unknown`**, and `Session::state()` called it — so every single
`/state` trapped while `/runtime` answered fine, and the page died at boot
reporting a `TypeError` reading `state.map`, having assigned it the `{error: …}`
it got back. It is the sort of failure that is invisible in a compile and
one line long in a panic message, which is why the module can now report its
own panics (`civvis_last_panic`) instead of dying as `RuntimeError: unreachable`
and a list of function indices.

### What a visitor lands on

An AI-only simulation on the stock opening world: four free-for-all majors on
the Tiny Tennis Ball planet, its heat scattered rather than banded by latitude,
with an Ancient start, Online game speed, and every victory condition enabled.
It is the world the desktop launchers open too, and the one that asks nothing
of somebody who has just arrived. `stock_opening_params` in `src/server.rs`
describes it. The site and every server open at Max Speed "Max" (unpaced; a
server opened at 2 turns/s until 2026-11-15); a cap the visitor picked by hand
(5, 2, 1, 0.5, 0.25 or 0.1 turns/s) is remembered and wins, and a Tactics
battle still opens at 2 turns/s. The lobby is right there for anyone who
wants to play a seat instead.

The lower-left build marker names the pinned revision and its commit time, then
ages the exact artifact in the browser. Source and packaging time stay visibly
distinct: rebuilding the same commit tomorrow produces a fresh build without
pretending Git history changed.

The world is **different every visit**. The engine is deterministic per seed and
imports nothing, so it cannot vary on its own — the page rolls a seed per load
and hands it over with the first request. `civvis.ai/test/?game=<n>` pins one,
which is how a world worth showing gets shared.

Settings travel the same way. A URL can make the lobby's choices before the
page loads — `?players=12&map=pangaea&victories=domination` opens on that
world instead of the stock exhibition, and every world after it stays on
those settings. `shim.js` reads the parameters and posts the one `/new`
request the setup screen would have; the vocabulary is the lobby's own ids:
`players`, `map`, `shape`, `poles`, `speed`, `era`, `turns`,
`victories` (a comma-separated list of tracks), `arena` (a battlefield's
dimensions, `20x20`), and `mode` (`play` seats the visitor in the first
chair for a single-player game; `watch` leaves the world to its AIs, which
is also the stock setting). A value the engine does not recognise leaves the
stock setting standing, so a mistyped link still opens on a world. The home
page's four game quadrants and its battle picker are exactly these links:
`/?mode=play` is Play Civ, `/` is Watch Civ, and a picked battle travels as
`/?map=<battle>&players=2` with `&mode=play` when it is to be played.

### One property worth knowing

On a socket, a spectated turn is played by a stepper thread and *delivered* to
viewers, and a great deal of machinery exists to stop a turn being simulated
before every viewer has painted the last one.

In the browser there is no stepper. A request that names the frame the page
holds **is** the step, and the viewer only issues that request after `render()`
has finished.

Left at that, a watched turn costs the engine's time plus the page's, each idle
while the other works, and in the opening turns the two are about the same
size. So at Lightning `shim.js` asks for the next frame itself, the moment it
hands the page the one before: the page's own next request, made early, so the
engine plays while the page paints. It is one frame and never two — a second is
not asked for until the page has taken the first — so the engine is never more
than one frame past the glass, and frames reach the page one at a time, in
order. The frame-per-turn contract still holds by construction rather than by
enforcement. The read-ahead is only a guess at a request: anything that changes
the world or what a state says about it (a pause, a pace, a seat to watch as, a
new game) drops it, and the engine answers what the page actually asks.
`shim_test.py` holds it to all of that against a scripted engine, and `?ahead=0`
turns it off for a side-by-side measurement.

The same file sends the viewer's opening pace and between-game hold ahead of
the page's first read, so the first state already agrees with the viewer and
nothing is posted, parsed and drawn again on the first frame; `shim_test.py`
pins those values to the viewer's own.

## Publishing

Nobody cuts test builds. `to-test-auto-30` fires **every half hour** (and on the
Actions button), and a gate decides in seconds whether a deploy is worth
twenty minutes of building: if `/test` already serves the current head it
skips, and scheduled runs pace themselves against **320 of Cloudflare's 500
deployments a month**, earned continuously rather than refilled at midnight.
A run past that pace still deploys when a change under `web/` or `beta/` has
landed since `/test` was built, up to a second line of **470 a month plus 20**,
also earned continuously. Promotions and manual runs are never gated. When it
does run, it rebuilds both lanes — `/test` from the head of `main`, `/` from
the `site-stable` tag — checks that each plays, and deploys them as one site.

The effect: a change to the site is on `/test` within about half an hour of
merging plus the build; a commit that touches only the engine waits for the
pace, and in a week of round-the-clock merging that spacing stretches toward
~2¼ hours instead of exhausting the month. The arithmetic that
forces a governor at all: this repository has merged several hundred commits a
day, and an ungoverned half-hour cadence is ~1,440 deployments a month against
a cap of 500.

**Promoting** is the human act. When `main` is in a state worth being the
front page: **Actions → to-prod-manual-only → Run workflow**, ref = the sha you have
been watching on `/test` (or `main`). It moves the tag and runs the same
publish job the schedule runs — one code path, no drift. The gates prove a
build is not broken; whether a whole game is worth watching is the judgement
being exercised here, usually after simply watching `/test` for a while.

By hand, the equivalent of one scheduled run:

```bash
./beta/publish.sh --commit <stable-sha> --out lane-stable
./beta/publish.sh --commit <head-sha>   --out lane-test
cp -R lane-stable site                    # each lane is a whole, relative-
cp -R lane-test site/test                 # pathed site; it mounts anywhere
rm -f site/test/_worker.js site/test/404.html   # the shell stays at the root
./beta/worker_test.py                     # prove the routing behaves
./beta/verify.py --dist site --mount root
./beta/verify.py --dist site --mount test --no-gate
npx wrangler pages deploy site --project-name civvis
```

`worker_test.py` exists because `_worker.js` is the only part of the site a
static server never runs, so opening the bundle in a browser proves nothing
about it — and it is the part holding both the password and the domain's
whole purpose. `verify.py`'s `check_gate` covers the same ground against the
real runtime, but it needs `npx wrangler` and therefore Node; a machine
without Node skipped the check entirely and said so in one line nobody reads.
`worker_test.py` needs only Chrome: it imports the module and calls it, with
`env.ASSETS` stubbed to report which file *would* have been served. The one
place a browser is not the Workers runtime — `Cookie` and `Set-Cookie` are
forbidden header names on the web and get dropped — is handled in the harness,
and documented there.

Measured on this Mac, the published engine answers `/runtime` (which builds the
world) in about 120 ms, a whole `/state` document in about 95 ms, and a turn in
about 126 ms — four or five turns a second in a browser tab.

`publish.sh` refuses to assemble a page whose asset rewrites no longer match
the viewer, so a restructured `web/index.html` fails the build instead of
publishing a page with missing sprites. The check is *"no root-absolute
reference survives"* rather than *"exactly N were rewritten"*: an exact count
failed the build every time somebody drew a new atlas, which is ordinary work
and says nothing about whether the rewrite still works.

## Weight

A hosted build has to stay something a person can be handed over an ordinary
connection, and both halves of it grow every week. `publish.sh` therefore fails
above **25 MiB assembled** (`BUNDLE_BUDGET_BYTES` to move it deliberately), and
prints where the build sits against that.

Measured on `7e681b6`, which is why two things happen at publication:

| | before | after |
| --- | --- | --- |
| engine, `wasm-opt -O3` | 9,972,139 | 8,084,494 |
| atlases, lossless WebP | 12,464,506 | 8,869,674 |
| **bundle** | **24,228,282** | **18,745,813** — 71% of budget |

- **`wasm-opt -O3`** takes a fifth off the module. `-Oz` was measured against
  it and is worth 0.3% more — 8,061,627 bytes — for a module that simulates
  whole games, so `-O3` stays. Over the wire brotli makes both about 1.74 MB,
  so this is mostly about the disk figure and the budget.
- **Lossless WebP** is pixel-identical to the PNG it replaces, and the atlases
  were the heaviest thing in the bundle — heavier than the engine. It happens
  to the *copy*: `web/assets` stays PNG, which is what the desktop build serves
  and what anyone editing the art works on. Needs Pillow; without it the
  atlases publish as PNG and the only cost is the budget.

Both steps are optional and both are the first thing to reach for when the
budget bites. After them the bundle is dominated by art, so the next real lever
is the atlases themselves rather than anything this directory does.

**A revision is promotable when all of these hold** (the schedule enforces
1–5 for the test lane on its own; promotion adds the sixth):

1. It is on `origin/main` and its CI run is green.
2. `cargo test --profile ci` passes at that revision.
3. `./beta/publish.sh --commit <sha>` completes, inside the size budget.
4. `./beta/worker_test.py` reports `the site routes correctly`.
5. `./beta/verify.py` reports `this build plays` — in both lanes.
6. A whole game is worth watching — the checks above prove it runs, not that
   it is good. Watching it play on `/test` for a while is the honest test,
   and is exactly what the test lane is for.

Each lane's `build.json` records its commit and build time, so anything on
civvis.ai can always be traced back to a revision — `/build.json` for the
front page, `/test/build.json` for the head lane.

## One-time setup

### 1. The host

Cloudflare Pages, on the free plan. It is chosen over the alternatives because
it is the only free host that can run the site's routing **on the server** —
`_worker.js` is not shipped to the browser — and because it serves the module
brotli-compressed at roughly a fifth of its size.

No machine needs wrangler or Node. The whole host-side setup is two repository
secrets under **Settings → Secrets and variables → Actions**:

| Secret | Where it comes from |
| --- | --- |
| `CLOUDFLARE_API_TOKEN` | Cloudflare dashboard → My Profile → API Tokens → Create Token → **Create Custom Token**: one permission, `Account → Cloudflare Pages → Edit`, scoped to the account. (The "Edit Cloudflare Workers" template is the commonly suggested shortcut, but Pages deploys need the Pages permission, not the Workers one.) |
| `CLOUDFLARE_ACCOUNT_ID` | The right-hand column of the account's overview page in the dashboard. |

The first deploying run of `to-test-auto-30` creates the `civvis` Pages project
itself if it does not exist, then deploys into it — which gives a working URL
at `civvis.pages.dev` before the domain is attached. Without the secrets the
workflow still builds and checks both lanes and keeps the site as an artifact,
and fails with a clear message if asked to deploy — rather than appearing to
publish and doing nothing.

### 2. The domain

`civvis.ai` is registered at Namecheap, on Namecheap's own nameservers, and
currently shows a parking page.

**The nameservers have to move to Cloudflare.** This is not a preference:
Cloudflare Pages will only attach an *apex* domain that is a zone on the same
account, because putting a CNAME on a zone apex is not legal DNS and the way
round it — CNAME flattening — only exists inside Cloudflare's own resolver. An
`ALIAS` record at Namecheap (which BasicDNS does support) is enough to point
the apex somewhere, but not enough for Pages to issue the certificate and claim
the hostname. A subdomain such as `beta.civvis.ai` *could* be attached over
external DNS with a plain CNAME — that is the fallback if the move is ever
unwanted, at the cost of the URL being the one asked for.

1. In Cloudflare, **Add a site** → `civvis.ai` → Free plan. It scans the
   existing records and gives you two nameservers.
2. Delete the scanned `MX` and SPF `TXT` records — they are Namecheap's mail
   forwarding, nothing is received through them, and they will not work off
   Namecheap's nameservers anyway. See below.
3. At Namecheap: **Domain List → Manage → Nameservers → Custom DNS**, and enter
   those two. (They replace `dns1.registrar-servers.com` /
   `dns2.registrar-servers.com`.)
4. Wait for Cloudflare to report the zone active — usually minutes, and the
   registrar's own propagation can take a few hours.
5. In the Pages project → **Custom domains** → add `civvis.ai` and
   `www.civvis.ai`. The records and the certificate are created for you.

Nothing else needs a DNS record. The apex and `www` are the whole site;
`/test` and `/download` are paths on it, not hostnames, which is why there is
no `beta.civvis.ai` to set up.

### The mail on this domain

`civvis.ai` currently publishes Namecheap's free email-forwarding records:

```
MX   10 eforward1.registrar-servers.com.  (and eforward2-5)
TXT  "v=spf1 include:spf.efwd.registrar-servers.com ~all"
```

Namecheap ties that service to *their* nameservers, so moving the zone stops it
working even though Cloudflare's scan copies the records across faithfully — the
records will be right and the service behind them will not answer. **Nothing is
received at any `@civvis.ai` address** (checked with Martin, 2026-08-01), so
this costs nothing and the records can simply be dropped.

If that ever changes, the replacement is **Cloudflare Email Routing** — free,
in the same dashboard under Email, and configured in the place the DNS now
lives.

### 3. The test lane is open

`/test` asks for nothing. It was behind a shared password while it was a thing
not meant to be found; it is now the thing the channel points people at.

Setting **`TEST_PASSWORD`** in the Pages project's environment variables
(Production *and* Preview) closes it again with no deploy. There is
deliberately **no fallback password in the code**: the old one was a literal in
this public repository, which is not a password but a speed bump with a
published height. Either the environment names a secret or there is no gate.

Everything under `/test` is still sent `X-Robots-Tag: noindex`. Open to anyone
following a link is not the same as wanting an unfinished build to be the first
search result for the project's name.

The routing is a `_worker.js` rather than the more obvious Pages `functions/`
directory, and that is not a style choice. `functions/` is resolved against the
**working directory wrangler runs in**, not the directory being deployed: run
the deploy from one level up and the whole file is quietly left behind, the
upload succeeds, and the site looks like it works. That happened here once. A
`_worker.js` lives *inside* the deployed directory and cannot be separated from
it — and both checks now ask the routing rather than trusting it, including
that `TEST_PASSWORD` has not been left set by accident.

### 4. The channel

`https://www.youtube.com/@civvis`, in three places: the forward on `/`
(`CHANNEL` in `beta/_worker.js`), the landing page's first button, and the
viewer's own header link in `web/index.html`.

### 5. The downloads

`beta/download.html` links `releases/latest/download/<asset>`, which GitHub
resolves to the newest release, so the page is written once and never
republished for a release. **The asset names are therefore load-bearing and
must not acquire version numbers.** `release.yml` builds exactly those four
names; `verify.py` fails a build whose download page links an asset no release
job produces, because the alternative is finding out from a visitor's 404.

Cutting one is a tag:

```bash
git tag v0.6.1 && git push origin v0.6.1
```

The workflow's "Run workflow" button builds all four targets without
publishing anything, which is how you learn a target stopped compiling before
a tag is public.

## Limits worth knowing

- **Saved games live in IndexedDB**, as the bytes the engine produced, with what the list shows stored apart so listing never loads a game. They used to live in `localStorage`, whose few megabytes a Tiny map outgrew by about turn 130, and a save left there by an older build moves across on first use. Clearing site data still deletes them, and they stay in the browser that made them.
- **The league roster is in the bundle**, compiled in like every other file
  under `data/`, so seats carry the same ratings they do on the desktop build.
  It was read from disk until then, and there is no disk here, so every seat
  showed the provisional 1500 that means "never heard of this player". The
  public static `/test/` site remains read-only. The installed desktop `/wasm/`
  channel has a trusted local host instead: it shares the native spectator's
  live roster, records every seated AI strategy through the native Glicko
  writer, deduplicates retries in `league.json`, and supplies the updated table
  to the module before the next game.
- **Every poll carries the whole world.** The socket build sends a tile *patch*
  — about 157 KB against 1.36 MB — because it keeps a per-viewer fingerprint of
  the map. Here the page is told the world whole every turn, which is the
  full-resync path it already has. It costs serialisation, not correctness, and
  is the first thing to do if the turn rate ever needs to be higher.
- **One tab is one world.** There is no supervisor, no handoff, and no shared
  state between visitors.
- **The module is single-threaded.** The engine's parallel paths are not used;
  a turn on a very large map costs more here than on the desktop build.
