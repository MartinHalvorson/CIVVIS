// The opponent: Stockfish, in a worker of its own, asked for one move at a
// time at the strength the player picked.
//
// The engine is Stockfish 18 as built for the browser by Stockfish.js
// (`vendor/stockfish-18-lite-single.js` and its `.wasm`: the lite network,
// one thread, GPL-3.0: `vendor/stockfish.LICENSE`). It speaks UCI, a line of text
// each way, so this file is the whole of what Wizard Chess knows about it.
//
// Strength is Stockfish's own setting, not one made here: `UCI_LimitStrength`
// with `UCI_Elo`. Its source says what the number is (`src/search.h`, 17.1):
// "based on a fit of the Elo results for games played between Stockfish at
// various skill levels and various versions of the Stash engine. Skill 0 ..
// 19 now covers CCRL Blitz Elo from 1320 to 3190, approximately". So an Elo
// here is a computer rating list's, approximately, and its range is the
// engine's, read from what it says of itself when it starts (`range`). At
// the top of the range the limit is taken off and it plays as well as it can.

export class Opponent {
  constructor(script) {
    this.range = {min: 1320, max: 3190};
    this.name = "Stockfish";
    this.waiting = null;
    this.failed = null;
    // Searches told to stop, whose last word ("bestmove") is still to come
    // and is nobody's answer.
    this.stale = 0;
    this.ready = new Promise((ready, failed) => {
      let worker;
      try {
        worker = new Worker(script);
      } catch (error) {
        this.failed = error;
        failed(error);
        return;
      }
      this.worker = worker;
      worker.onerror = event => {
        this.failed = new Error(event.message || "the engine could not be started");
        failed(this.failed);
        if (this.waiting) this.waiting.failed(this.failed);
      };
      worker.onmessage = event => {
        for (const line of String(event.data).split("\n")) this.hear(line.trim(), ready);
      };
      worker.postMessage("uci");
    });
    // Nothing waits on a failure it was not asked about.
    this.ready.catch(() => {});
  }

  hear(line, ready) {
    if (line.startsWith("id name ")) this.name = line.slice(8).replace(/ WASM$/, "");
    const elo = /^option name UCI_Elo type spin default \d+ min (\d+) max (\d+)/.exec(line);
    if (elo) this.range = {min: +elo[1], max: +elo[2]};
    if (line === "uciok") {
      this.worker.postMessage("setoption name Hash value 32");
      this.worker.postMessage("isready");
    }
    if (line === "readyok" && ready) { this.awake = true; ready(this); }
    const best = /^bestmove (\S+)/.exec(line);
    if (best && this.stale > 0) { this.stale -= 1; return; }
    if (best && this.waiting) {
      const waiting = this.waiting;
      this.waiting = null;
      waiting.found(best[1] === "(none)" ? null : best[1]);
    }
  }

  // A game that is not the one before it.
  newGame() {
    if (this.awake) this.worker.postMessage("ucinewgame");
  }

  // The move it plays from a position (a FEN), at an Elo, given so long to
  // think: a move as UCI writes it ("e7e5"), or null when it was told to
  // stop or has no move.
  async move(fen, elo, milliseconds = 900) {
    await this.ready;
    this.stop();
    const limited = elo < this.range.max;
    this.worker.postMessage(`setoption name UCI_LimitStrength value ${limited}`);
    if (limited) this.worker.postMessage(`setoption name UCI_Elo value ${Math.max(this.range.min, Math.round(elo))}`);
    this.worker.postMessage(`position fen ${fen}`);
    return new Promise((found, failed) => {
      this.waiting = {found, failed};
      this.worker.postMessage(`go movetime ${Math.max(50, Math.round(milliseconds))}`);
    });
  }

  // Whatever it is thinking about is let go: the move it was asked for is
  // answered with null.
  stop() {
    if (!this.waiting) return;
    const waiting = this.waiting;
    this.waiting = null;
    this.stale += 1;
    this.worker.postMessage("stop");
    waiting.found(null);
  }
}
