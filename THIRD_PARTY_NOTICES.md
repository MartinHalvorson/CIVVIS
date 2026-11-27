# Third-party notices

CIVVIS's own code, data and documentation are under the MIT License
([`LICENSE`](LICENSE)). Some things in this repository, or compiled into the
programs it ships, are other people's work. Each keeps its own licence;
nothing here relicenses it. This file lists them: what each is, which
version, under which licence, where it lives here and where it comes from.

Audited 2026-11-27 over every tracked file: vendored code, minified or
foreign-headed scripts, fonts (there are none: every page uses system fonts),
images, data sets, `mods/`, `web/`, `beta/` and `tools/`, and the crates in
`Cargo.lock`. `tools/test_third_party_notices.py` fails when a crate in
`Cargo.lock` or a file in `beta/chess/vendor/` is missing from this list.
Everything not listed here, including `mods/` and every script under `web/`,
`beta/` and `tools/` outside `beta/chess/vendor/`, is CIVVIS's own.

Earlier revisions keep the terms they were published under: from #899
(2026-08-02) to #5830 (2026-11-25) the repository said GPL-3.0-or-later, and
from #5830 until MIT it stated no licence.

## Code kept in this repository

### Stockfish.js 18 (Wizard Chess's opponent)

| | |
|---|---|
| Files | `beta/chess/vendor/stockfish-18-lite-single.js`, `beta/chess/vendor/stockfish-18-lite-single.wasm` |
| Version | Stockfish.js 18, npm `stockfish@18.0.8`, its `bin/` files, byte-identical (checked 2026-11-27 against the npm tarball) |
| Licence | GPL-3.0. The full text is beside the files: [`beta/chess/vendor/stockfish.LICENSE`](beta/chess/vendor/stockfish.LICENSE) |
| Copyright | Stockfish.js (c) 2026 Chess.com, LLC. Stockfish (c) T. Romstad, M. Costalba, J. Kiiski, G. Linscott and other contributors. Network by Linmiao Xu (linrock) |
| Upstream | https://github.com/nmrugg/stockfish.js, built from https://github.com/official-stockfish/Stockfish |

**Source of this exact build.** GPL-3.0 asks that whoever passes on the
object code says where its Corresponding Source is:

- Stockfish.js at tag `v18.0.8`, commit
  `93c994592dcf3b4b21052ab925e9b534df9c0918`, the commit npm records as the
  package's `gitHead`:
  https://github.com/nmrugg/stockfish.js/tree/93c994592dcf3b4b21052ab925e9b534df9c0918
  (as an archive: https://github.com/nmrugg/stockfish.js/archive/93c994592dcf3b4b21052ab925e9b534df9c0918.tar.gz).
  It holds the engine's C++ source (`src/`) and its build (`build.js`,
  emscripten 3.1.7). These two files are the lite, single-threaded flavour.
- The network compiled into the `.wasm`, `nn-9067e33176e8.nnue`, which that
  commit names in `src/lite_nets.h`:
  https://tests.stockfishchess.org/api/nn/nn-9067e33176e8.nnue

The same directions are next to the files, in
[`beta/chess/vendor/README.md`](beta/chess/vendor/README.md).

**How CIVVIS uses it.** As a separate program. `beta/chess/engine.js` starts
it in a Web Worker of its own and speaks to it only in UCI, a line of text
each way. No CIVVIS code is linked into it or derived from it, so CIVVIS's
code stays MIT.

**Where it is published.** In this repository. Wizard Chess was on civvis.ai
at `/wizard-chess` from 2026-11-23 to 2026-11-24 and is set aside now: no
lane publishes it (`beta/publish.sh`). If it goes back on the site, publish
`stockfish.LICENSE` and these directions with it.

### three.js r160 (Wizard Chess's board)

| | |
|---|---|
| File | `beta/chess/vendor/three.module.min.js` |
| Version | npm `three@0.160.1`, `build/three.module.min.js`, byte-identical (checked 2026-11-27); upstream commit `02d4c5aa091ea8b6137db5bf1ad38819308246e3` |
| Licence | MIT, Copyright © 2010-2023 three.js authors. Text beside it: [`beta/chess/vendor/three.LICENSE`](beta/chess/vendor/three.LICENSE) |
| Upstream | https://github.com/mrdoob/three.js, https://threejs.org/ |

## Code compiled into the shipped programs

The `civvis` programs on the download page and GitHub Releases, and
`civvis.wasm` on civvis.ai, compile in these crates from crates.io. None is
copied into this repository; `Cargo.lock` pins the versions. Where a crate
offers a choice of licence, CIVVIS takes it under MIT.

| Crate | Version | Licence | In | Upstream |
|---|---|---|---|---|
| `serde` | 1.0.229 | MIT OR Apache-2.0 | programs, wasm | https://github.com/serde-rs/serde |
| `serde_core` | 1.0.229 | MIT OR Apache-2.0 | programs, wasm | https://github.com/serde-rs/serde |
| `serde_json` | 1.0.151 | MIT OR Apache-2.0 | programs, wasm | https://github.com/serde-rs/json |
| `itoa` | 1.0.18 | MIT OR Apache-2.0 | programs, wasm | https://github.com/dtolnay/itoa |
| `zmij` | 1.0.23 | MIT | programs, wasm | https://github.com/dtolnay/zmij |
| `memchr` | 2.8.3 | Unlicense OR MIT | programs, wasm | https://github.com/BurntSushi/memchr |
| `libm` | 0.2.16 | MIT | programs, wasm | https://github.com/rust-lang/compiler-builtins |
| `mimalloc` | 0.1.52 | MIT | programs only | https://github.com/purpleprotocol/mimalloc_rust |
| `libmimalloc-sys` | 0.1.49 | MIT; it carries Microsoft's mimalloc C library (v3, `MI_MALLOC_VERSION 30302`), also MIT | programs only | https://github.com/purpleprotocol/mimalloc_rust, https://github.com/microsoft/mimalloc |

Used only while building, not part of what ships:

| Crate | Version | Licence | What for | Upstream |
|---|---|---|---|---|
| `serde_derive` | 1.0.229 | MIT OR Apache-2.0 | writes serde's code at compile time | https://github.com/serde-rs/serde |
| `proc-macro2` | 1.0.107 | MIT OR Apache-2.0 | for `serde_derive` | https://github.com/dtolnay/proc-macro2 |
| `quote` | 1.0.47 | MIT OR Apache-2.0 | for `serde_derive` | https://github.com/dtolnay/quote |
| `syn` | 3.0.2 | MIT OR Apache-2.0 | for `serde_derive` | https://github.com/dtolnay/syn |
| `unicode-ident` | 1.0.24 | (MIT OR Apache-2.0) AND Unicode-3.0 | for `syn` | https://github.com/dtolnay/unicode-ident |
| `cc` | 1.4.5 | MIT OR Apache-2.0 | compiles mimalloc's C code | https://github.com/rust-lang/cc-rs |
| `find-msvc-tools` | 0.1.12 | MIT OR Apache-2.0 | for `cc` on Windows | https://github.com/rust-lang/cc-rs |
| `shlex` | 2.0.1 | MIT OR Apache-2.0 | for `cc` | https://github.com/comex/rust-shlex |

The MIT licence asks that each copyright notice travel with copies. The
notices of the crates that ship:

- `serde`, `serde_core`, `serde_json`: Erick Tryzelaar and David Tolnay.
  `itoa`, `zmij`: David Tolnay.
- `memchr`: Copyright (c) 2015 Andrew Gallant.
- `libm`: Copyright (c) 2018 Jorge Aparicio. Portions from musl libc,
  Copyright © 2005-2020 Rich Felker, et al., whose math code is Copyright ©
  1993,2004 Sun Microsystems or © 2003-2011 David Schultz or © 2003-2009
  Steven G. Kargl or © 2003-2009 Bruce D. Evans or © 2008 Stephen L. Moshier
  or © 2017-2018 Arm Limited; and from the CORE-MATH project (MIT, per file).
- `mimalloc`, `libmimalloc-sys`: Copyright 2019 Octavian Oncescu.
- mimalloc (C): Copyright (c) 2018-2025 Microsoft Corporation, Daan Leijen.

Each under these terms:

> Permission is hereby granted, free of charge, to any person obtaining a
> copy of this software and associated documentation files (the "Software"),
> to deal in the Software without restriction, including without limitation
> the rights to use, copy, modify, merge, publish, distribute, sublicense,
> and/or sell copies of the Software, and to permit persons to whom the
> Software is furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in
> all copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
> FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
> DEALINGS IN THE SOFTWARE.

## Data from public sources

### `data/earth_surface.txt`, Earth's surface on a half-degree grid

Built by `tools/earth_surface.py`. The file holds one derived class per
half-degree cell (sea, lake, terrain, hills, vegetation), not any source's
own data.

| Source | Licence | Upstream |
|---|---|---|
| Natural Earth 1:50m `land` and `lakes` | Public domain: "All versions of Natural Earth raster + vector map data found on this website are in the public domain." | https://www.naturalearthdata.com/, files from https://github.com/nvkelso/natural-earth-vector (`master` when the grid was built) |
| SRTM15+ V2.7 (Tozer et al. 2019, https://doi.org/10.1029/2019EA000658), resampled to 10 arc-minutes by the GMT data server (`earth_relief_10m_p`) | Public domain: the series' README says "All data are derived from public domain sources and these data are also in the public domain." | https://topex.ucsd.edu/pub/srtm15_plus/README_V2.7.txt, https://www.generic-mapping-tools.org/remote-datasets/earth-relief.html |
| Köppen-Geiger climate classification at half a degree (Kottek et al. 2006, Meteorologische Zeitschrift 15, 259-263) | None stated: see Unresolved | http://koeppen-geiger.vu-wien.ac.at/ |

## Icons from game-icons.net

| | |
|---|---|
| Files | `web/assets/unit-glyphs.png` (one silhouette per unit) and the pictographs on `web/assets/yield-icons.png`, rasterised by `tools/own_art.py` from the path data in `tools/own_art_icons.json` |
| Version | https://github.com/game-icons/icons at `82d948812bfe` (2026-04-23), 120 icons, shapes unmodified |
| Licence | CC BY 3.0, https://creativecommons.org/licenses/by/3.0/ |
| Authors | Lorc, Delapouite, Skoll, Sbed, Cathelineau and Caro Asercion; the author of every icon, and what it draws, in [`web/assets/ART_CREDITS.md`](web/assets/ART_CREDITS.md). The viewer links that page from its build marker. |

The plates, yield discs and city-wall mark are CIVVIS's own, drawn by the same
tool; `web/assets/art-manifest.json` lists every image the site may ship, by
hash.

## Sid Meier's Civilization VI

CIVVIS is a fan project, not affiliated with or endorsed by Firaxis Games,
2K or Take-Two Interactive. Sid Meier's Civilization and Civilization VI are
their trademarks. The game, its text, art, music and data are theirs. You
need your own copy of the game to play against it. What of the game this
repository touches:

- **Rules** (`data/*.json`, `src/`): numbers, type names such as
  `UNIT_WARRIOR` and `LEADER_TRAJAN`, and the names of civilizations,
  leaders, units and abilities, read from the game's database and
  Civilopedia so the engine plays by the same rules, with short descriptions
  in CIVVIS's own words. No file of the game is copied into the repository;
  the screenshots of it, below, are the exception.
- **The mod that plays the real game** (`tools/civ6_control/mod/`,
  `tools/civ6_mod/`): CIVVIS's own Lua and XML, MIT like the rest. It calls
  the game's scripting interface; where it follows the game's own UI scripts,
  a comment names the script, and no Firaxis file is included.
- **Colours** (`data/civ6_player_colors.json`, the district colours in
  `web/assets/app.js`): colour values read from the game's colour atlas.
- **Art**: none of the game's ships. Until #5940 four sprite sheets cut from
  the installed game were compiled in and served by the site; the map now
  draws CIVVIS's own (`tools/own_art.py`, below). The cutters listed in
  `docs/CIV6_ART.md` still exist for somebody who owns the game and write only
  into the git-ignored `local/civ6-art/`, which only a local server reads;
  `tools/art_provenance.py` refuses such art in the repository and in every
  published build.
- **Screenshots used as test fixtures** (`tools/fixtures/deal_refuse_live.jpg`,
  `tools/fixtures/ordinary_map_live.jpg`, `tools/fixtures/game_modes_scaled.png`):
  screens of the running game, read by the tools' tests and never published.
  See Unresolved.

## Unresolved

Items whose licence this audit could not establish. They are listed, not
claimed under MIT.

| What | Where | Why |
|---|---|---|
| Civilization VI screenshots | `tools/fixtures/deal_refuse_live.jpg`, `tools/fixtures/ordinary_map_live.jpg`, `tools/fixtures/game_modes_scaled.png` | Pictures of Firaxis's and 2K's game; no licence. Test inputs only, not shipped or served. |
| The hidden-map vignettes | `web/assets/hidden-map-monsters.png` | Where they came from is not recorded. They arrived in #391 (2026-07-26), redrawn in #947, #953 and #2222; none says whether they were drawn, generated or taken from elsewhere. They do not look like Civilization VI's art. They ship inside the programs and on the site. (The two terrain feature atlases of the same kind, unused since #4865, were deleted in #5940.) |
| Köppen-Geiger climate data | input to `data/earth_surface.txt` | The source site states no licence or terms for the download. The repository keeps only a coarse terrain class derived from it, and cites it. |
