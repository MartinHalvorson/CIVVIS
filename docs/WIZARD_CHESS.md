# Wizard Chess

Operator, 2026-11-23: "below civ 6, add wizard chess. the background there
can be black and white checkerboard. in it, i want you to build wizard chess.
a laptop version and a version for a VR headset (but that can be later).
please buid out the laptop version. work efficiently. i want you to best
utilize the screen of the latop to create the experience of a person playing
wizard chess. analyze what and how you should approach buliding this. the
chess strategy can be set to a chess elo and the chess will play at that
elo."

It is the last name in the site list, on a chessboard of a button, and it
opens `/wizard-chess/`: a page of its own with nothing of the simulator in
it. This is what was decided, and why.

## What the experience is

Wizard chess is chess in which nobody touches a piece. The player says what
is to happen, the piece does it by itself, and a piece that takes another
breaks it. So the experience has four parts, and a version of the game is a
way of giving each of them with what the device has:

1. **You command.** A move is something said, not something done with the
   hand.
2. **The pieces obey, and are alive.** They move themselves, each in its own
   way, and a capture is a blow.
3. **You are at the board.** The pieces stand over it; you look across it at
   the other side.
4. **Somebody plays against you**, as well as you asked them to.

## The laptop version

A laptop has a wide screen, a microphone, speakers, a trackpad and a
keyboard. It has no depth and no hands. The page uses each for one of the
four parts.

**The screen is the player's seat.** The board is the whole window, with no
page around it. The view is from the player's own side, low enough that the
near pieces stand tall and the far ones recede into a dark hall, and exactly
as far back as it takes for the whole board to be on the screen between what
is written at the top and the command at the foot (`Seat.resize` finds that
distance for the window's shape, so a 16:10 laptop and a wide monitor each
get a board that fills them). Everything written stands at the edges: the
opponent and the game's controls top right, the game as it is written down
under them, the command at the bottom. Two fingers on the trackpad turn the
view round the board and over it, a pinch draws it near, a double click puts
it back.

**A move is a command, given three ways that are one thing.**

- *Said.* Hold Space (or the microphone button) and say it: "knight to f3",
  "pawn takes d5", "castle kingside", "e2 to e4". The browser's own speech
  recognition hears it, where the browser has one (Chrome does). In a
  browser without it the page says so and the button is gone.
- *Typed.* Start typing anywhere: the same words, or the way a game is
  written down ("Nf3", "exd5", "O-O", "e8=Q").
- *Pointed.* Click a piece, which wakes and shows where it may go, and
  click a square.

All three end in the same place: the command is read against the position
(`chess/command.js`), and the piece then makes the move itself. What a
microphone hears is not what was said (a file comes back as "see" or "bee",
a rank as "for" or "ate", a knight as "night"), so the reader does not parse
a grammar. It collects what the command names and asks which legal moves fit
all of it. One is the move; none or several is said back in words ("Which
knight? Nbd2, Nfd2"). The phonetic alphabet is read as files, which is what
to say when the letters will not come through ("knight to foxtrot three").
The recognizer gives several guesses for what it heard, and the first that
is a legal move is taken.

**The pieces are stone, and move themselves** (`World.play`). A piece tips
into its move and rights itself as it arrives. A knight leaps. Castling
brings the rook over the king. A capture is each piece's own: a pawn and a
king rear back and lunge, a rook rams without stopping, a bishop comes in
spinning, a knight comes down on what it takes, and a queen rises where she
stands and what she looks at breaks. The piece taken shatters into rubble
that is thrown, falls, lies on the board and is gone. The view leans in for
it. A king in check stands on a square that pulses red, and a king that is
mated falls over. Stone on stone is heard, made in the page rather than
recorded (`Sound`), and the opponent's move is said aloud as it is made.

**The opponent plays at an Elo** (`chess/engine.js`). It is Stockfish 18, in
a worker of its own, held to a strength by Stockfish's own setting
(`UCI_LimitStrength` with `UCI_Elo`). The range, 1320 to 3190, is the
engine's and is read from what it says of itself when it starts. What the
number means is Stockfish's to say, and its source does (`src/search.h`,
17.1): "based on a fit of the Elo results for games played between Stockfish
at various skill levels and various versions of the Stash engine. Skill 0 ..
19 now covers CCRL Blitz Elo from 1320 to 3190, approximately". So an Elo
here is a computer rating list's, approximately. It is not a FIDE rating or
a chess site's, and nothing in this repository has measured it. At the top
of the range the limit is taken off. There is no setting under 1320: the
engine has none, and a weaker player made here would have a number nobody
had measured.

## How it is built

| File | What it is |
|------|------------|
| `beta/chess.html` | The page: the board's canvas and what is written at its edges |
| `beta/chess/game.js` | The world (hall, board, pieces, how a move is played out), the seat (camera, pointer, keyboard), the voice, the sound, and the game |
| `beta/chess/rules.js` | The rules of chess: legal moves, the position after one, check, mate and the draws, and a move written (SAN, UCI) and said aloud |
| `beta/chess/command.js` | A command, said or typed, read as a move |
| `beta/chess/engine.js` | The opponent: Stockfish in a worker, one move at a time at an Elo |
| `beta/chess/vendor/` | three.js r160 (MIT) and Stockfish.js 18, lite and single-threaded (GPL-3.0, this repository's license), as published; its README names versions and sources |

Two things were not written here, on purpose. The board is drawn with
three.js because it gives shadows and reflections now and a headset later.
The opponent is Stockfish because "plays at that Elo" needs an engine whose
strength setting somebody measured; one written here would have had an Elo
nobody had. Both are in the repository as files (8 MB together) and not
fetched from somebody else's server when the page loads.

The rules are written here, in under 300 lines, and held to the published perft
counts for six positions (`beta/chess/rules_test.mjs`): the number of
positions reachable in so many moves, which a move generator gets wrong the
moment castling, en passant, a pin or a promotion is wrong. A whole game is
read from its written moves and written back the same, and the commands are
checked as a microphone garbles them.

`beta/publish.sh` puts the page at `wizard-chess/` and its directory beside
it. `beta/wizard_chess_test.py` runs the node checks, holds the page to its
address and the vendored files to their bytes, and opens the page in a
browser: the engine wakes and gives its range, a commanded pawn makes its
move, the opponent answers, a move is taken back, and a capture leaves one
piece fewer.

Measured in headless Chrome on a MacBook Air (M4), the page served from a
local directory with its files already cached: the board is on screen about
0.15 s after the page is asked for and the engine is awake at about 0.2 s
(three runs: 0.20, 0.18, 0.17 s). Over a network the engine is a 7.3 MB
file and wakes when it has arrived; the board does not wait for it. The
opponent is given 0.9 s to think about each move.

## The headset version, later

The world is built so that a headset can take it as it is.

- `World` (the hall, the board, the pieces and `World.play`) knows nothing
  of a screen. It is in squares, one unit a square, and is told only which
  move to play out and whom to tell about sound and where to look.
- `Seat`, `Voice` and the pointer and keyboard handlers are the laptop. A
  headset replaces them: three.js's `renderer.xr` for the two eyes, the
  player standing at their side of a board scaled to the room (a square a
  pace wide, pieces to the shoulder), controllers or hands to point with,
  and the same `readCommand` behind a microphone that needs no key held.
- The game, the rules and the engine are untouched.

What a headset adds that a laptop cannot give is the third part of the
experience, being at the board: pieces that are as tall as they are, a
capture that happens at arm's length, rubble that lands at your feet. None
of that is built.

## What it does not do

- It does not hear speech in a browser without speech recognition; typing
  and pointing work everywhere. Speech was not tried with a real microphone
  while this was built (the checks run in a browser with nobody to speak):
  what is checked is the reader, on the words a recognizer returns.
- A browser's speech recognition is the browser's, and may send what it
  hears to its maker's service to be recognized. The page asks for the
  microphone only when Space or the microphone button is first held.
- There is no clock, no opening book of its own, no saved games beyond the
  one in progress (kept in the browser, and set down again on return), and
  no two-player mode.
- Nothing weaker than Elo 1320 (see above).
- Not built for a phone: it works by touch, but the layout is a laptop's.
