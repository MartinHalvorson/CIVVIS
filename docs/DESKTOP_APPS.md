# Local Rust and WASM desktop apps

`CIVVIS Rust.app` and `CIVVIS Wasm.app` are two local presentations of one
game. The Rust app runs the optimized native server on port 8785. The WASM app
serves the optimized browser build on port 8790. Both open a stable channel
URL, reuse an existing Chrome tab, and carry exact commit and artifact-build
timestamps.

The bundles are generated artifacts. Their source of truth is:

- `tools/civvis_desktop_apps.py` for pinning, building, signing, archival,
  installation, launch, and verification;
- `tools/desktop/CIVVIS Launcher.zsh.in` for the shared launcher behavior;
- `src/wasm.rs` and the native CLI arguments in the launcher for the shared
  opening exhibition;
- `beta/publish.sh` and `beta/serve.py` for the local WASM site.

## Install the current main build

Run this from a CIVVIS checkout on macOS:

```bash
python3 tools/civvis_desktop_apps.py install
```

The command fetches `origin/main`, resolves one exact commit, builds native and
WASM release artifacts in a detached worktree, creates both app bundles from
scratch, ad-hoc signs and strictly verifies them, archives the current apps,
installs the replacements, launches both, and verifies their live metadata and
routes. A process lock prevents two cooperating installers from racing over the
Desktop bundles.

Installation also registers `ai.civvis.desktop-refresh` as a per-user launchd
agent. It checks GitHub every ten minutes and rebuilds the pinned pair when
`main` advances or either artifact reaches ten minutes old. That headroom keeps
a successful build inside the 20-minute freshness contract even while Cargo is
working. The job installs both bundles transactionally without interrupting a
live game. Opening either icon performs the same locked freshness check in the
background, so a laptop that just woke converges without waiting for the next
scheduled check.

The native channel runs under the repository spectator supervisor. A promoted
runtime waits for the next game boundary, then the next default simulation
starts on it; checkpoints also preserve an active map across crash recovery.
The browser module checks the installed `build.json` at its result boundary and
reloads before dealing the next world when a newer WASM bundle is present.

Each app owns a small Chrome-tab watcher. Closing its matching tab (or Chrome)
stops that channel's supervisor/server within about ten seconds. Double-clicking
the icon starts it again. A minimized Chrome window is restored before its tab
is focused.

Build without installing:

```bash
python3 tools/civvis_desktop_apps.py build --ref <commit>
```

Run the same cheap freshness check used by the launchers:

```bash
python3 tools/civvis_desktop_apps.py refresh
```

Audit installed, live apps against current main, require artifact ages no
greater than 20 minutes, and verify the recurring launchd agent:

```bash
python3 tools/civvis_desktop_apps.py verify
```

Build records and staged apps remain under
`~/.local/share/civvis-desktop/build-<short>-<UTC>/`. Replaced bundles move to
`~/.local/share/civvis-desktop/previous/`. Because the updater is perpetual,
it retains the two newest build trees and four newest archived bundles rather
than allowing generated artifacts to consume the disk without bound.

## Shared opening exhibition

Both apps open directly into AI simulation with six major civilizations on a
Small 74x46 flat Continents map, nine city-states, no teams, an Ancient start,
Online game speed, Blitz 1000 ms watch speed, normal hot-equator/cold-poles
latitude, and every victory condition enabled. The builder fails closed if the
native launcher or WASM opening parameters drift from that contract.

## Channel and provenance contracts

- Native content is available at `/rust/` and the legacy root.
- WASM content is available at `/wasm/`, locally mapped onto the packaged
  `/beta/` tree with query strings preserved.
- The WASM launcher requires the listener's commit and `built_at` values to
  match its bundle. The Rust launcher also recognizes its owned supervisor
  while that supervisor is holding a newly promoted runtime for a safe game
  boundary. An unrelated owner of either port is left alone and causes a
  visible error.
- The viewer shows compact commit and build ages in the lower-right provenance
  marker beneath the World minimap. Exact timestamps remain in its tooltip.

The public `/rust` and `/wasm` edge routes remain governed by
`beta/_worker.js`; the desktop WASM app intentionally uses `beta/serve.py`
instead of emulating Cloudflare Pages.
