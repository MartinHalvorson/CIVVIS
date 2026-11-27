# What Wizard Chess did not write

Two things in this directory are other people's work, shipped as they were
published and loaded by the page as they are. Nothing in them is changed.

| File | What | Version | License | From |
|------|------|---------|---------|------|
| `three.module.min.js` | three.js, the 3D library the board is drawn with | r160 (`three@0.160.1` on npm, `build/three.module.min.js`) | MIT (`three.LICENSE`) | https://github.com/mrdoob/three.js |
| `stockfish-18-lite-single.js`, `stockfish-18-lite-single.wasm` | Stockfish 18 built for the browser: the lite network, one thread | `stockfish@18.0.8` on npm, `bin/` | GPL-3.0 (`stockfish.LICENSE`) | https://github.com/nmrugg/stockfish.js, built from https://github.com/official-stockfish/Stockfish |

The engine's source is at those two addresses, at the versions named. It
runs in a worker of its own and is spoken to in UCI, a line of text each way
(`../engine.js`), so it stays a separate program under its own licence and
the rest of CIVVIS stays MIT.

**Where the source of this exact Stockfish build is** (what GPL-3.0 asks for
next to the object code):

- Stockfish.js at tag `v18.0.8`, commit
  `93c994592dcf3b4b21052ab925e9b534df9c0918` (the npm package's `gitHead`):
  https://github.com/nmrugg/stockfish.js/tree/93c994592dcf3b4b21052ab925e9b534df9c0918.
  It holds the engine's C++ source (`src/`) and the build (`build.js`,
  emscripten 3.1.7; these two files are its lite, single-threaded flavour).
- The network compiled into the `.wasm`, `nn-9067e33176e8.nnue` (named in that
  commit's `src/lite_nets.h`):
  https://tests.stockfishchess.org/api/nn/nn-9067e33176e8.nnue.
- The two files here are byte-identical to `bin/` of
  https://registry.npmjs.org/stockfish/-/stockfish-18.0.8.tgz (checked
  2026-11-27).

`../../../THIRD_PARTY_NOTICES.md` lists these with everything else in the
repository that is other people's work.

`beta/wizard_chess_test.py` holds each file to the SHA-256 it had when it was
put here, so a copy that has been altered, or swapped for another build,
fails a check before it is published.
