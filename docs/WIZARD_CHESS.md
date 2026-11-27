# Wizard Chess

> **Set aside, 2026-11-24.** Operator: "also, lets set aside wizard chess
> right now. please remove from civvis.ai". Wizard Chess is off the site: no
> page lists it, `beta/publish.sh` publishes nothing of it, and
> `/wizard-chess/` answers the site's 404. The game is kept in the tree
> (`beta/chess.html`, `beta/chess/`, `beta/wizard_chess_test.py`, this
> document). Putting it back on the site is reverting #5793, the pull request
> that took it off. What follows describes it as it stood on the site.

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
  click a square. From a low seat a piece stands in front of the ones
  behind it, so a click means the first thing along the line of sight that
  it can mean (`meant`): a piece that has no move does not hide the one
  behind it, and a click on a piece is never a click on the square behind
  it.

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

## Riding a knight

Operator, 2026-11-24 (a `/loop` every 30 minutes): "build wizard chess out
more and more to be like the real experience of sitting on a knight in the
harry potter game of wizard chess. optimizz everything. what is tthe optimal
meta structure for this?"

What sitting on a knight adds to the four parts is being *in* the game
rather than over it: the pieces are as tall as you, your own knight carries
you when it moves, the pieces that can take it are the ones you watch, and
if it is taken you go with it. The set, the hall and everything in them
stay this page's own; nothing is taken from the film.

**The seat** (`Seat`, modes `table`, `ride`, `thrown`, `aside`). "On a
knight", top right, puts the player on the back of their king's knight
("Other knight" changes it). The eye is a point on the knight itself
(`EYE`), so the rider is carried through its leap, rears with it as it takes
off and lurches as it lands. It is low on the knight's neck and to its right:
the knight's head is beside the rider, its cloth under them, and the pieces
stand over them, a pawn near their eye, a queen a third again its height and
a king more than half again (the table seat is unchanged). On the right,
the head is on the left of the screen, where nothing written stands over
it, and the plates fold to what a rider needs (the opponent's strength, the
seat, the game) and let the hall show through them. The
field of view widens from 36° to 64°. The rider's head turns to what is
happening: a capture most of all, and any piece that moves. A drag, two
fingers or an arrow key turn the head instead (take hold of the world and
pull it), and the head stays where it was put for two seconds. The
knight's head is on the rider's left: turned toward it they rise in the
saddle and lean in to see over it (`Seat.rise`), fully by half a right angle
round, and sit down again when they look ahead. From one seat
to another the view travels; it does not cut. A rider does not see their own
cloak.

**Two players at one laptop** (`game.versus`, `pass`, `World.dress`). "A
person here", top right, has a second player take the other side instead of
the engine: the two take turns at the laptop. After each move the board is
handed over: the seat goes round to the side to move (or flies to the piece
that player rides), the names of the files and ranks turn to be read from
there, and it is said whose move it is, with what now threatens that
player's piece. Each side rides a piece of its own (`game.mounts`), and the
rider who has just moved stays on their piece, seen from the other side, a
cloaked figure with their reins. A take back is the last move, whoever made
it, and the board goes back to them. There is no Elo to set, so it is not
shown.

**A knight gathers and lands** (`World.gather`, `World.land`). Before it
leaps, a knight sinks a tenth of a square and tips back onto its haunches,
and the leap springs out of that, rearing as before; as it lands it gives,
nose down to level with a nod in it, and comes back up. A rider feels the
whole of it: measured at their eye over a leap from g1 to f3, at rest 1.24,
the knight down 0.10 as it gathers, the eye up to 2.44 at the top, the knight
down 0.09 as it lands, and the view surges wider for an instant as it
springs (`hooks.surge`).

**The fallen stay fallen** (`World.heaps`). A piece taken does not only
break: what it was lies as a heap of its broken stone on the board's rim for
the rest of the game, Black's along the left of White's side and White's
along the right, half a square apart, a bigger heap for a bigger piece. A
take back stands it up again; a new game clears the rim.

**The rider commands with their hand** (`World.gesture`). Commanding the
piece they ride (or any piece) from the saddle, the rider's right hand is
thrown out at arm's length toward the square, and stays pointing while the
command is obeyed.

**The rider's own hands** (`World.arms`, `World.reach`). What a rider sees
of themself: two gauntleted fists with gold cuffs and the forearms up to
them from below the eye. On a knight they hold the reins, which end in them;
standing on any other piece they are held out before the rider, braced, in
the bottom corners of the view. They grip, drawn back and clenched, while
the piece moves under the rider, and ease when it stands. The rider's cloak
and hood are hidden from their own eye; their hands are not.

**Any piece can be ridden** (`rideSquare`, `RIDER_TOPS`). A double click
on one of the player's own pieces rides it, from either seat; anywhere else
a double click puts the view back. The knight is ridden in its saddle (below);
any other piece is ridden standing on top of it, its rider seen there from
the board: on a pawn's head, among the pieces' feet with everything over
them; in a rook's battlements; on a bishop's mitre; in the queen's crown or
the king's, high over the board. The piece carries its rider as it goes,
turns to where it is going and back to the enemy after, and is what "ride
to …" moves. The button names it ("On a pawn"), and so do the warnings ("Your
queen is threatened …"). A pawn ridden to the last rank carries its rider
up onto what it becomes.

**The knight ridden** (`World.ride`) wears a cloth over its neck in its
side's colour, with its rider sitting behind its ears, so that from the
board it is plain which knight is ridden. It is alive to the one on its
back (`World.attend`): a carved crest of mane along its neck, an eye lit from inside with its
side's light that closes now and then, a bit and reins that run back to the
rider's hands, and ears that turn toward the piece that is moving and prick
up while it moves. It breathes out as it lands, and the stone groans under
the rider as it turns. It looks straight at the enemy, not
in toward the middle like the others, and turns to where it is going before
it goes.

**Sound where it happens** (`Sound.place`, `Sound.hear`). Every sound the
world makes is made at the point of the hall where it happens, through a
panner, and heard from the seat's eye, facing the way it faces. At the board
everything is some way off and heard alike; on a knight a sound is at full
strength only within about two squares, so a capture beside the rider is
loud and on that side, one across the board is far, and one behind is
behind.

**The pieces alive** (`World.regard`, `World.flinch`). Not only the knight
ridden: every piece standing watches the piece that is moving, a knight
turning its head to it and the rest leaning a little its way, less the
further off. A king in check flinches away from what checks it, shaking,
until it is out of check. The piece about to be taken trembles from the
moment the blow is coming until it breaks (`World.doomed`). Who holds
themselves how is one rule with an order: the threat to the knight ridden
first, then a king in check, then watching.

**The board hears a command as it is given** (`candidates`, `World.stir`,
`stir`). While a command is still being typed, or said (the recognizer's
first guesses, before it is sure), the board listens: the pieces it could
be meant for stir, lifting a little with their light brightening ("knight":
both knights; "ride": the knight ridden), as long as that is a few of them
and not the whole side. When what is said can only be a capture of one
piece ("knight takes e5"), that piece turns to face what is coming and
braces. Cleared, or played, the board is still again.

**The opponent deliberates** (`World.muse`, `answer`). While the opponent
thinks, its pieces shift their weight, each a little and in its own time.
When it has chosen, there is a breath before the move is played (never less
than 0.55 s): the piece chosen stirs, what it will take braces, and a
rider's eye is drawn to it, so that from the saddle the blow is seen
coming. Then it is said and played.

**Riding into the blow** (`hooks.impact`, `pace`, `Seat.kick`,
`World.shudder`). A blow that lands at a rider's side, their own knight's
or the opponent's, jars the view wider for an instant and shakes it, the
more the nearer; within about two squares the world's own time slows to a
third for half a second, so the stone is seen breaking and hangs in the air
past the rider's eye (the rider's head keeps the clock's time, and can turn
while it hangs). A blow beside the knight ridden that is not on it rocks the
knight away from it and back, and the rider with it. The head is never
pulled to what is nearly underneath it (the prey of the knight as it comes
down on it), and tips only so far up and down, so the dive onto a capture
is ridden, not spun.

**Each piece its own stone** (`VOICES`, `Sound.chime`). Every sound the
world makes says whose it is, and each kind of piece is its own stone: a
pawn scrapes light and high, a rook is a grinding mass with its weight
under it, a bishop, a queen and a king ring like struck stone after they are
set down (a bell's note and the out-of-tune partial that makes it stone),
and a knight's hooves strike twice as it lands. The knight ridden breathes
out through its nostrils as it lands. Rendered offline in the page, a slide
and a landing, each: a pawn 0.18 s of sound at the highest pitch (5,600 zero
crossings a second) and the quietest (RMS 0.011); a knight 0.27 s; a bishop
0.72 s; a rook the loudest (0.039, three and a half times a pawn) and low
(2,300); a queen the longest ring (0.86 s); a king the lowest (2,200).

**The hall's own sound** (`Sound.wire`, `hallEcho`, `Sound.burn`). A room
of stone answers what is made in it: every placed sound also goes to the
hall, which gives it back off the dome and the wall 90 ms late, darker and
dying away over two and a half seconds, with the first reflections standing
out at its start. Rendered offline, a queen's crash two squares off: in its
first 0.6 s RMS 0.054 without the hall and 0.070 with it; from 0.6 to 2.5 s,
0.00003 without (silence) and 0.0051 with, about 23 dB under the crash, gone
by four seconds. The four braziers burn where they stand, each a low roar
of its own with a crack now and then, heard from wherever the seat is. A
sound is made on the audio context of where it is going, so a crack sent to
one context is never made on another.

**The hall's light follows the game** (`World.mood`, `dusk`, `spotlight`,
`blaze`). The room darkens as the board empties (the sky and the key light
fall by up to 38% and 30% as the pieces go from 32 to 4), so the braziers
and the shaft stand out more the further the game has gone. Each piece that
falls flares the fires. A king in check is picked out: the light from the
dome's eye narrows and brightens onto it, and a pale beam comes down through
the dusty air onto its square, until it is out of check. At a mate the fires
blaze and hold, and the hall brightens round the fallen king, until a new
game is set down.

**The hall** (`World.vault`). From a knight's back the room is what is
above the pieces, so it is built to be looked up at: a ring of twelve stone
columns round the board under a ribbed dome, a wall beyond them with tall
arched windows of night (stars, and the stone bars that hold the glass),
and a shaft of pale light from the dome's eye that leaves a cool pool on the
board, with the braziers' warm light round it. It is far enough out that the
seat at the board is inside the ring, with no column on the line from either
seat to the board. It is this page's own room, not anyone else's. Nothing in
it casts a shadow, and each part of the columns is drawn twelve at once: it
costs a quarter of a millisecond a frame.

**Peril** (`peril`, `World.menace`, `World.regard`, `Seat.glance`, `risks`).
The pieces that could take the knight ridden glow red and pulse, and lean
toward it as if about to come; a knight among them turns its head to it.
When the threat is new after the opponent's move it is said ("Your knight is
threatened by the bishop on b4") with a low drum, and the rider's head
snaps round to the piece and holds there a moment. A move after which the
knight ridden can be taken, by its own going or by what it uncovers, is
asked about first ("Your knight can be taken on g5, and you with it. Ride
on?"), aloud too: "Ride on" (or yes, Enter) plays it, "Hold your ground" (or
no, Escape) does not. Said again, it is the rider's choice and is not asked
twice.

**Thrown** (`Seat.thrown`). A knight taken under its rider breaks, and the
rider is thrown on the way the blow came, over the edge of the board,
tumbles, lies a moment looking up, and gets up at the side of the board,
eye level with its surface, the pieces towering over them. They command from
there; "Ride again" mounts their other knight if they have one, and a move
taken back puts them back in the saddle of the one they lost.

**Commanding your knight.** "Ride to f3", "gallop to e5", "charge d5" are
the ridden knight's moves (`command.js`, `RIDE`), and when two knights
could go where a command says, the one ridden is the one meant. A click on
the ridden knight's ears is a click on what is beyond them; with nothing
there it chooses (or lets go of) the knight itself.

## How it is built out

The page is built out a slice at a time, every 30 minutes (the operator's
loop). The structure that makes that converge rather than wander:

1. **One ledger.** This section is it: what the experience is still
   missing, ranked by how much nearer it brings the feeling of sitting on a
   knight. Each tick reads it, takes the top item that fits a tick, and
   rewrites it after.
2. **One slice per tick, shipped.** A slice is a thing the player can feel
   (not a refactor on its own): built on its own branch, played in a
   browser (Chrome for Testing on the shared Mac, never the real Chrome,
   which the live mirror's follower takes over), looked at in pictures, held
   by `beta/wizard_chess_test.py`, and merged. Work left in a worktree is
   not work done.
3. **Each slice in the lowest layer that can own it.** The world (what is
   there and how it moves) knows nothing of a laptop, so the headset version
   later is one more seat, not a second game. Riding is a seat; the cloth,
   the rider and the threatened pieces' light are the world.
4. **Measured, not guessed.** Frame time on this Mac's own GPU, the
   engine's Elo from its own scale, and what a test can see (where the
   camera is, which pieces glow) rather than "looks right".
5. **Played, every few slices.** A tick that builds nothing new: a game
   ridden against the engine, a picture after every exchange, and what
   breaks the feeling fixed. Slices are each looked at alone; a game puts
   them together, and finds what none of them showed (#5788).

Done: the rider's seat (#5747); the rider low and beside the head, and
sound where it happens (#5755); the knight ridden alive (#5757); peril felt
(#5758); seeing over your own knight, and the frame measured (#5760); the
hall (#5766); the pieces alive (#5768); the board hearing a command as it is
given (#5771); the opponent deliberating (#5772); riding into the blow
(#5774); each piece its own stone (#5775); the hall's own sound (#5776); the
hall's light following the game (#5777); riding any piece (#5778); the
rider's own hands (#5781); two players at one laptop (#5782); the headset
seat (#5784); the fallen and the commanding hand (#5785); a knight that
gathers and lands (#5786); a played game, and what it found (#5788).

What is next, most felt first:

- **Two players on two machines: the operator's choice first.** The moves
  have to travel between two pages, and every way of that has a cost the
  operator should choose: (a) peer to peer with no server, the two players
  pasting a code to each other, which reaches across the internet only with a
  STUN server (someone else's, such as Google's, which then sees both
  addresses) and otherwise works on one local network; (b) a small relay of
  this site's own, which is new infrastructure to run; (c) a public relay
  service, which sends every move to a third party. Until one is chosen, two
  players share one laptop.
- **The headset tried in a headset.** On a Quest's browser: comfort of the
  carried rig, the scale, the controllers' rays; then hands, and the panels
  in the world.
- **Speed: measured, and there is room.** `wizardChess.frameCost(n)` times
  `n` frames of the world's work and drawing, each finished on the graphics
  card (a pixel read back), and names what drew them. On the shared Mac
  (Apple M5 Max, through Metal, Chrome for Testing, 2026-11-24, the machine
  at a load of about 40) at a laptop's own pixel ratio (2880 by 1514): 2.1
  ms a frame at the board, 1.9 on a knight, 1.8 while a capture breaks the
  knight and throws the rider; at 1440 by 757, 1.3 to 1.4. That is eight
  times inside 60 frames a second and four inside 120, so nothing is cut;
  what is left over can buy the hall. With the hall: 2.3, 2.1 and 2.3. Animation runs on the world's time, 50
  ms at most a frame, so where frames are slow (CI draws in software) every
  move is slow too; a test waits on what it checks, not on a clock.
- **The mount seen whole.** Its ears and mane are above a rider's eye: a
  glance up, or a look back at it from beside the board while riding.
- **The knights as horses.** The pieces of the film-free set are busts; a
  rider's knight wants a body under them: a stone horse's neck and shoulders
  before the saddle, kept a chess knight from the board. (Its gathering and
  landing are done, #5786: the felt half first.)
- **The board between moves.** What a rider hears and sees while nobody
  moves: the pieces' small shifts, the fires, a far piece turning its head,
  so that the wait for the opponent is a wait among the living.

## How it is built

| File | What it is |
|------|------------|
| `beta/chess.html` | The page: the board's canvas and what is written at its edges |
| `beta/chess/game.js` | The world (hall, board, pieces, how a move is played out, the knight ridden), the seat (at the board, on a knight, thrown, aside; pointer, keyboard), the voice, the sound, and the game |
| `beta/chess/rules.js` | The rules of chess: legal moves, the position after one, check, mate and the draws, and a move written (SAN, UCI) and said aloud |
| `beta/chess/command.js` | A command, said or typed, read as a move, and the moves a command not yet finished could still mean |
| `beta/chess/engine.js` | The opponent: Stockfish in a worker, one move at a time at an Elo |
| `beta/chess/vendor/` | three.js r160 (MIT) and Stockfish.js 18, lite and single-threaded (GPL-3.0, its own license, kept beside it as `stockfish.LICENSE`), as published; its README names versions and sources |

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
piece fewer. And a rider: the eye is on the king's knight, "ride to f3"
carries it there, the pawn that can take it glows, the rider is thrown when
it does and stands at the side of the board, and a take back puts them in
the saddle again. On this Mac run it with Chrome for Testing (the real
Chrome's tab is taken over by the live mirror's follower):

    python3 -c 'import runpy,sys; sys.path.insert(0,"beta"); import verify; verify.CHROME_CANDIDATES=[sys.argv[1]]; sys.argv=["wizard_chess_test.py"]; runpy.run_path("beta/wizard_chess_test.py", run_name="__main__")' "<Chrome for Testing binary>"

Measured in headless Chrome on a MacBook Air (M4), the page served from a
local directory with its files already cached: the board is on screen about
0.15 s after the page is asked for and the engine is awake at about 0.2 s
(three runs: 0.20, 0.18, 0.17 s). Over a network the engine is a 7.3 MB
file and wakes when it has arrived; the board does not wait for it. The
opponent is given 0.9 s to think about each move.

## The headset version

Built (#5784), and not yet tried in a headset: nothing on the shared Mac
can open an immersive session, so what is checked is what a browser without
one can show (below). It is what the world was kept apart for.

- **"Enter VR"**, by the seat buttons, is shown only where the browser has an
  immersive headset to offer (`navigator.xr.isSessionSupported`), and opens
  one session (`immersive-vr`, a `local` reference space: the wearer's head
  where the seat puts them).
- **The seat in a headset** (`Seat.frame`, `headsetPlace`). The camera stands
  in a rig, and in a headset it is the rig that is placed: at the board, the
  wearer stands at their side's edge of it, a little above it, a square a
  metre wide and the king two metres tall; on a piece, at its rider's eye,
  carried as it goes and turned the way it faces, but kept level; thrown or
  beside the board, where they are. Their own head does all the looking. What
  a body does not feel an eye in a headset is not shown: no travel of the
  view between seats, no shake, no widening, no head turned for them.
- **Pointing** (`Seat.pointed`, `pick`). Each hand's controller draws a gold
  ray, and its trigger is a click: a piece, a square, a capture, by the same
  rule as the mouse (`meant`), checked to pick exactly what a click on the
  same line of sight picks.
- **The loop** is the renderer's (`setAnimationLoop`), which a headset drives
  when one is on and the window does otherwise. Sound is heard from the eye's
  place in the world, inside the rig or not.

Not built: hands seen as hands (the controllers are rays), a keyboard (a
headset's own, or speech where its browser has it), and the panels inside
the world rather than on a screen that is not there.

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
  two players play at one laptop only (not yet over a connection).
- Nothing weaker than Elo 1320 (see above).
- Not built for a phone: it works by touch, but the layout is a laptop's.
