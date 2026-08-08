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
civvis.ai            the STABLE lane — the promoted build, moved by a person
civvis.ai/test       the HEAD lane — latest main, republished automatically
civvis.ai/home       the landing page with build and project links
civvis.ai/rust       the latest published native Rust release
civvis.ai/wasm       the latest published WASM build
civvis.ai/download   the native binaries, from the latest GitHub release
```

**The site is the same viewer twice, from two commits.** `/test` follows
`main` on a schedule so the newest engine is always playable without anyone
deciding anything; `/` is whatever the `site-stable` tag names and moves only
when somebody runs the **to-prod-manual-only** workflow, because whether a build is
worth being the front page is a judgement no gate can make. Each lane states
its commit in its own `build.json`. Both lanes ship in every deployment —
Pages deployments are immutable snapshots of a whole directory, so there is no
such thing as updating half a site; a promotion is nothing but moving the tag
and deploying again.

Repeat visitors cannot be left on an earlier deployment. Each lane's HTML is a
moving pointer and revalidates on every visit. The publisher gives `shim.js`,
`worker.js`, `civvis.wasm`, and every referenced atlas a query version derived
from that file's bytes. A changed file therefore has a new URL and is fetched
after the fresh page arrives; an unchanged file keeps its URL and can be reused
from cache. Legacy unversioned URLs also revalidate. An already-open tab keeps
running its loaded build until it is reloaded.

`/rust` and `/wasm` are the stable build channels. They use uncached temporary
redirects to `/download/` and `/test/`, respectively, so those short addresses
keep following each newly published artifact without trapping a past visitor
on one release. A query on `/wasm`, including `?game=<n>`, survives the redirect.

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
| `beta/shim.js` | Intercepts `fetch` before it reaches the network. Also owns the three things that genuinely became the page's job: the turn clock, the selected between-game finale countdown, and saved games in `localStorage`. |
| `beta/_worker.js` | The whole site's routing: the two lanes, the `/rust` and `/wasm` pointers, the optional gates on `/` and `/test`, and the response headers. |
| `beta/landing.html` | `civvis.ai/home` — the landing page: project links and a gallery of preset simulations, each card a settings-carrying link into the simulator. |
| `beta/download.html` | `civvis.ai/download`. Links `releases/latest/download/<asset>`, so it never needs republishing when a release is cut. |
| `.github/workflows/release.yml` | Builds those assets for Windows, macOS (both architectures) and Linux on a `v*` tag. |
| `.github/workflows/to-test-auto-30.yml` | Builds both lanes, checks them, and deploys — half-hourly behind the gate, and on demand. Without the Cloudflare secrets it degrades to a dry run, so the schedule is safe before the account exists. |
| `.github/workflows/to-prod-manual-only.yml` | Moves the `site-stable` tag to a chosen revision, then runs to-test-auto-30 — the only way `/` changes. |
| `beta/publish.sh` | Assembles `beta/dist/` from a named revision. |
| `beta/verify.py` | Opens the assembled bundle in a real browser, watches it play, and walks through the password door. |
| `beta/worker_test.py` | Calls `_worker.js` directly — the forward, the password, the headers — needing only Chrome. |
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

An AI-only simulation on the stock Small map: six free-for-all majors and nine
city-states on a 74×46 flat Continents world, with a hot equator and cold poles,
an Ancient start, Online game speed, Blitz watch pace, and every victory
condition enabled. It is the shape of game the desktop launchers open too, and
the one that asks nothing of somebody who has just arrived. The lobby is right
there for anyone who wants to play a seat instead.

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
`victories` (a comma-separated list of tracks), and `arena` (a battlefield's
dimensions, `20x20`). A value the engine does not recognise leaves the stock
setting standing, so a mistyped link still opens on a world. The home page's
preset cards are exactly these links.

### One property worth knowing

On a socket, a spectated turn is played by a stepper thread and *delivered* to
viewers, and a great deal of machinery exists to stop a turn being simulated
before every viewer has painted the last one.

In the browser there is no stepper. The page's request for the next turn **is**
the step, and the viewer only issues that request after `render()` has
finished. A turn nobody painted therefore cannot be simulated at all: the
frame-per-turn contract holds here by construction rather than by enforcement.
`verify.py` still measures it, because a property you never check is a property
you are guessing about.

## Publishing

Nobody cuts test builds. `to-test-auto-30` fires **every half hour** (and on the
Actions button), and a gate decides in seconds whether a deploy is worth
twenty minutes of building: if `/test` already serves the current head it
skips, and scheduled runs pace themselves against **440 of Cloudflare's 500
deployments a month** — pro rata by day — so the other 60 always remain for
promotions and manual runs, which are never gated. When it does run, it
rebuilds both lanes — `/test` from the head of `main`, `/` from the
`site-stable` tag — checks that each plays, and deploys them as one site.

The effect: on a quiet day `/test` is fresh within about half an hour of a
commit; in a week of round-the-clock merging the governor stretches spacing
toward ~100 minutes instead of exhausting the month. The arithmetic that
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
cp -R lane-test site && mkdir site/home && mv site/index.html site/home/index.html
cp -R lane-stable/test/. site/           # the viewer is relative-pathed; it
                                         # mounts at the root unchanged
./beta/worker_test.py                    # prove the routing behaves
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

- **Saved games live in `localStorage`**, which is a few megabytes per origin.
  A large late-game world can exceed it; the shim drops the autosave to make
  room and reports the failure rather than silently losing a game.
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
