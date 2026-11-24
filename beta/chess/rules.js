// The rules of chess, for Wizard Chess: what may be played from a position,
// what a position is after a move, and how a move is written. It draws
// nothing and plays nothing; `game.js` shows the board and the engine
// (`engine.js`) picks the opponent's moves.
//
// A square is 0..63, a1 = 0, h1 = 7, a8 = 56. A piece is one letter, upper
// case for White: P N B R Q K. A position is never changed in place: `play`
// answers with a new one.
//
// `rules_test.mjs` holds this to the published perft counts (how many
// positions are reachable in so many moves), which is how a move generator
// is known to be right: en passant, castling through check, pins and
// promotions each have a position that miscounts if they are wrong.

export const START = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

const FILES = "abcdefgh";
export const squareName = sq => FILES[sq & 7] + ((sq >> 3) + 1);
export const squareAt = name => {
  const file = FILES.indexOf(name[0]), rank = +name[1] - 1;
  return file < 0 || !(rank >= 0 && rank < 8) || name.length !== 2 ? -1 : rank * 8 + file;
};
export const colourOf = piece => (piece === piece.toUpperCase() ? "w" : "b");
export const PIECE_NAMES = {p: "pawn", n: "knight", b: "bishop", r: "rook", q: "queen", k: "king"};

export function fromFen(fen) {
  const [placed, turn, castling, ep, half, full] = fen.trim().split(/\s+/);
  const board = new Array(64).fill(null);
  placed.split("/").forEach((row, down) => {
    let file = 0;
    for (const mark of row) {
      if (mark >= "1" && mark <= "8") file += +mark;
      else board[(7 - down) * 8 + file++] = mark;
    }
  });
  return {board, turn: turn === "b" ? "b" : "w", castling: castling === "-" || !castling ? "" : castling,
    ep: ep && ep !== "-" ? squareAt(ep) : -1, half: +half || 0, full: +full || 1};
}

export function toFen(position) {
  const rows = [];
  for (let rank = 7; rank >= 0; rank--) {
    let row = "", empty = 0;
    for (let file = 0; file < 8; file++) {
      const piece = position.board[rank * 8 + file];
      if (!piece) { empty++; continue; }
      if (empty) { row += empty; empty = 0; }
      row += piece;
    }
    rows.push(row + (empty || ""));
  }
  return `${rows.join("/")} ${position.turn} ${position.castling || "-"} ` +
    `${position.ep >= 0 ? squareName(position.ep) : "-"} ${position.half} ${position.full}`;
}

// The same position for the threefold rule: the pieces, whose move it is,
// the castling rights and an en passant square a pawn could really take on.
export function positionKey(position) {
  let ep = "-";
  if (position.ep >= 0 && legalMoves(position).some(move => move.flag === "ep")) ep = squareName(position.ep);
  return `${toFen(position).split(" ").slice(0, 3).join(" ")} ${ep}`;
}

const KNIGHT = [[1, 2], [2, 1], [2, -1], [1, -2], [-1, -2], [-2, -1], [-2, 1], [-1, 2]];
const KING = [[1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0], [-1, -1], [0, -1], [1, -1]];
const ROOK = [[1, 0], [0, 1], [-1, 0], [0, -1]];
const BISHOP = [[1, 1], [-1, 1], [-1, -1], [1, -1]];

const step = (sq, [df, dr]) => {
  const file = (sq & 7) + df, rank = (sq >> 3) + dr;
  return file < 0 || file > 7 || rank < 0 || rank > 7 ? -1 : rank * 8 + file;
};

// Whether a square is attacked by a colour's pieces as they stand.
export function attacked(board, sq, by) {
  const mine = piece => piece && colourOf(piece) === by;
  const pawnFrom = by === "w" ? -1 : 1;
  for (const df of [-1, 1]) {
    const from = step(sq, [df, pawnFrom]);
    if (from >= 0 && board[from] === (by === "w" ? "P" : "p")) return true;
  }
  for (const hop of KNIGHT) {
    const from = step(sq, hop);
    if (from >= 0 && mine(board[from]) && board[from].toLowerCase() === "n") return true;
  }
  for (const hop of KING) {
    const from = step(sq, hop);
    if (from >= 0 && mine(board[from]) && board[from].toLowerCase() === "k") return true;
  }
  for (const [lines, sliders] of [[ROOK, "rq"], [BISHOP, "bq"]]) {
    for (const line of lines) {
      for (let at = step(sq, line); at >= 0; at = step(at, line)) {
        const piece = board[at];
        if (!piece) continue;
        if (mine(piece) && sliders.includes(piece.toLowerCase())) return true;
        break;
      }
    }
  }
  return false;
}

export function kingOf(board, colour) {
  return board.indexOf(colour === "w" ? "K" : "k");
}
export function inCheck(position, colour = position.turn) {
  const king = kingOf(position.board, colour);
  return king >= 0 && attacked(position.board, king, colour === "w" ? "b" : "w");
}

// Every move the side to move could make if its own king's safety did not
// matter: {from, to, piece, captured, promo, flag}. `flag` is "ep", "double",
// "castle-k", "castle-q" or null.
function pseudoMoves(position) {
  const {board, turn} = position, moves = [];
  const foe = turn === "w" ? "b" : "w";
  const add = (from, to, extra = {}) => moves.push({from, to, piece: board[from], captured: board[to] || null, promo: null, flag: null, ...extra});
  for (let from = 0; from < 64; from++) {
    const piece = board[from];
    if (!piece || colourOf(piece) !== turn) continue;
    const kind = piece.toLowerCase();
    if (kind === "p") {
      const up = turn === "w" ? 1 : -1, home = turn === "w" ? 1 : 6, last = turn === "w" ? 7 : 0;
      const promote = (to, extra = {}) => {
        if ((to >> 3) === last) for (const promo of "qrbn") add(from, to, {...extra, promo: turn === "w" ? promo.toUpperCase() : promo});
        else add(from, to, extra);
      };
      const one = step(from, [0, up]);
      if (one >= 0 && !board[one]) {
        promote(one);
        const two = step(one, [0, up]);
        if ((from >> 3) === home && !board[two]) add(from, two, {flag: "double"});
      }
      for (const df of [-1, 1]) {
        const to = step(from, [df, up]);
        if (to < 0) continue;
        if (board[to] && colourOf(board[to]) === foe) promote(to);
        else if (to === position.ep) add(from, to, {flag: "ep", captured: turn === "w" ? "p" : "P"});
      }
      continue;
    }
    if (kind === "n" || kind === "k") {
      for (const hop of kind === "n" ? KNIGHT : KING) {
        const to = step(from, hop);
        if (to >= 0 && (!board[to] || colourOf(board[to]) === foe)) add(from, to);
      }
    } else {
      const lines = kind === "r" ? ROOK : kind === "b" ? BISHOP : KING;
      for (const line of lines) {
        for (let to = step(from, line); to >= 0; to = step(to, line)) {
          if (!board[to]) { add(from, to); continue; }
          if (colourOf(board[to]) === foe) add(from, to);
          break;
        }
      }
    }
    if (kind === "k") {
      // Castling: the right is still held, the squares between are empty,
      // and the king is not in check, does not pass through it and does
      // not land in it.
      const rank = turn === "w" ? 0 : 7, home = rank * 8 + 4;
      if (from !== home || attacked(board, home, foe)) continue;
      const [short, long] = turn === "w" ? ["K", "Q"] : ["k", "q"];
      const rook = turn === "w" ? "R" : "r";
      if (position.castling.includes(short) && board[home + 3] === rook && !board[home + 1] && !board[home + 2] &&
          !attacked(board, home + 1, foe) && !attacked(board, home + 2, foe)) add(home, home + 2, {flag: "castle-k"});
      if (position.castling.includes(long) && board[home - 4] === rook && !board[home - 1] && !board[home - 2] && !board[home - 3] &&
          !attacked(board, home - 1, foe) && !attacked(board, home - 2, foe)) add(home, home - 2, {flag: "castle-q"});
    }
  }
  return moves;
}

// The position after a move. The move is taken to be one `legalMoves` gave.
export function play(position, move) {
  const board = position.board.slice(), {from, to} = move, turn = position.turn;
  board[to] = move.promo || board[from];
  board[from] = null;
  if (move.flag === "ep") board[to + (turn === "w" ? -8 : 8)] = null;
  if (move.flag === "castle-k") { board[to - 1] = board[to + 1]; board[to + 1] = null; }
  if (move.flag === "castle-q") { board[to + 1] = board[to - 2]; board[to - 2] = null; }
  // A right goes when its king or rook moves, or its rook is taken.
  let castling = position.castling;
  const lose = marks => { for (const mark of marks) castling = castling.replace(mark, ""); };
  for (const sq of [from, to]) {
    if (sq === 4) lose("KQ"); else if (sq === 60) lose("kq");
    else if (sq === 0) lose("Q"); else if (sq === 7) lose("K");
    else if (sq === 56) lose("q"); else if (sq === 63) lose("k");
  }
  const pawn = move.piece.toLowerCase() === "p";
  return {board, turn: turn === "w" ? "b" : "w", castling,
    ep: move.flag === "double" ? (from + to) >> 1 : -1,
    half: pawn || move.captured ? 0 : position.half + 1,
    full: position.full + (turn === "b" ? 1 : 0)};
}

export function legalMoves(position) {
  return pseudoMoves(position).filter(move => !inCheck(play(position, move), position.turn));
}

// How many positions are reachable in `depth` moves: the generator's check.
export function perft(position, depth) {
  if (depth === 0) return 1;
  const moves = legalMoves(position);
  if (depth === 1) return moves.length;
  let count = 0;
  for (const move of moves) count += perft(play(position, move), depth - 1);
  return count;
}

// Neither side can ever give mate: king against king, a lone knight or
// bishop, or bishops that all stand on one colour of square.
export function deadPosition(board) {
  const minor = [];
  for (let sq = 0; sq < 64; sq++) {
    const piece = board[sq];
    if (!piece || piece.toLowerCase() === "k") continue;
    if (!"nb".includes(piece.toLowerCase())) return false;
    minor.push([piece.toLowerCase(), ((sq & 7) + (sq >> 3)) & 1]);
  }
  if (minor.length <= 1) return true;
  return minor.every(([kind, shade]) => kind === "b" && shade === minor[0][1]);
}

// How the game stands for the side to move: "checkmate", "stalemate",
// "fifty" (a hundred half-moves with no pawn moved and nothing taken),
// "dead", or "check" or "play". `seen` counts positions by `positionKey`,
// and one seen three times is "threefold".
export function standing(position, seen = null) {
  const moves = legalMoves(position), check = inCheck(position);
  if (!moves.length) return check ? "checkmate" : "stalemate";
  if (deadPosition(position.board)) return "dead";
  if (position.half >= 100) return "fifty";
  if (seen && (seen.get(positionKey(position)) || 0) >= 3) return "threefold";
  return check ? "check" : "play";
}

// A move as the engine writes it: e2e4, e7e8q.
export function uci(move) {
  return squareName(move.from) + squareName(move.to) + (move.promo ? move.promo.toLowerCase() : "");
}
export function moveFromUci(position, text) {
  return legalMoves(position).find(move => uci(move) === text) || null;
}

// A move in the notation a game is written down in: Nf3, exd5, O-O, e8=Q+.
export function san(position, move, moves = legalMoves(position)) {
  let text;
  if (move.flag === "castle-k") text = "O-O";
  else if (move.flag === "castle-q") text = "O-O-O";
  else {
    const kind = move.piece.toUpperCase(), to = squareName(move.to), takes = move.captured ? "x" : "";
    if (kind === "P") text = (takes ? FILES[move.from & 7] + "x" : "") + to + (move.promo ? "=" + move.promo.toUpperCase() : "");
    else {
      // As little of where it came from as tells it from the others of its
      // kind that could go there: the file, else the rank, else both.
      const others = moves.filter(other => other.piece === move.piece && other.to === move.to && other.from !== move.from);
      let from = "";
      if (others.length) {
        const file = FILES[move.from & 7], rank = String((move.from >> 3) + 1);
        if (!others.some(other => (other.from & 7) === (move.from & 7))) from = file;
        else if (!others.some(other => (other.from >> 3) === (move.from >> 3))) from = rank;
        else from = file + rank;
      }
      text = kind + from + takes + to;
    }
  }
  const after = play(position, move);
  if (inCheck(after)) text += legalMoves(after).length ? "+" : "#";
  return text;
}

// A move said aloud: "knight takes f3, check".
export function spoken(position, move) {
  const after = play(position, move), check = inCheck(after);
  const ending = check ? (legalMoves(after).length ? ", check" : ", checkmate") : "";
  if (move.flag === "castle-k") return `castles kingside${ending}`;
  if (move.flag === "castle-q") return `castles queenside${ending}`;
  const name = PIECE_NAMES[move.piece.toLowerCase()];
  const promo = move.promo ? `, and becomes a ${PIECE_NAMES[move.promo.toLowerCase()]}` : "";
  return `${name} ${move.captured ? "takes" : "to"} ${squareName(move.to)}${promo}${ending}`;
}
