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
civvis.ai            a landing page that points at the channel
civvis.ai/beta       the published build, behind a password
```

## How it fits together

| Piece | What it does |
| --- | --- |
| `src/wasm.rs` | The engine's request router for the browser. A child module of `server`, `cfg`-gated to wasm, answering the same endpoints over the same JSON. |
| `beta/worker.js` | Runs the module off the main thread. A turn is not a quick call, and the viewer paints on `requestAnimationFrame`; on the page's own thread the engine would stall the frames it exists to produce. |
| `beta/shim.js` | Intercepts `fetch` before it reaches the network. Also owns the three things that genuinely became the page's job: the turn clock, the ten-second finale countdown, and saved games in `localStorage`. |
| `beta/_worker.js` | The password on `/beta`, checked at the edge, plus the response headers for the whole site. |
| `beta/landing.html` | `civvis.ai` itself. |
| `beta/publish.sh` | Assembles `beta/dist/` from a named revision. |
| `beta/verify.py` | Opens the assembled bundle in a real browser, watches it play, and walks through the password door. |
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

Every few days, when `main` is in a state worth showing:

```bash
./beta/publish.sh --commit <sha>   # build the bundle from a pinned revision
./beta/verify.py                   # prove it plays and that the door is shut
./beta/serve.sh                    # optional: look at it yourself
npx wrangler pages deploy beta/dist --project-name civvis
```

Measured on this Mac, the published engine answers `/runtime` (which builds the
world) in about 120 ms, a whole `/state` document in about 95 ms, and a turn in
about 126 ms — four or five turns a second in a browser tab.

`publish.sh` refuses to assemble a page whose asset rewrites no longer match
the viewer, so a restructured `web/index.html` fails the build instead of
publishing a page with missing sprites.

**A revision is publishable when all of these hold:**

1. It is on `origin/main` and its CI run is green.
2. `cargo test --profile ci` passes at that revision.
3. `./beta/publish.sh --commit <sha>` completes.
4. `./beta/verify.py` reports `this build plays`.
5. A whole game is worth watching — the check above proves it runs, not that it
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
2. **Check the scan kept the mail records before continuing** — see below.
3. At Namecheap: **Domain List → Manage → Nameservers → Custom DNS**, and enter
   those two. (They replace `dns1.registrar-servers.com` /
   `dns2.registrar-servers.com`.)
4. Wait for Cloudflare to report the zone active — usually minutes.
5. In the Pages project → **Custom domains** → add `civvis.ai` and
   `www.civvis.ai`. The records and the certificate are created for you.

### The mail on this domain

`civvis.ai` is **not** a bare parked domain. It is currently publishing:

```
MX   10 eforward1.registrar-servers.com.  (and eforward2-5)
TXT  "v=spf1 include:spf.efwd.registrar-servers.com ~all"
```

That is Namecheap's free email forwarding, and Namecheap ties that service to
*their* nameservers. Moving the zone to Cloudflare will very likely stop it
working even though Cloudflare's scan copies the records across faithfully — the
records will be right and the service behind them will not answer.

If anything is actually being received at an `@civvis.ai` address, replace it
with **Cloudflare Email Routing** (free, in the same dashboard, under Email) and
point the address at whatever inbox should have it. It is a better service than
the one being replaced, and it is configured in the place the DNS now lives. Do
this in the same sitting as the nameserver change, not afterwards.

### 3. The password

It is `2008`, and it lives in `beta/_worker.js` as the fallback. To change it
without a deploy, set **`BETA_PASSWORD`** in the Pages project's environment
variables (Production *and* Preview).

The gate is a `_worker.js` rather than the more obvious Pages `functions/`
directory, and that is not a style choice. `functions/` is resolved against the
**working directory wrangler runs in**, not the directory being deployed: run
the deploy from one level up and the gate is quietly left behind, the upload
succeeds, the site works, and the beta is wide open with nothing anywhere to
say so. That happened here once. A `_worker.js` lives *inside* the deployed
directory and cannot be separated from it — and `verify.py` now opens the door
and walks through it rather than trusting that it exists.

The gate is deliberately soft: one shared password, no accounts, a cookie good
for a week. It keeps the build from being stumbled upon, and the pages behind it
are sent `X-Robots-Tag: noindex`. It is not access control, and the repository
is public, so nothing behind it should be anything that would matter if it got
out.

### 4. The channel link

`beta/landing.html` has exactly one YouTube URL, marked with a comment. When
the new account exists, change that line and republish.

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
