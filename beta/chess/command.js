// A command to the pieces, as it is said or typed, read as a move.
//
// In wizard chess nobody picks a piece up: the player says what is to
// happen and the piece does it. So a move is a command, and this reads one:
// "knight to f3", "pawn takes d5", "e2 to e4", "castle kingside", "queen h5",
// or the way a game is written down, "Nf3", "exd5", "O-O", "e8=Q".
//
// It does not parse a grammar. It collects what a command names (a piece, a
// square to go to, a square or file or rank to come from, a capture, a
// promotion, a castling side) and asks which of the position's legal moves
// fit all of it. One move fitting is the move. None, or several, is said
// back to the player in words, with what was heard.
//
// What a microphone hears is not what was said. A file comes back as a word
// ("see", "bee", "gee"), a rank as a homophone ("for", "ate", "won"), a piece
// as something else entirely ("night", "pond", "rock"). The lists below are
// those, and the phonetic alphabet is read as files too ("knight to foxtrot
// three"), which is what to say when the letters will not come through.

import {legalMoves, san, squareName, PIECE_NAMES} from "./rules.js";

const PIECES = {
  king: "k", kings: "k",
  queen: "q", queens: "q", green: "q",
  rook: "r", rooks: "r", rock: "r", ruck: "r", brook: "r", castle: "r", tower: "r",
  bishop: "b", bishops: "b",
  knight: "n", knights: "n", night: "n", nights: "n", horse: "n",
  pawn: "p", pawns: "p", pond: "p", pan: "p", palm: "p", pon: "p", porn: "p", prawn: "p",
};
const FILE_WORDS = {
  a: "a", ay: "a", eh: "a", hey: "a", alpha: "a", alfa: "a",
  b: "b", be: "b", bee: "b", bravo: "b",
  c: "c", see: "c", sea: "c", si: "c", charlie: "c",
  d: "d", dee: "d", de: "d", delta: "d",
  e: "e", ee: "e", he: "e", echo: "e",
  f: "f", ef: "f", eff: "f", if: "f", foxtrot: "f",
  g: "g", gee: "g", ji: "g", jee: "g", golf: "g",
  h: "h", aitch: "h", age: "h", hotel: "h", each: "h",
};
const RANK_WORDS = {
  1: 1, one: 1, won: 1, first: 1,
  2: 2, two: 2, too: 2, second: 2,
  3: 3, three: 3, tree: 3, free: 3, third: 3,
  4: 4, four: 4, for: 4, fore: 4, fourth: 4,
  5: 5, five: 5, fifth: 5,
  6: 6, six: 6, sex: 6, sixth: 6, sicks: 6,
  7: 7, seven: 7, seventh: 7,
  8: 8, eight: 8, ate: 8, eighth: 8, ait: 8,
};
// The knight the player rides is named by what is said to a horse: "ride to
// f3", "gallop to e5", "charge d5".
const RIDE = new Set(["ride", "rides", "gallop", "gallops", "charge", "charges", "steed", "mount", "onward", "giddyup"]);
const TAKES = new Set(["takes", "take", "captures", "capture", "x", "times", "kills", "smashes", "smash", "attacks", "attack", "tanks"]);
const FILLER = new Set(["to", "the", "a", "an", "my", "move", "moves", "go", "goes", "please", "and", "on", "at", "square", "from",
  "into", "piece", "then", "now", "becomes", "become", "promote", "promotes", "promotion", "equals", "check", "checkmate", "mate", "with"]);

// What a command is made of, in the order it was said.
function tokens(text) {
  const words = text.toLowerCase()
    .replace(/[+#!?.,;:]/g, " ")
    .replace(/=/g, " = ")
    .replace(/\b([a-h])\s*([1-8])\b/g, " $1$2 ")
    .split(/[\s-]+/).filter(Boolean);
  const out = [];
  for (let at = 0; at < words.length; at++) {
    const word = words[at], next = words[at + 1];
    if (/^[a-h][1-8]$/.test(word)) { out.push({square: word}); continue; }
    if (/^[a-h][1-8][a-h][1-8][qrbn]?$/.test(word)) {
      out.push({square: word.slice(0, 2)}, {square: word.slice(2, 4)});
      if (word[4]) out.push({piece: word[4], promotion: true});
      continue;
    }
    // A file, and the rank said after it, are a square: "e 4", "echo four".
    // "to" between two squares is the word and not the number: "e2 to e4".
    if (word in FILE_WORDS && next in RANK_WORDS && !(word === "a" && next === "to")) {
      out.push({square: FILE_WORDS[word] + RANK_WORDS[next]});
      at += 1;
      continue;
    }
    if (TAKES.has(word)) { out.push({takes: true}); continue; }
    if (RIDE.has(word)) { out.push({mount: true}); continue; }
    if (word === "castle" || word === "castles" || word === "castling") {
      // "castle" is also what many people call a rook. With a side, or
      // alone, it is the move; before a square it is the piece.
      const side = words.slice(at + 1).find(other => !FILLER.has(other));
      if (side === undefined || /^(king|kings|kingside|short|queen|queens|queenside|long)$/.test(side)) {
        out.push({castle: true});
        continue;
      }
    }
    if (word === "kingside" || word === "short") { out.push({side: "k"}); continue; }
    if (word === "queenside" || word === "long") { out.push({side: "q"}); continue; }
    if (word === "side") continue;
    if (word === "=") { out.push({promotes: true}); continue; }
    if (Object.hasOwn(PIECES, word)) { out.push({piece: PIECES[word]}); continue; }
    // "a" is a word and a file. After a piece it is the file it came from
    // ("rook a to d1"); anywhere else it is the word.
    const last = out[out.length - 1];
    if (word === "a" && last && last.piece) { out.push({file: "a"}); continue; }
    if (FILLER.has(word)) {
      if (/^(becomes?|promotes?|promotion|equals)$/.test(word)) out.push({promotes: true});
      continue;
    }
    // A file said by itself: the letter, or a word of three letters or more
    // that is heard for one. The two-letter ones ("be", "he", "if") are
    // only a file with a rank after them, which was read above.
    if (word in FILE_WORDS && word.length !== 2) { out.push({file: FILE_WORDS[word]}); continue; }
    // "to" comes back from a microphone as "2" or "two" as often as not:
    // with something after it, it is the word.
    if (RANK_WORDS[word] === 2 && next !== undefined) continue;
    if (word in RANK_WORDS) { out.push({rank: RANK_WORDS[word]}); continue; }
    out.push({unknown: word});
  }
  return out;
}

// The way a game is written down, which says more in fewer letters than
// speech does and is read by its own rule: Nbd7, R1e2, exd5, e8=Q, O-O.
function written(text) {
  let mark = text.trim().replace(/[+#!?]+$/g, "");
  // Typed without the capital: "nf3", "qxd5". Not "b", which is a file.
  if (/^[kqrn][a-h1-8x]*[a-h][1-8]/.test(mark)) mark = mark[0].toUpperCase() + mark.slice(1);
  mark = mark.replace(/=?([qrbn])$/, (whole, promo) => /[18]=?[qrbn]$/.test(mark) ? "=" + promo.toUpperCase() : whole);
  if (/^(o-o-o|0-0-0)$/i.test(mark)) return {castle: "q"};
  if (/^(o-o|0-0)$/i.test(mark)) return {castle: "k"};
  const found = /^([KQRBN])?([a-h])?([1-8])?(x)?([a-h][1-8])(?:=?([QRBN]))?$/.exec(mark);
  if (!found) return null;
  const [, piece, file, rank, takes, to, promo] = found;
  return {piece: (piece || "P").toLowerCase(), file: file || null, rank: rank ? +rank : null, takes: !!takes, to,
    promo: promo ? promo.toLowerCase() : null};
}

// What a spoken or typed command names, as a test of a move, or an error in
// words when it names nothing to go on.
function spokenWants(heard, moves) {
  const said = tokens(heard);
  const unknown = said.filter(token => token.unknown).map(token => token.unknown);
  const squares = said.filter(token => token.square).map(token => token.square);
  const castle = said.some(token => token.castle), side = (said.find(token => token.side) || {}).side;
  const takes = said.some(token => token.takes);
  // A piece named after "becomes", or a second piece, is what a pawn turns into.
  let piece = null, promo = null, promoting = false;
  for (const token of said) {
    if (token.promotes) promoting = true;
    else if (token.piece) {
      if (token.promotion || promoting || piece !== null) promo = token.piece;
      else piece = token.piece;
    }
  }
  const files = said.filter(token => token.file).map(token => token.file);
  const ranks = said.filter(token => token.rank).map(token => token.rank);
  if (castle || (side && !squares.length)) {
    // "castle" alone is the kingside, or the one side that is on offer.
    const castles = moves.filter(move => move.flag && move.flag.startsWith("castle"));
    const queens = said.some(token => token.piece === "q") || side === "q";
    const kings = said.some(token => token.piece === "k") || side === "k";
    return {wants: move => !!move.flag && move.flag.startsWith("castle") &&
      (queens ? move.flag === "castle-q" : kings ? move.flag === "castle-k" : castles.length === 1 || move.flag === "castle-k")};
  }
  const to = squares.length ? squares[squares.length - 1] : null;
  const from = squares.length > 1 ? squares[0] : null;
  // The knight ridden, named by what is said to a horse, is a piece named.
  if (!to && !piece && !takes && !said.some(token => token.mount)) {
    return {error: unknown.length ? `I heard “${heard}”, and do not know a move by that.` : `“${heard}” names no square to go to.`};
  }
  const fit = (kind, becomes) => move =>
    (!to || squareName(move.to) === to) &&
    (!from || squareName(move.from) === from) &&
    (!kind || move.piece.toLowerCase() === kind) &&
    (!takes || !!move.captured) &&
    (!becomes || (move.promo || "").toLowerCase() === becomes) &&
    // A file or a rank said by itself says where the piece comes from
    // ("rook a to d1") or, with no square at all, where a capture lands.
    files.every(file => squareName(move.from)[0] === file || (!to && squareName(move.to)[0] === file)) &&
    ranks.every(rank => (move.from >> 3) + 1 === rank || (!to && (move.to >> 3) + 1 === rank));
  // "e8 queen": no queen can go to e8, and a pawn can, to become one.
  const becoming = piece && !promo && piece !== "p" && piece !== "k" ? fit("p", piece) : null;
  return {wants: fit(piece, promo), becoming, named: piece !== null, riding: said.some(token => token.mount)};
}

// The legal moves a command, or the start of one, could mean, before one is
// chosen among them: what the board hears while it is still being said.
function fitting(position, heard, mount) {
  const moves = legalMoves(position);
  // The written game first, which says the most in the fewest letters, and
  // then the words. A reading that no legal move fits gives way to the next.
  let fits = [];
  const note = written(heard);
  if (note) {
    fits = moves.filter(note.castle
      ? move => move.flag === `castle-${note.castle}`
      : move => move.piece.toLowerCase() === note.piece && squareName(move.to) === note.to &&
          (!note.file || squareName(move.from)[0] === note.file) && (!note.rank || (move.from >> 3) + 1 === note.rank) &&
          (!note.takes || !!move.captured) && (!note.promo || (move.promo || "").toLowerCase() === note.promo));
  }
  let named = !!note;
  if (!fits.length) {
    const said = spokenWants(heard, moves);
    if (said.error) return {error: said.error, moves};
    fits = moves.filter(said.wants);
    if (!fits.length && said.becoming) fits = moves.filter(said.becoming);
    named = said.named;
    if (said.riding) {
      if (mount < 0) return {error: "You are not riding a knight.", moves};
      fits = fits.filter(move => move.from === mount);
      named = true;
    }
  }
  if (fits.length > 1 && mount >= 0 && fits.every(move => move.piece.toLowerCase() === "n") && fits.some(move => move.from === mount))
    fits = fits.filter(move => move.from === mount);
  return {fits, named, moves};
}

// Every legal move that what has been said so far could still mean ("knight":
// every knight move; "knight takes": every knight capture), or none when it
// names nothing yet. The page makes the pieces spoken to stir before the
// command is done.
export function candidates(position, text, {mount = -1} = {}) {
  const heard = text.trim();
  if (!heard) return [];
  const read = fitting(position, heard, mount);
  return read.error ? [] : read.fits;
}

// A command read against a position: `{move}` when exactly one legal move
// fits it, and otherwise `{error}` in words for the player, with `options`
// when the trouble is that several fit.
//
// `mount` is the square of the knight the player rides, or -1: "ride to f3"
// is that knight's move, and when two knights could go where a command says
// the one ridden is the one meant.
export function readCommand(position, text, {mount = -1} = {}) {
  const heard = text.trim();
  if (!heard) return {error: "Nothing was said."};
  const read = fitting(position, heard, mount);
  if (read.error) return {error: read.error};
  let {fits} = read;
  const {named, moves} = read;
  // A pawn that reaches the last rank becomes a queen unless told otherwise.
  if (fits.length > 1 && fits.every(move => move.promo && move.from === fits[0].from && move.to === fits[0].to))
    fits = fits.filter(move => move.promo.toLowerCase() === "q");
  // With no piece named, a square is a pawn's when one pawn can go there
  // ("e4"), as it is in the written game.
  if (fits.length > 1 && !named) {
    const pawns = fits.filter(move => move.piece.toLowerCase() === "p");
    if (pawns.length === 1) fits = pawns;
  }
  if (fits.length === 1) return {move: fits[0]};
  if (!fits.length) return {error: `“${heard}” is not a move that can be played here.`};
  const options = fits.map(move => san(position, move, moves));
  const kinds = new Set(fits.map(move => move.piece.toLowerCase()));
  const which = kinds.size === 1 ? `Which ${PIECE_NAMES[[...kinds][0]]}?` : "Which move?";
  return {error: `${which} ${options.slice(0, 6).join(", ")}${options.length > 6 ? "…" : ""}`, options};
}
