# What Wizard Chess did not write

Two things in this directory are other people's work, shipped as they were
published and loaded by the page as they are. Nothing in them is changed.

| File | What | Version | License | From |
|------|------|---------|---------|------|
| `three.module.min.js` | three.js, the 3D library the board is drawn with | r160 (`three@0.160.1` on npm, `build/three.module.min.js`) | MIT (`three.LICENSE`) | https://github.com/mrdoob/three.js |
| `stockfish-18-lite-single.js`, `stockfish-18-lite-single.wasm` | Stockfish 18 built for the browser: the lite network, one thread | `stockfish@18.0.8` on npm, `bin/` | GPL-3.0 (`stockfish.LICENSE`) | https://github.com/nmrugg/stockfish.js, built from https://github.com/official-stockfish/Stockfish |

The engine's source is at those two addresses, at the versions named. It
runs in a worker of its own and is spoken to in UCI, a line of text each way
(`../engine.js`).

`beta/wizard_chess_test.py` holds each file to the SHA-256 it had when it was
put here, so a copy that has been altered, or swapped for another build,
fails a check before it is published.
