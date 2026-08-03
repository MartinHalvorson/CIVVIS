# Local Rust and WASM desktop apps

`Rust CIVVIS.app` and `WASM CIVVIS.app` are two local presentations of one
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

After installation, opening either desktop icon is immediate: it opens or
focuses its installed channel. The same click also starts a locked background
refresh. If GitHub `main` is newer, or either artifact is more than 30 minutes
old, the refresh builds one exact revision, transactionally replaces both apps,
and relaunches both channels. Clicking both icons still creates at most one
build, and a minimized Chrome window is restored before its tab is focused.

Build without installing:

```bash
python3 tools/civvis_desktop_apps.py build --ref <commit>
```

Run the same cheap freshness check used by the launchers:

```bash
python3 tools/civvis_desktop_apps.py refresh
```

Audit installed, live apps against current main and require artifact ages no
greater than 30 minutes:

```bash
python3 tools/civvis_desktop_apps.py verify
```

Build records and staged apps remain under
`~/.local/share/civvis-desktop/build-<short>-<UTC>/`. Replaced bundles move to
`~/.local/share/civvis-desktop/previous/`; they are not deleted.

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
- Both launchers require the listener's commit and `built_at` values to match
  their own before reusing it. An older matching CIVVIS process is stopped;
  an unrelated owner of either port is left alone and causes a visible error.
- The viewer shows compact commit and build ages in the lower-right provenance
  marker beneath the World minimap. Exact timestamps remain in its tooltip.

The public `/rust` and `/wasm` edge routes remain governed by
`beta/_worker.js`; the desktop WASM app intentionally uses `beta/serve.py`
instead of emulating Cloudflare Pages.
