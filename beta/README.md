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
civvis.ai            forwards to youtube.com/@civvis
civvis.ai/home       the landing page, linking the three below
civvis.ai/beta       the published build, open to anyone
civvis.ai/download   the native binaries, from the latest GitHub release
```

The domain's job is to be the channel's address — somebody typing `civvis.ai`
wants the videos. So `/` is a **302** to the channel and the landing page moved
to `/home`. A 302 rather than a 301 because browsers cache a permanent redirect
effectively for ever, and the day this becomes a real front page every past
visitor would still be sent to YouTube. Setting `ROOT_REDIRECT` in the Pages
environment changes the destination, or `off` serves the landing page at `/`
again — neither needs a deploy.

## How it fits together

| Piece | What it does |
| --- | --- |
| `src/wasm.rs` | The engine's request router for the browser. A child module of `server`, `cfg`-gated to wasm, answering the same endpoints over the same JSON. |
| `beta/worker.js` | Runs the module off the main thread. A turn is not a quick call, and the viewer paints on `requestAnimationFrame`; on the page's own thread the engine would stall the frames it exists to produce. |
| `beta/shim.js` | Intercepts `fetch` before it reaches the network. Also owns the three things that genuinely became the page's job: the turn clock, the ten-second finale countdown, and saved games in `localStorage`. |
| `beta/_worker.js` | The password on `/beta`, the forward on `/`, plus the response headers for the whole site. |
| `beta/landing.html` | `civvis.ai/home`. |
| `beta/download.html` | `civvis.ai/download`. Links `releases/latest/download/<asset>`, so it never needs republishing when a release is cut. |
| `.github/workflows/release.yml` | Builds those assets for Windows, macOS (both architectures) and Linux on a `v*` tag. |
| `.github/workflows/publish-site.yml` | Builds, checks and deploys the whole thing from CI, so publishing needs a decision rather than a particular laptop. |
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

Six majors on a Continents map, spectated: the shape of game the channel shows,
and the one that asks nothing of somebody who has just arrived. The lobby is
right there for anyone who wants to play a seat instead.

The world is **different every visit**. The engine is deterministic per seed and
imports nothing, so it cannot vary on its own — the page rolls a seed per load
and hands it over with the first request. `civvis.ai/beta/?game=<n>` pins one,
which is how a world worth showing gets shared.

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

## Cutting a build

Every few days, when `main` is in a state worth showing: **Actions →
publish-site → Run workflow**, on the revision you want. It installs the
toolchain, assembles the bundle, runs both checks and deploys, and it uploads
the bundle as an artifact whether or not the deploy runs. Unticking *deploy*
makes it a dry run.

Publishing stays manual on purpose. The gates prove a build is not broken; they
cannot tell whether a whole game is worth watching, and that is the actual
question.

By hand, which is the same sequence:

```bash
./beta/publish.sh --commit <sha>   # build the bundle from a pinned revision
./beta/worker_test.py              # prove the forward and the door behave
./beta/verify.py                   # prove it plays, and walk through the door
./beta/serve.sh                    # optional: look at it yourself
npx wrangler pages deploy beta/dist --project-name civvis
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

**A revision is publishable when all of these hold:**

1. It is on `origin/main` and its CI run is green.
2. `cargo test --profile ci` passes at that revision.
3. `./beta/publish.sh --commit <sha>` completes, inside the size budget.
4. `./beta/worker_test.py` reports `the site routes correctly`.
5. `./beta/verify.py` reports `this build plays`.
6. A whole game is worth watching — the checks above prove it runs, not that it
   is good. That judgement is the point of publishing every few days rather
   than every commit.

`beta/dist/beta/build.json` records the commit and build time, so anything on
civvis.ai can always be traced back to a revision.

## One-time setup

### 1. The host

Cloudflare Pages, on the free plan. It is chosen over the alternatives because
it is the only free host that can check the password **on the server** — the
gate is not shipped to the browser — and because it serves the 6 MB module
brotli-compressed at about 1.4 MB.

```bash
npm install -g wrangler      # or use npx
wrangler login               # opens a browser once
wrangler pages project create civvis --production-branch main
wrangler pages deploy beta/dist --project-name civvis
```

That already gives a working URL at `civvis.pages.dev`.

Then, so nobody has to do that again, two repository secrets under **Settings →
Secrets and variables → Actions**:

| Secret | Where it comes from |
| --- | --- |
| `CLOUDFLARE_API_TOKEN` | My Profile → API Tokens → Create Token → **Edit Cloudflare Workers** template, scoped to this account. Pages deploys use the Workers permission. |
| `CLOUDFLARE_ACCOUNT_ID` | The right-hand column of the account's overview page, or `wrangler whoami`. |

`publish-site.yml` runs without them — it builds, checks, and keeps the bundle
as an artifact — and fails with a clear message if asked to deploy while they
are missing, rather than appearing to publish and doing nothing.

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
`/beta` and `/download` are paths on it, not hostnames, which is why there is
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

### 3. The beta is open

`/beta` asks for nothing. It was behind a shared password while it was a thing
not meant to be found; it is now the thing the channel points people at.

Setting **`BETA_PASSWORD`** in the Pages project's environment variables
(Production *and* Preview) closes it again with no deploy. There is
deliberately **no fallback password in the code**: the old one was a literal in
this public repository, which is not a password but a speed bump with a
published height. Either the environment names a secret or there is no gate.

Everything under `/beta` is still sent `X-Robots-Tag: noindex`. Open to anyone
following a link is not the same as wanting an unfinished build to be the first
search result for the project's name.

The routing is a `_worker.js` rather than the more obvious Pages `functions/`
directory, and that is not a style choice. `functions/` is resolved against the
**working directory wrangler runs in**, not the directory being deployed: run
the deploy from one level up and the whole file is quietly left behind, the
upload succeeds, and the site looks like it works. That happened here once. A
`_worker.js` lives *inside* the deployed directory and cannot be separated from
it — and both checks now ask the routing rather than trusting it, including
that `BETA_PASSWORD` has not been left set by accident.

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
  showed the provisional 1500 that means "never heard of this player". What is
  still missing is the other direction: nothing is recorded, so a game played
  here moves no rating.
- **Every poll carries the whole world.** The socket build sends a tile *patch*
  — about 157 KB against 1.36 MB — because it keeps a per-viewer fingerprint of
  the map. Here the page is told the world whole every turn, which is the
  full-resync path it already has. It costs serialisation, not correctness, and
  is the first thing to do if the turn rate ever needs to be higher.
- **One tab is one world.** There is no supervisor, no handoff, and no shared
  state between visitors.
- **The module is single-threaded.** The engine's parallel paths are not used;
  a turn on a very large map costs more here than on the desktop build.
