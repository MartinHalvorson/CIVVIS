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
| `beta/_middleware.js` | The password on `/beta`, checked at the edge. |
| `beta/landing.html` | `civvis.ai` itself. |
| `beta/publish.sh` | Assembles `beta/dist/` from a named revision. |
| `beta/verify.py` | Opens the assembled bundle in a real browser and watches it play. |
| `beta/serve.sh` | Serves `beta/dist/` locally the way a static host would. |

Nothing under `src/` or `web/` is edited to suit the web build. `src/server.rs`
gains eight lines declaring the module; that is the entire footprint on the
desktop program, and none of it compiles for a native target.

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
./beta/verify.py                   # prove it plays, in real Chrome
./beta/serve.sh                    # optional: look at it yourself
npx wrangler pages deploy beta/dist --project-name civvis
```

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

`civvis.ai` is registered at Namecheap and currently parked. An apex domain
cannot be a CNAME, so Cloudflare has to run the zone:

1. In Cloudflare, **Add a site** → `civvis.ai` → Free plan. It reads the
   existing records and gives you two nameservers.
2. At Namecheap: **Domain List → Manage → Nameservers → Custom DNS**, and enter
   those two. (They replace `dns1.registrar-servers.com` /
   `dns2.registrar-servers.com`.)
3. Wait for Cloudflare to report the zone active — usually minutes.
4. In the Pages project → **Custom domains** → add `civvis.ai` and `www.civvis.ai`.
   The DNS records and the certificate are created for you.

### 3. The password

It is `2008`, and it lives in `beta/_middleware.js` as the fallback. To change
it without a deploy, set **`BETA_PASSWORD`** in the Pages project's environment
variables (Production *and* Preview).

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
- **The league roster is not in the bundle.** `data/league` is read from disk,
  which does not exist here, so seats are labelled without elo. Nothing else
  depends on it.
- **One tab is one world.** There is no supervisor, no handoff, and no shared
  state between visitors.
- **The module is single-threaded.** The engine's parallel paths are not used;
  a turn on a very large map costs more here than on the desktop build.
