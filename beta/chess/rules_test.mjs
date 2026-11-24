// The checks of the rules and of the command reader, run by node:
//   node beta/chess/rules_test.mjs
// `beta/wizard_chess_test.py` runs it with the rest of the page's checks.
import assert from "node:assert/strict";
import {START, fromFen, toFen, perft, legalMoves, play, san, uci, moveFromUci, standing, spoken, positionKey, squareAt} from "./rules.js";
import {readCommand} from "./command.js";

// Perft: the published counts of positions reachable in so many moves
// (chessprogramming.org, "Perft Results"). Castling, en passant, pins and
// promotions each miscount one of these when they are wrong.
for (const [fen, counts] of [
  [START, [20, 400, 8902, 197281]],
  ["r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", [48, 2039, 97862]],
  ["8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", [14, 191, 2812, 43238]],
  ["r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1", [6, 264, 9467]],
  ["rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8", [44, 1486, 62379]],
  ["r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10", [46, 2079, 89890]],
]) {
  assert.deepEqual(counts.map((_, at) => perft(fromFen(fen), at + 1)), counts, fen);
  assert.equal(toFen(fromFen(fen)), fen);
}

// A game played out in the notation it is written in, which the reader
// reads and `san` writes back the same: the Opera Game (Morphy, 1858).
const opera = "e4 e5 Nf3 d6 d4 Bg4 dxe5 Bxf3 Qxf3 dxe5 Bc4 Nf6 Qb3 Qe7 Nc3 c6 Bg5 b5 Nxb5 cxb5 Bxb5+ Nbd7 O-O-O Rd8 Rxd7 Rxd7 Rd1 Qe6 Bxd7+ Nxd7 Qb8+ Nxb8 Rd8#".split(" ");
let position = fromFen(START);
const seen = new Map();
for (const mark of opera) {
  const read = readCommand(position, mark);
  assert.ok(read.move, `${mark}: ${read.error}`);
  assert.equal(san(position, read.move), mark);
  assert.equal(uci(moveFromUci(position, uci(read.move))), uci(read.move));
  position = play(position, read.move);
  seen.set(positionKey(position), (seen.get(positionKey(position)) || 0) + 1);
}
assert.equal(standing(position, seen), "checkmate");
assert.equal(toFen(position), "1n1Rkb1r/p4ppp/4q3/4p1B1/4P3/8/PPP2PPP/2K5 b k - 1 17");

// How the game stands.
assert.equal(standing(fromFen(START)), "play");
assert.equal(standing(fromFen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1")), "stalemate");
assert.equal(standing(fromFen("8/8/8/8/8/5k2/8/5K1B w - - 0 1")), "dead");
assert.equal(standing(fromFen("8/8/8/8/8/5k2/8/5K1R w - - 100 80")), "fifty");
assert.equal(standing(fromFen("rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3")), "checkmate");
assert.equal(standing(fromFen("4k3/8/8/8/8/8/4q3/4K3 w - - 0 1")), "check");

// What is said to the pieces, and what a microphone makes of it.
const read = (fen, text) => {
  const from = fromFen(fen), got = readCommand(from, text);
  return got.move ? san(from, got.move) : got.error;
};
for (const [text, mark] of [
  ["e4", "e4"], ["E4", "e4"], ["pawn to e4", "e4"], ["e2 to e4", "e4"], ["e2e4", "e4"], ["e 2 e 4", "e4"],
  ["knight to f3", "Nf3"], ["Knight to F3.", "Nf3"], ["night 2 f 3", "Nf3"], ["knight foxtrot three", "Nf3"],
  ["horse to see three", "Nc3"], ["nf3", "Nf3"], ["Nf3", "Nf3"], ["g1f3", "Nf3"], ["knight g1 to f3", "Nf3"],
  ["pond to dee for", "d4"], ["move the pawn to d4 please", "d4"], ["delta four", "d4"],
]) assert.equal(read(START, text), mark, text);
// Captures, and which of two pieces.
const middle = "r1bqk2r/pppp1ppp/2n2n2/2b1p3/2B1P3/2NP1N2/PPP2PPP/R1BQK2R w KQkq - 1 5";
for (const [text, mark] of [
  ["castle", "O-O"], ["castle kingside", "O-O"], ["castles king side", "O-O"], ["short castle", "O-O"], ["O-O", "O-O"], ["0-0", "O-O"],
  ["bishop takes f7", "Bxf7+"], ["bishop captures f7", "Bxf7+"], ["Bxf7+", "Bxf7+"], ["bishop g5", "Bg5"],
  ["knight takes e5", "Nxe5"], ["knight to d5", "Nd5"], ["queen e2", "Qe2"], ["king to f1", "Kf1"],
  ["rook to b1", "Rb1"], ["castle to b1", "Rb1"], ["rook a to b1", "Rb1"],
]) assert.equal(read(middle, text), mark, text);
assert.match(read(middle, "knight to e2"), /^Ne2$/);
assert.match(read(middle, "knight to d2"), /^Nd2$/);
// Two knights can reach one square: asked which, then told.
const twins = "4k3/8/8/8/8/8/8/1N2KN2 w - - 0 1";
assert.match(read(twins, "knight to d2"), /^Which knight\? Nbd2, Nfd2$/);
assert.equal(read(twins, "knight b to d2"), "Nbd2");
assert.equal(read(twins, "knight f1 d2"), "Nfd2");
assert.equal(read(twins, "Nfd2"), "Nfd2");
// Castling long, en passant, and a pawn that becomes a piece.
assert.equal(read("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", "castle queenside"), "O-O-O");
assert.equal(read("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", "long castle"), "O-O-O");
assert.equal(read("r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1", "queen side castle"), "O-O-O");
assert.equal(read("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 2", "pawn takes d6"), "exd6");
assert.equal(read("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 2", "e takes d6"), "exd6");
const last = "8/4P3/8/8/8/8/k7/4K3 w - - 0 1";
assert.equal(read(last, "e8"), "e8=Q");
assert.equal(read(last, "pawn to e8"), "e8=Q");
assert.equal(read(last, "e8 knight"), "e8=N");
assert.equal(read(last, "pawn to e8 becomes a rook"), "e8=R");
assert.equal(read(last, "e7e8b"), "e8=B");
assert.equal(read(last, "e8=N"), "e8=N");
assert.equal(read(last, "e8n"), "e8=N");
// What cannot be played is said, with what was heard.
assert.match(read(START, "knight to e5"), /not a move that can be played/);
assert.match(read(START, "abracadabra"), /do not know a move/);
assert.match(read(START, ""), /Nothing was said/);
assert.match(read(START, "knight"), /^Which knight\?/);

// The knight ridden: "ride to f3" is its move, and of two knights that
// could go somewhere the one ridden is the one meant.
const rides = (fen, text, mount) => {
  const from = fromFen(fen), got = readCommand(from, text, {mount});
  return got.move ? san(from, got.move) : got.error;
};
assert.equal(rides(START, "ride to f3", 6), "Nf3");
assert.equal(rides(START, "gallop to h3", 6), "Nh3");
assert.equal(rides(START, "charge c3", 1), "Nc3");
assert.match(rides(START, "ride to c3", 6), /not a move that can be played/);
assert.match(rides(START, "ride to f3", -1), /not riding a knight/);
assert.equal(rides(twins, "knight to d2", 5), "Nfd2");
assert.equal(rides(twins, "knight to d2", 1), "Nbd2");
assert.equal(rides(twins, "Nd2", 1), "Nbd2");
assert.match(rides(twins, "knight to d2", -1), /^Which knight\?/);
assert.equal(rides("4k3/8/8/3p4/8/4N3/8/4K3 w - - 0 1", "gallop takes d5", 20), "Nxd5");

// A move said aloud, for the opponent's to be announced.
const says = (fen, move) => { const from = fromFen(fen); return spoken(from, moveFromUci(from, move)); };
assert.equal(says(START, "g1f3"), "knight to f3");
assert.equal(says(middle, "c4f7"), "bishop takes f7, check");
assert.equal(says(middle, "e1g1"), "castles kingside");
assert.equal(says(last, "e7e8n"), "pawn to e8, and becomes a knight");
assert.equal(squareAt("h8"), 63);
assert.equal(legalMoves(fromFen(START)).length, 20);
console.log("wizard chess rules: perft, a whole game read and written, and the commands, all hold");
