# Sim debugger

The Sim debugger is the played game (`docs/SINGLE_PLAYER.md`) with a
debugger's controls on it. Playing every decision by hand is its lowest
level; everything here is a layer of automation above that, or a way to move
through the game's own history.

The engine half is `src/server/debugger.rs`, served as `POST /debug` by the
native server and the browser build alike. The page half is
`web/assets/app_debugger.js`: a header box at the top of the window beside
the left panel, with the map area beneath it.

The engine half is held by `src/server/debugger_tests.rs`. The box is held by
`beta/debugger_test.py`, which loads the script as it ships into a real
browser, on a page with a left panel and a toy engine behind it, and drives
the controls: where the box stands (beside the left panel, the map area under
it), the timeline on top with turn 1 at one end and the flag on the last turn
at the other, the open turn in the middle under it and the turn buttons in
their order, the marker moving right as turns open, the turn box, the
timeline back to a played turn and on to one ahead, what it says under the
pointer, the wheel, the turn buttons (one and ten, back and on, turn 1), the
eras named on the timeline, Era Back, Era Next and Play to End, save points
and their flags, a
breakpoint picked from the timeline, a timeline with turns thinned out of it,
the Civ AI picker, Auto-play with its count and its Stop, the breakpoints
form with auto-play stopped by a turn, by a new era and by hand, layers,
reveal, the Single player switch, undo, and a round stepped a seat at a time.
It walks a game back, picks another Civ AI and goes on by each way there is
(the turn buttons, the turn box, the timeline, Auto-play), and reads that the
pick was said to the engine at once and that every one of them played, with
the Civ AI picked, where the turns ahead were another one's.
It then splits the screen head to head, each half the same page with a toy
engine of its own: a Civ AI is picked for each half, from the strip and from
the end of its branch, both are played to a turn locked and then unlocked,
each half and then both are moved by the sliders in the host's box, and the
test reads which Civ AI each half's steps named, the numbers set side by
side (open as the split opens), a copy, and a branch dropped by its name. It
reads the timeline as it is
drawn at each step: one rail up to the turn of the split and a branch for
each from there, from the moment it opens, whether the two halves hold the
same worlds or not, a copy included. A part holds the lock: that
Branch opens the split at once, that the lock stands at the root of the
tree and closes only on one turn, that the host's deck is the one panel of a
locked split and plays both halves, that each half shows its own panel again
when the lock opens, that two identical halves are asked for once, that a
half playing ahead by itself reaches further along its branch, and that the
timeline shows each half's breakpoint turns and save points and goes to the
one that is clicked. A last
part holds the branch: that Branch stands under the marker and moves with
it, that a branch made behind the last turn played forks the timeline on
that turn with the branch holding nothing past it, that the tree grows from
the left with nothing of either branch left of the trunk, that each branch is
named on itself and ends in its letter, its picker and its ✕, that closing
the timeline as it
was leaves the branch as the timeline, and that each stretch is named by the
Civ AI that played it. Two more parts hold what the box asks (Switch or
Branch for another Civ AI picked on the one timeline, and which Civ AI plays
the branch that Branch is about to make), and one that the timeline says
Click to travel until it is clicked to travel. The next holds the opening
view: a page opened as anyone opens it draws its first state on one timeline
with the top-rated Civ AI in its seat, nothing pressed and nothing asked,
and a page that asks for two tracks (`?tracks=2`) opens on them, once a
game. Every part before that one asks for the one timeline by name
(`?tracks=1`). The last part holds Autosearch (see
"Autosearch"): the button's size and place, the panel's choices, a search
played on copies of the toy engine with the page's own game left alone, the
copies let go when it ends or is stopped, an engine that cannot be copied,
the line played on the one timeline and in the other half of a split, and
Autosearch pressed on a half's own panel. A control added to the box or the
strip is added there.

## The bar

The debugger's controls are one header box at the top of the window. It
begins at the right side of the left panel and runs to the window's right
edge, so the left panel keeps the window's whole height. The map area stands
beneath the box: where the map area begins and how tall the box is are
measured and published as `--dbg-left` and `--dbg-top`, and the map is fitted
again whenever either changes (the left panel folded away, brought back or
dragged wider). A watched game has no box and is not moved.

Above the timeline is the Civ AI that plays the seat from the open turn, to
pick there. A page's first game opens with the best one there is in it, the
one with the highest Elo ("The opening view", under "Split screen"). Picking
another one there asks first what to do with it
(operator 2026-11-22: "leading strategies should be editable and when edited
to a different strategy, should ask to switch or branch"): **Switch**, and
this timeline plays on with it from the open turn, or **Branch**, and a
second timeline begins there with it while this one keeps the Civ AI it had.
Cancel, Escape or a press anywhere else leaves the seat as it was. Until it
is answered nothing is said to the engine. (The left panel's own picker, in
the Single player view, seats its pick at once as before.) Under the
timeline's marker, wherever the marker stands, is
Branch (operator 2026-11-21: "branch should say "Branch" and should move
around below where the timeline "cursor" is. always rpesenting the option to
branch"); it keeps its word in a box of any width. Under that is one row in
three parts.

In the middle, under the middle of the timeline, is the turn that is open, in
a box that also goes to a turn typed into it, with the game's last turn
beside it. The turn buttons stand either side of it, in this order:

- **Turn 1** goes to where the timeline begins;
- **◀◀ Era** (Era Back) goes back to the first turn of the era that is open,
  and from there to the first turn of the era before;
- **◀◀ 10** and **◀ 1** back;
- the turn box;
- **1 ▶** and **10 ▶▶** on;
- **Era ▶▶** (Era Next) goes on to the first turn of the next era: one the
  turns already played reach is gone to, and past the last of them the Civ AI
  plays until a new world era begins. It is off in the last era;
- **Play to End** plays the game out.

The buttons that go on walk the turns already played, and past the last of
them the seat's Civ AI plays on. They do not play over what was played: ten
on from turn 3 with nine turns played walks to turn 9 and plays four more.
(Auto-play from an earlier turn is the way to play those turns again.) Turns
that another Civ AI played are not walked to at all: see "Whose turns are
walked". Whatever they play, a breakpoint stops early and Auto-play's button
is the Stop.

At the left end is Auto-play, which used to be in the left panel: the Civ AI
that plays the seat, how many turns (1 to 250, or all; it is set to 10 to
begin with), and the button. It plays that many turns from the turn that is
open and stops early when a breakpoint does. While it plays, the button
counts the turns (`7/20`, or just `7` when it plays all) and is its own Stop.
A turn played to from the timeline or by the turn buttons is counted and
stopped the same way. Beside it are Layers and Breakpoints, and after them
**Autosearch**, a size larger than the tools beside it (operator 2026-11-22:
"make it a bit bigger than the others"): see "Autosearch". Branch (the split
screen) is under the timeline's marker.

At the right end are the rest of the tools: step in, step a seat, undo, save
points, Reveal and the Single player switch.

The words on the tools are shown while both ends fit in two rows. When they
do not, the words on Turn 1 and Play to End go first (⏮ and ⏭) and then every tool's,
leaving the signs. In a box 720px wide or narrower the three parts stack, the
turn buttons first. What the box has to say (why auto-play stopped, what the
Civ AI just did) stands in the box, between the turn buttons and the tools at
the right; a stacked box says it under itself.

The timeline is the whole game, twice the height of a tool row, and bright:
sky behind and the turns played in sun across it. Turn 1 is named at its left
end. Its right end is the game's last turn under a chequered flag, or turn
250 with no flag when the game does not say which turn is its last. A game
played on past its last turn grows 50 turns at a time, and the flag is then a
chequered line across the rail where the last turn was. The round turns
between the ends are numbered along the top of the rail. On it:

- a sunlit band across the middle of the rail over the turns that were
  played. When turns have been thinned to make room the band pales and the
  turns still kept stand out on it;
- the eras the game has been through, each named at the foot of the rail on
  the first turn it has of that era, with a dotted line up the rail. A name
  is clicked to go to that turn. They come from the engine
  (`state.debug.eras`: the first kept turn of each world era the marks pass
  through), so an era whose first turn was thinned away is named on the
  nearest kept turn after it;
- the open turn, as a pill with its number, which moves right as turns open.
  Dragging it, or clicking anywhere on the timeline, goes to that turn: a turn
  that was played is put back as it stood, and a turn still ahead is played to
  by the seat's Civ AI from the turn that is open. While it plays, the stretch
  still to go is hatched;
- until the timeline has been clicked to travel, the words **Click to travel**
  on it, large, yellow and calling with a ring, in the open rail ahead of the
  marker (behind it once the marker is past 60% of the rail), and only while a
  click would travel: not while turns are being played (operator 2026-11-22:
  "have the initial timeline say click to travel so people understand the
  mechanism. make the click to travel very visible at the start and then not
  appear once they understand"). They take no press: a click on them is the
  timeline's, under them. The first slider let go of on another turn, or mark
  on the rail clicked (an era, a breakpoint turn, a save point), makes them go,
  and the browser keeps that (`civvis-debug-click-to-travel` in its local
  storage), so they are not said again, in that game or any later one. Going
  to a turn any other way (the turn box, the turn buttons, the Save points
  panel) leaves them;
- save points as flags along the top edge and turn breakpoints as red marks
  along the bottom, each to click;
- under the pointer, the turn there and what letting go would do: "play to
  here", "play on from turn 7 to here" when the open turn is behind the last
  one played (the turns after it are played over), or the turn a thinned one
  would land on.

### Panels are windows

Layers, Breakpoints, Save points and Autosearch each open a panel under
their button, and what the box asks opens under what it is about (Switch or
Branch under the picker, and which Civ AI plays a branch under Branch). Each
is a window, and a window
is moved
by its title (operator 2026-11-22: "clicking and dragging on a window title
allows for repositioning"): press the title, drag, and the window goes where
the pointer takes it, over the map or over either half of a split
(`debugMoveWindows`). The rest of a panel is not a handle: its fields and
buttons take their own presses.

A window stays where it was put while the page is open: it opens there
again, and is still there when the box changes shape (the place is kept as
the window's corner in the browser window, `debugWindowPlace`). Each panel
has its own place, and so has each of the two questions. It cannot be taken
out of the browser window, nor far enough down that its title is out of
reach. A double click on the title puts it back under what it opens from.

The windows over the map (standings, victory tracker, world minimap,
dossiers) are the viewer's own and were already moved by their title bars
(`[data-widget-drag]` in `web/assets/app.js`), on one screen and in each
half.

## Two views

A game under the debugger is shown one of two ways, and the bar's **Single
player** switch goes between them. The choice is this browser's, kept in
`civvis-debugger-view-v1`.

**Debugger** is how it opens, and it is the view a watched game has: the map,
the standings across the top, the victory tracker down the right, and a box
with the seat's governors, research, civic, government and policy cards (the
box a mirrored Civilization VI game shows, without that game's two buttons).
Nothing of the played seat's own screen is on it: no yield strip, launch bar,
world tracker, notification rail, unit panel or End Turn. It is for reading
the game while the bar plays it, so nothing is selected, a press on a unit or
a city reads it as it does for a watcher, the keys that play the seat do
nothing, and a turn's banners stay off the map.

**Single player** is the Civilization VI screen of `docs/SINGLE_PLAYER.md`,
for playing the seat by hand. Switching to it picks up the next unit waiting
for orders; switching back puts the selection and any open city, empire,
diplomacy or trade screen away.

The view is the layout and nothing else. Whose eyes the map is drawn through
is Reveal's, in either view, and the turn plate's POV line says which: Single
Player through the seat's fog, Global with Reveal on.

In the page the switch is `SOLO_UI`, read by `civ6Frame()` and
`debuggerView()` in `web/assets/app.js`; a held seat with no debugger behind
it (`state.debug` absent) has no switch and keeps the Civilization VI screen.
`setSoloUi` in `web/assets/app_debugger.js` changes it.

## The timeline

A played session marks the world at the top of every turn its seat opens. A
mark is a clone of the game; the game shares whatever a turn did not change
with its clones, so a mark costs what its turn changed and not the whole
world. That is still megabytes (the figures are below). Every turn keeps its
mark for as long as there is room, so any turn that was played can be put back
on the table as it stood, and going there takes about as long as drawing it.

Room is 480 marks, and in a browser tab also a line on the memory in use
(1 GB unless the page names another with `room`), because a tab's memory only
grows. A mark was measured at 1.3 MB in the first 60 turns of a six-seat game
and 2.8 MB by turn 130. When room runs out the first mark and the newest 60
stay whole (the newest half of the room when that is fewer), and between them
the oldest turns go first, the odd ones before the even: a very long game
keeps its early turns one in two, then one in four, and so on to one in 64. If
that is still too many the oldest go, so the room is never exceeded.

Past the memory line, each time the memory has grown by another 1/32 of the
line (32 MB at 1 GB), a fifth of the room is taken away, down to 12 marks.
There is no gentler floor on purpose. With the line at 256 MB and the room
never falling below the newest 60 turns, a six-seat game went from 248 MB at
turn 110 to 425 MB at turn 200 and was still growing by 2 MB a turn, because
late marks are larger than the early ones they replace.

A thinned turn is not played again to bring it back. A turn played by hand
cannot be played by an agent, and an agent decides things its logged actions
do not carry, so the replay would be another world.

| Control | Operation | What it does |
|---------|-----------|--------------|
| Turn 1 | `jump` to the first mark | Go to where the timeline begins |
| ◀◀ Era (Era Back) | `jump` | Back to the first turn of the era that is open, and from there to the first turn of the era before |
| ◀◀ 10 | `jump` | Back ten turns, or to where the timeline begins |
| ◀ 1 | `back` | The top of this turn if you have acted in it, otherwise the turn before |
| turn box | `jump`, or `walk` then `step` repeated | Go to a turn by its number, the same way. Up and down in the box walk one played turn. Its tooltip says which turns were played and how many are still kept |
| 1 ▶ | `forward`, or `step` | Next turn: forward along the turns already played, and past the last of them the seat's Civ AI plays one whole turn |
| 10 ▶▶ | `jump`, then `step` repeated | Ten turns on: forward along the turns already played, and past the last of them the Civ AI plays the rest |
| Era ▶▶ (Era Next) | `jump`, then `step` repeated | On to the first turn of the next era. One the turns already played reach is gone to; otherwise the walk goes to the last turn played and the Civ AI plays until the state's `world_era` changes (the page stops the run itself, as the "A new era begins" breakpoint would). Off in the last era |
| Play to End (⏭) | `jump`, then `step` repeated | Along the turns already played to the last of them, then the Civ AI plays until the game is over, stopping early at a breakpoint |
| timeline | `jump`, or `walk` then `step` repeated | Go to any turn of the game. One that was played is put back as it stood; one that is no longer kept answers with the nearest before it, and the bar says so. One still ahead is played to by the seat's Civ AI, stopping early if a breakpoint does. The mouse wheel over it walks one played turn at a time, and the arrow keys move it a turn |
| Civ AI | | Which Civ AI plays the seat when a turn is auto-played, stepped or played to. It is the page's own picker (`#autoplaystrategy`), shown above the timeline, from where the rail begins. Each Civ AI reads `<Elo> - <name>`, the highest Elo first, and the box is as wide as its longest listing, so the Elo and the whole name are read without opening it |
| Turns, ▶▶ Auto-play | `walk`, then `step` repeated | Play the turns picked (1 to 250, or all; 10 unless changed) from the turn that is open, stopping early at a breakpoint. Turns among them that the Civ AI already played from here are taken as they are kept and not played again (see below). The button counts the turns as they are played and is the Stop. Each turn is asked for the moment the one before it is answered, so the engine plays it while the page draws that one; a Stop pressed while a turn is being drawn lands a turn later. In the browser build the request is handed to the engine's worker inside the `fetch` call itself (`beta/shim.js`): handed over in a `then`, it waited for the draw that follows it in the same task, 18 to 30 ms a turn |
| ↶ (`Ctrl`/`Cmd`+`Z`) | `undo` | Take back your last action this turn; the note says so, and how many more can be taken back |
| ▸ | `step_action` | Step in: the seat's Civ AI takes its next action, and ends the turn when it has none left |
| ▸\| | `step_seat` | Step one seat: end your turn as it stands, then one civilization's turn at a time |
| ◉ | `reveal` | Show the whole map and every civilization, not only what the seat can see |
| ⚑ | `save_point`, `goto_save`, `drop_save` | Save points: keep this world to come back to, go to one, or let one go. Each stands on the timeline as a flag at its turn; clicking the flag goes there |
| ⚑ | `POST /save`, `POST /load` | Saves, in the same panel: keep this turn on the device under a name, or load one that was kept |
| ⑂ Branch | `branch`, in the new half | A second timeline from this turn, side by side with this one and locked on the same turn. It hangs under the timeline's marker and moves with it; see Split screen below |

### Whose play each stretch was

Each stretch of the turns played is named on the timeline by the Civ AI
that played it (operator 2026-11-21: "lets label ai strats on the timeline
too, to make the branching more clear"). The name stands at the start of its
stretch, every other stretch is a shade darker, and a line stands on the
turn the seat passed from one Civ AI to the next. A stretch the person
played reads "By hand". A name is cut short at the end of its stretch,
except the last, which has the rail ahead of it to run on over; a name the
marker stands on steps past the marker, and in a split's rows past an era's
name as well. The words under the pointer say whose play opened the turn
there.

A branch of a split is named on itself (operator 2026-11-22: "lets label the
branches themselves"): the name on a branch's first stretch in its own row
begins with the branch's letter, "A · JackOfAllTrades". A stretch that is
all on the branch's slant from the trunk to its row, where no word is drawn,
is named nowhere. A branch nothing has been played on yet is named by the
Civ AI seated in it, the one that will play it.

The engine says it in `debug.played_by`: for every kept turn, the ones ahead
that another Civ AI played too, a list of `from`, `to` and `by`. A stretch
begins on the turn its first turn was played from, so each begins where the
one before it ends. The turns ahead that are not on the seat's line keep
their grey dashes and are named the same way.

Going back forgets nothing. The turns ahead stay marked, and 1 ▶ walks them
again exactly as they were played. They are replaced only when the world is
played forward differently from an earlier turn: by ending a turn by hand, or
by a step.

Undo replays the turn. It puts the turn's mark back and applies every action
the seat took except the last. The engine is deterministic and the random
stream is part of the game, so the replay is the same world.

### Whose turns are walked

A turn ahead is walked to only under the Civ AI that played it. Each mark
remembers the Civ AI whose play opened its turn, and what the engine gives as
the seat's timeline (`turns`, `last`, `can_forward`) is every turn up to the
open one and, ahead of it, the turns the Civ AI now seated played. With
another one seated, the kept turns ahead are that other agent's game and not
what this one would do from here. They are not walked to by 1 ▶, 10 ▶▶, Era ▶▶, Play to End, the
turn box, the timeline or the wheel, `forward` and a `jump` onto them are
refused, and every way forward plays instead, with the Civ AI that is picked.
The page says the pick to the engine when it is made (`status` with
`strategy`), so this holds from the pick and not first from the next step.

The turns are still kept. The timeline draws them apart, as a grey dashed
stretch past the gold; the note says what going on does and whose they are
("SiegeTheDay plays on from here · turns 11 to 41 are by JackOfAllTrades");
and picking
their Civ AI again before they are played over makes them the timeline again.
The first turn played from there replaces them, as it replaces any turns
ahead. It makes no difference whether the Civ AI was picked after the walk
back or before it. A turn the person ended by hand is nobody else's, and is
walked to under any pick. A turn being stepped in is planned again when the
seat changes hands in the middle of it.

Why: on 2026-11-21 the operator walked a game back, picked another Civ AI and
went on, and was shown the first one's turns again. Auto-play did play from
there with the new pick. The turn buttons, the turn box and the timeline
walked whatever was kept ahead, and nothing asked whose turns those were.

### Turns that are not played again

A run does not play a turn the engine already holds. Played 50 turns on,
walked 20 back and played 30 on, the first 20 of the 30 are kept turns: the
run goes to the last of them in one request and the Civ AI plays ten.

Every run of the page (Auto-play, the turn box and the timeline past the last
turn played, a breakpoint picked on the timeline) first sends `walk`, with
the turn it is headed for. The engine puts the latest kept turn on the way on
the table and plays nothing; the run's `step`s go on from there. On the way
it asks the breakpoints of every kept turn it passes, each against the turn
before it as a step does, so the walk stops on the turn, and for the reason,
that the run which played those turns would have stopped. `"era": true`
stops it on the first turn of a new world era too, for a run that plays
until one begins.

A turn is taken from its mark only where playing it again would arrive at
the same world, and `debug.played_ahead` is the last such turn ahead (null
when there is none, and then the page sends no `walk`):

- the open turn stands as it opened: nothing done in it by hand or by a
  layer, and not in the middle of a round;
- the agents kept with it are the ones to sit down again (a half of a split
  keeps none and so walks nothing), and the rivals are seated as they were;
- each turn from here to there was played by the Civ AI now seated, from its
  top to its end. A turn the person ended, a turn that was touched before the
  agent took it, and another Civ AI's turn are where the walk ends: the
  seated agent's play from there is another game, and it is played;
- no turn between has been thinned away.

Where the walk ends short of the run's turn, the first turn played replaces
the kept turns past it, as it always has. The turn buttons are as they were:
they walk every turn of the seat's line, hand-ended ones included, and play
only past the last of them.

Why: on 2026-11-21 the operator played 50 turns on, went 20 back and played
30 on, and the 20 kept turns were played a second time: "we already have the
saves. take the latest save on the path and start from there". Auto-play,
the turn box and the timeline asked for a `step` a turn from the open turn,
and the first step let go of every turn kept ahead.

A walked-to world is played on by the agents that were playing it. An agent
keeps plans from turn to turn (where its Settlers are headed, who it means to
fight), so the world alone is not all of a game. Each mark and each save point
therefore keeps the agents as they sat at that moment, every seat's, each a
copy with what it remembered then. Putting the world back seats copies of
those. The agents sitting at the time of the walk are not used: their plans
belong to the future the world was taken out of. So a turn walked back to and
played again with the same Civ AI is the turn that was played, and so is every
turn after it; stepping into a turn that was walked to shows what the agent
did there, not what a newcomer would do.

Two things seat a new agent instead. A Civ AI picked for the seat since the
turn was marked plays from there as itself (the kept agent was another's), and
rivals seated with another Civ AI since then are seated anew likewise. A game
that was loaded from a save starts with new agents, because a save carries the
world and not the agents.

Why: walked on civvis.ai/test on 2026-11-21 (build `2e641775a`, four seats,
TouristTrap in the seat). A game played from turn 10 to turn 40, walked back
to turn 10 and played to turn 40 again parted from the first on turn 14. Walked
back to turn 25 of the second and played on, it parted on turn 26, the very
next turn. Two walks back to the same turn agreed with each other turn for
turn, so nothing but the agents' memory was missing. Until then a walked-to
world was given new agents, as a loaded save is.

The page drops its own standing orders (a unit's journey, a city's queue)
when a world is walked to: those belong to the future the world was taken out
of, and a turn that was walked to is shown as it stood.

A save carries the world and not the timeline. A loaded game starts a
timeline of its own from the turn it resumes.

## Save points

A save point keeps the world under a name, as it stands: part-way through a
turn or not. It is kept beside the timeline and not on it, so it is still
there after the game has been played on differently from an earlier turn, and
it costs what a mark does. A timeline holds 32 at once. A name already taken
is given to the newer world, and one with no name is called by its turn.

Going to a save point puts its world on the table with the turns of its own
history: the marks it was played on from, the top of the turn it was taken in
(so undo works there), and whatever was played on from it. Marks of another
history are let go, because a timeline is one history. The world that was
left is kept as a save point called "Where you were", unless it is a save
point already, so going back is the same operation.

In the bar they are under ⚑: a name and "Save point" to set one, and a list
to go to one or let it go. Each also stands on the timeline as a flag at its
turn, gold for one that was set and grey for "Where you were".

Save points last as long as the page is open. What is to be kept longer is a
save, in the same panel: it is written to the device, and carries the world
and not the timeline.

Save points live with the timeline: both halves of a split start with the
ones there were, a save file does not carry them, and a new or loaded game
starts with none.

## Stepping in

Step in takes the agent's turn apart. The agent plays the whole turn once,
on a copy of the world, and each press applies the next action of that plan
to the real one; the bar names it and the map centres on it. When the plan is
spent, the next press puts the agent's own copy on the table and ends the
turn there, and the rivals play.

The plan is kept between presses for as long as nothing else touches the
world. An action by hand, an undo or a layer makes it stale, and the next
press plans again from the world as it then stands. An action the real world
refuses, because the plan was made before something it leaned on changed, is
passed over. Each step is an ordinary action of the seat's, so undo takes it
back.

Taken apart or taken whole, it is the same turn, because the last step ends
on the world the agent itself played: the test
`stepping_in_takes_the_agents_turn_one_action_at_a_time` plays one both ways
and compares the worlds. An agent also decides things that are not actions,
its standing preferences for the governors of its cities. Those are carried
onto the real world when the plan is made, the way the agent's own
plan-then-replay path carries them, so a turn the person takes over halfway
keeps them.

## Stepping a seat

Step in takes the played seat's turn apart; step one seat takes the round
apart. On your own turn it ends the turn as it stands and stops there, before
any rival has moved. After that each press plays whoever is up, a rival, a
city-state or the barbarians, with that seat's own agent, and stops again.
The bar says who played, how many actions they took and who is next. With
Reveal on, each of those turns can be watched on the map.

When the round comes back to the played seat, its next turn opens exactly as
it does after an ordinary end of turn: the turn is marked and the layers that
are on take their share. A round taken apart is the same round as one taken
whole, and a test plays one both ways and compares the worlds.

In the middle of a round the turn is not the person's (`state.debug.playing`
is the seat that is up):

- 1 ▶ (`step`) lets the rest of the round play, and holds the turn that opens
  to the breakpoints like any other. End Turn does the same (`finish_round`).
- ▸ (`step_action`) steps the next seat, which is the smallest step there is.
- ◀ goes to the top of the turn the round belongs to.
- A rival's turn is not the seat's action to take back, and not a place to
  keep: undo and save points are refused until the round finishes.

In a locked split the other half steps the same seat, and follows when the
turn changes, as it does for an ended turn.

## Reveal

Reveal changes what the page is shown, and nothing else. The state document
is built from the omniscient observation, the one a spectator of the whole
table is given, with the seat's own reach to act with kept on it. The seat's
legal actions are the same either way, and the world is not touched by
looking. A world that is walked to, and a fork, stay revealed.

In a split this is the second point of view: one half played through the
seat's fog, the other with the fog lifted.

## Layers

Between playing by hand and handing the seat over there are five layers, and
each can be given to the seat's Civ AI separately:

| Layer | Decisions |
|-------|-----------|
| `research` | Research and civics |
| `cities` | Production, purchases, a captured city's fate |
| `empire` | Government, policies, dedications, pantheon and beliefs, governors, Great People |
| `diplomacy` | War and peace, deals, delegations, the World Congress, envoys, spies |
| `units` | Everything done by something standing on the map |

The agent plans the whole turn on a copy of the world. Its actions are then
offered to the real world in the order it took them, and only those in a layer
that is on are applied. One that no longer applies, because it leaned on a
decision left to the person, is dropped. Ending the turn belongs to no layer.
With the cities layer on, the agent's standing preferences for its governors
are carried across too; they are decisions, though not actions.

Layers that are on are played when one of your turns opens by being played
into. "Apply to this turn" (`assist`) plays them in the turn that is open
now. A turn that was walked to is left as it stood.

## Breakpoints

Auto-play, and a turn played to, stop by themselves when:

- one of the listed turns opens;
- the turn that opened is the first of a new world era, if "A new era
  begins" is on (`eras`);
- the turn added an event in a chosen category (`EVENT_CATEGORIES`);
- the turn added an event whose text holds a chosen phrase;
- the turn added an event the engine marks important, if that is on;
- a condition on the state of the game became true;
- the game ends or the seat is eliminated.

Only the played seat's own events are read, which is what a person playing
it is shown. "Every civilization's events" reads the others too.

A condition reads one thing the observation says about one civilization and
holds it to a line: `seat`, `key`, `test` (`<`, `<=`, `>`, `>=`, `==`, `!=` or
`changes`) and `value`. The reading is that civilization's entry in the
omniscient observation (`players[seat][key]`), so whatever a spectator of the
whole table is told about a civilization can be a condition, for any seat:
`cities`, `gold`, `score`, `population`, `unit_count`, `wonder_count` and the
rest. A number is read as itself, a yes or no as 1 or 0, and a list as its
length; `changes` takes any reading at all. A key the observation does not
have is refused when the condition is set, not left to never fire.

A condition stops the run on the turn it becomes true: it holds now and did
not a turn ago. One that stays true does not stop every turn after.

Reading the conditions is building that observation, which is the whole
table, so a step with a condition set pays for it: 60 to 80 ms of a 150 ms
step at turns 80 to 105 of a six-seat game when a step read it before and
after. A step now keeps what it read after, and the next step uses that as
its reading before when the world is still that world (the same log, action
for action) and the conditions are the same; any other debugger operation
lets go of it. A run with conditions set reads the table once a turn.

The stop is reported on `state.debug.stopped` and in the bar, with a Show
button when the event has a place on the map.

A listed turn stands on the timeline as a red mark. Clicking it goes to that
turn: back to it if it was played, and on to it if it is still ahead.

## Split screen

Branch puts two simulators in one window. It asks one thing first, which
Civ AI plays the branch (below), and nothing about how the two are laid out:
when a Civ AI is picked the screen splits at once, side by side, and the two
halves begin locked on the same turn (the strip between them can stack them
instead). Both start from the world as it stood when the screen was split,
and from there each can play its own history: play one by hand and step the
other, or try two answers to the same turn.

**The opening view.** A game opens on its one timeline, with one Civ AI
(operator 2026-11-24: "ai debugger should start with 1 strategy (maybe the
best strategy). players can branch if they want (they should choose the new
strategy when they branch)"). The Civ AI is the best there is: the one with
the highest Elo anything has measured, which is the first the picker lists
(`debugBestAgent`). It is seated once a page, for the page's first game,
whatever that browser last picked, and the engine is told at once; after
that the seat is whoever is picked for it, through every new game on the
page. A page whose Civ AIs nothing has rated keeps the seat it had. A second
timeline is the person's to make, with Branch.

From 2026-11-22 until then every game opened on two tracks (operator: "in ai
debugger, the opening view should go to the 2 track immediately"), and a
page asked for with `tracks=2` in its address still does: the first state of
a game drawn under the debugger is branched where it stands, as Branch does
it, with nothing pressed and nothing asked, both tracks on the one Civ AI.
That is given once a game (`debugOpenTheGame`, by the game's seed): a branch
that is dropped leaves the one timeline, which stays one until Branch makes
another or a new game begins. Doctor Strange's CIVVIS is two worlds and
opens on them, unless it is asked for with `tracks=1`. A window under 900px
wide, a small device, and a seat being played by hand in the Single player
view open on the one timeline whatever was asked.

**Branch asks which Civ AI plays the branch**, and makes the branch when one
is picked (operator 2026-11-24: "they should choose the new strategy when
they branch"). Pressed, it opens a list under itself of every Civ AI the
page offers, as the picker lists them, Elo first and the best at the top,
with the one this timeline has marked "plays this timeline". A press on one
makes the branch for it in one step: B begins seated with it, and A keeps
its own. The timeline's own Civ AI may be picked too, and then the two are
one world until one of them is played another way, and still two branches. Nothing is done until
one is picked: Cancel, Escape, a press anywhere else, or Branch pressed
again leaves the one timeline as it was, and nothing is said to the engine.
A branch made from the Switch or Branch question above was made for a Civ
AI the same way. Two screens are asked nothing: each branch's picker, at the
end of its branch, seats another Civ AI in it. (From 2026-11-22 until
2026-11-24 Branch split the screen first, with B on A's Civ AI, and asked
afterwards.) A game that opens on two tracks is asked nothing either, since
nobody pressed Branch for it (operator 2026-11-22: "branch b should only ask
when branch is selected. not just auto ask rom the start"): it opens with B
on A's Civ AI, and B's picker seats another.

A is the timeline as it was, and B is the branch. A keeps every turn it
holds, the ones past the open turn included. B begins on the open turn: when
turns are kept past it, the host tells B so as it loads, and B lets go of
them (`branch`). A branch made on the last turn played has nothing to let
go of and is asked nothing, so the two stay twins ("Identical halves are
played once"). Two branches are the most there are: while the screen is
split there is no Branch, and closing one brings it back.

Each branch has an ✕ after the right end of its row (operator 2026-11-21:
"each branch should have an X"; 2026-11-22: "move x deletes over to the
right side please, right after each closable timeline ends"), between the
row's rail and the game's last turn. It closes that branch, and the other is
the timeline from then on with every turn it holds (`adopt`, the same as
Drop A and Drop B on the strip). Closing B leaves the timeline
as it was. Closing A leaves the branch: one timeline that passes from the
Civ AI that played up to the fork to the one that played the branch
(operator: "if the initial is deleted though, the branchbecomes the main
tree, transitioning from one ai strat to another"), each stretch named.

Each half is the whole page again in a frame, and both are answered by one
engine, which keeps a session for each (`src/server/panes.rs`). A half's
session is a fork of the main one (`Session::fork`): the same world, the same
agents, and the timeline behind it. So a half can step back to turns played
before the screen was split, and nothing is lost when the halves change
places.

A half names itself in its address (`?pane=a-<split>`):

- On a socket server every request its page makes carries that in `Referer`,
  with the page doing nothing, and the server answers it from the half's
  session.
- On the published build the half's shim starts no engine. It hands each
  request to the engine of the page that framed it, with the half's name on
  it (`beta/shim.js`, `pane-ask` / `pane-answer`), and the engine's router
  answers from the half's session exactly as the socket server does.

Only requests that read or change a world are answered from a half's
session. A new split replaces the sessions of the one before it.

The bar of the page that was split stays across the top of the window, over
both halves, and its timeline is theirs. It has two rows, A's over B's, with
the Civ AI each half seats at the head of its row, to pick there. Each half
has a slider the length of the game in its row. A slider is the hand on its
half, and on both halves while they are locked. Letting go of it on a turn
every half concerned has played walks them there; on a turn ahead of any of
them it plays them there, each with its own Civ AI. A half is told which Civ
AI holds its seat when it is picked, and says so to its engine at once. Kept
turns ahead that another Civ AI played are drawn apart in the half's row as
they are on an unsplit timeline ("Whose turns are walked"), and a locked
half that is asked to follow the other onto such a turn plays to it with its
own Civ AI. The halves of a new split start on the Civ AI picked in the box,
and a half that is kept keeps its own.

### One screen and two change places in one move

A half is a whole page in a frame and takes about a second to be there: it
loads, folds its left panel away, draws its first state, and tells the host
where it stands, what is on its timeline and its deck. The page under the
halves takes a moment as well, to become the half that is kept when a branch
is closed. None of that is shown (operator 2026-11-22: "i do not like the
whole transition between ai debugger 1 screen and 2 screens. please clean
this up a lot more"). Before, the window showed all of it: an empty split,
each half's left panel and empty frames, the two maps arriving one after the
other, the box without its deck and then with it, which pushed both halves
down, and on the way back every control greyed and then a cut.

**Held, then changed once.** While the other picture is made ready the
window is held as it stands (`debugHold`): the box is not drawn again and
takes no press, and nothing is asked of the world a split is being made
from. Then the one picture takes the other's place in one move
(`debugSwap`).

- **Branch.** When the branch's Civ AI is picked the split is made at once
  and its halves load where they will
  stand, unseen (`#debugsplit.dbg-unseen`). The one screen stays up, and
  Branch reads "Branching…". The two screens are shown when both halves are
  there (`debugSplitReady`): each has said its turn, its timeline and its
  numbers, half A has given the deck the one panel is drawn from, each seats
  the Civ AI it was given, the branch has let go of the turns ahead of it,
  and each has painted itself (`pane-shown`). So the box has its deck from
  the first frame and nothing moves after it, and B is already seated with
  the Civ AI picked for it. A half that never arrives is not waited on for ever:
  after 6 s the two are shown as they are.
- **A game that opens on two tracks** (`tracks=2`, Doctor Strange's CIVVIS) is not shown on one screen first.
  In the task that draws its first state the window goes under a veil, and
  the veil lifts on the two screens.
- **Drop A, Drop B, a branch's ✕.** The two screens stay as they are, with
  nothing greyed, while the engine makes the kept half the game (`adopt`) and
  this page draws it under them. Then two become one.

**The same ground.** The halves open looking where the one screen was
looking, at the same zoom: the host reads its camera and the place on the
map at the middle of what shows (`debugLookNow`), and each half puts that
place at the middle of its own map (`pane-look`). Closing a branch, the one
screen looks where the kept half was looking. Whether the map was moved by
hand goes with it, so a view somebody chose is not framed over at the next
turn.

**The move.** Where the browser has view transitions the change is one, about
a quarter of a second long. The box goes from one shape to the other with
its right end, where the game ends, held still. The map area goes to half A,
which carries the game on (or the kept half to the map area), and each of the
two pictures is carried by as much as puts the middle of its map on the
middle of the other's, so what is seen is one map with its panels changing
round it while the other half and the strip come or go. Nothing is
stretched. A browser without view transitions, and anyone who has asked for
less motion (`prefers-reduced-motion`), gets the change made and no move.

How long Branch holds the window is how long the halves take: about 0.7 to
1.5 s on the published engine (2026-11-22, headless Chrome in windows 1440
to 1920 wide; the longer times with the host busy).

### One timeline into two, on the turn of the branch

A split's timeline is one up to the turn it was branched on, and two from
there, from the moment Branch makes it (operator 2026-11-26: "please take
more of a doctor strange's civ 6 approach. branches should split from 1 to
2... not 2 to 1"), as Doctor Strange's CIVVIS draws its worlds parting.
Two halves that go on playing the same moves are still two branches, each
with its own Civ AI. Until then the timeline stayed one rail for as long as
the halves held the same world (operator 2026-11-21: "branching the
timeline should keep 1 timeline where they share the timeline and then
branch to 2 when they actually diverge in simulated worlds"), so a branch
whose Civ AI played as A's did showed two pickers joined into one timeline.

- **The fork** is on the turn the branch was made on. It is earlier only
  where the halves' worlds part earlier: when one was walked back behind
  that turn and played on from there differently. Which worlds are the same
  is the engine's word (`debug.worlds`), as before.
- **Each branch** has a rail of its own in its row from the fork, with the
  turns it has played since the split in its own colour and its own marker.
  Two halves that both stand on a turn of the trunk, or on the fork itself,
  have one marker there. The two are drawn as a tree ("A tree on its side",
  below).
- **A branch made behind the last turn played** forks there at once: A's
  rail carries the turns A holds past the fork, and B's is empty.

### A tree on its side

A split's timeline is drawn as a tree lying on its side (operator
2026-11-22: "the branching should form more of a distinct y shape. or like 4
or h like (the one can be between the 2). but more of a tree like timeline
please"):

- **The trunk** is the one timeline up to the turn of the branch. It is a
  slim rail in the middle of the box, with the lock and the game's first
  turn at its root. A's row is over it and B's row under it, and nothing of
  either stands left of it: the tree grows from the left, one timeline into
  two. Each branch ends at the right in what is its own: its letter, the
  Civ AI that plays it and its ✕.
- **The fork** is on the turn of the branch. A branch leaves the trunk
  there for each half, A's slanting up to A's row and B's down to B's, and a
  little further along each is a rail in its own row, with the box's dark
  between the two all the way to the end of the game. Where the branches
  leave, the trunk's gold band is cut down its middle: the top half goes on
  in A's colour and the bottom half in B's, as far as each has played. The
  turn the tree forks on is said between the two branches, beside a ⑂,
  unless both halves stand on that turn: then their one marker is on the
  fork itself.
- **The numbers** of the turns are said once for the whole tree, on the
  dark beside its rails: over the trunk up to the fork, and between the two
  branches from there.
- **The markers**: two halves on one turn of the trunk have one marker, on
  the trunk. Otherwise each half's marker stands in the half's own row, A's
  over the tree and B's under it. Where the tree is not in that row (on the
  trunk, and where a branch has only just left it) a pin runs from the
  marker to the tree, and two locked halves' markers have a link between
  them instead.
- **Save points, breakpoints and eras** a half holds stand where the tree
  is: on its branch, and on the trunk for the turns the two share. While the
  halves are unlocked each half's are its own to go to, and on the trunk A's
  stand a little over B's so that either can be pressed.
- **Pressing it**: the top half of the tree is the hand on A and the bottom
  half the hand on B, on the trunk as on the branches. Each half's slider is
  pressed from its own row to the middle of the tree.

A tree has two branches and no more: there is no Branch while the screen is
split.

How it is drawn (`web/assets/app_debugger.js`): the tree's measures are in
one place (`DEBUG_TREE`: a rail 28 pixels tall, 24 pixels of dark between
the branches, a branch in its row 32 pixels past the fork). Each half's lane
is laid out in its own row, and the part of a branch between the trunk and
the row is the half's rail over again (`dbg-arm`), slanted by the style and
cut along the middle of the tree. What a half's lane holds on the trunk, or
on that slanted part, is moved to where the tree is (`debugOffRow`).

Which worlds are the same is the engine's to say, and it says it turn by
turn. Every mark carries a print of its world (`world_print` in
`src/server/debugger.rs`): the print of the mark it was played on from,
carried on over every action applied since by any seat, and then the turn,
the random stream, and each seat's treasury and progress towards its
research and its civic. The engine is deterministic and the random stream is
part of the game, so two sessions that began as one world hold the same
world for as long as the same actions were applied to both, and their prints
agree for exactly that long. What is read off the world itself tells apart
two worlds that differ by something no action carries. A state document
gives the prints beside the turns they belong to (`debug.worlds`, in the
order of `debug.turns`).

Each half tells the host the turns it holds and their prints (`pane-marks`),
and the host reads the branch off the two lists (`debugSplitFork` in
`web/assets/app_debugger.js`): the last turn both give the same print for is
the last one they share, and the timeline forks there once either half
holds a turn past it. All that is remembered between draws is whether the
timeline was last drawn forked, for a split in the middle of a move, so the
timeline is one again as soon as the halves hold the same worlds again:
after a copy, or after one half goes back and plays its turns as the other
played them.

The stretches are named on a split's timeline as on any ("Whose play each
stretch was"): the turns the halves hold as one on the trunk, and what each
holds past them on its own row. Each half sends its `played_by` with what
else it holds (`pane-marks`).

When turns have been thinned to make room ("The timeline"), only the turns
both halves still keep can be compared, so the branch is drawn on the last
kept turn they share, which may be earlier than the turn they parted on.

The timeline also carries what stands on each half's own, as the timeline of
an unsplit page does: a flag on each save point, a red dot on each
breakpoint turn, and the name of each era on the turn it began. A half has
no timeline of its own to show them on, so it tells the host whenever they
change (`pane-marks`), locked or not. A's stand in the top row and B's in
the bottom row. Clicking a dot goes to that turn the way letting go of the
half's slider there does. Clicking a flag takes that half to its save point;
locked, the other half is brought to the same turn. On the trunk, an era
both halves began on the same turn is named once, at the foot. A save point
and a breakpoint turn are a half's own to go to, so those are said once only
while the halves are locked: the save point at the top and the breakpoint at
the foot, as on an unsplit timeline. Unlocked, each row's marks are its
half's alone.

### The lock

A lock joins the two halves, on a bracket that runs from the middle of one
half's row to the middle of the other's, left of the timeline. It says
whether the two halves are held on one turn. (Whether they are one world is
what the timeline itself shows.)

- **Locked** (how a split opens): the bracket is whole and the lock is shut.
  The halves stay on the same turn. Whatever moves one is repeated in the
  other the same way: an ended turn ends the other's turn as it stands, a
  step steps it, a walk along the timeline walks it. What one is asked that
  leaves its turn where it was is asked of the other too: a layer, a
  breakpoint, Reveal, one action or one seat stepped, an undo, a save point.
  While both stand on one turn of their own branches a link joins their two
  markers.
- **Unlocked**: the bracket is broken and the lock is open. Neither half
  follows the other.

Clicking the lock opens it. It closes only on two halves that stand on the
same turn, which is where it then keeps them; while they stand on different
turns it cannot be pressed, and its tooltip says why. (Walk one half's
slider to the other's turn first, or make one a copy of the other.)

Locked, the two halves are one thing to play, so there is one control panel:
the host's deck, under the timeline. Each half puts its own bar away and tells
the host what was on its deck (`pane-deck`). The host draws its deck from
half A's, and a press or a change on it is passed to A's own deck
(`pane-press`, `pane-change`), which does what it would have done had it
been pressed there; B follows. So every control on the panel means what it
means for one world, and applies to both. The Civ AI is not on the panel: it
is picked at the end of each branch, one for each half. Saves to the device
are one world's and are kept from a half's own panel, unlocked.

When two halves are locked after playing apart, B takes A's layers,
breakpoints and Reveal, so that the one panel is true of both.

Unlocked, the host shows the timeline and no deck, and each half shows its
own bar again: the tools for its world, with neither a timeline nor a picker.

Two locked halves that seat the same Civ AI would be asked the same things,
so A names B as its twin in what it asks the engine. If the two are also the
same world, the engine plays them once, and B's page only reads its state;
see "Identical halves are played once".

The strip between the halves holds the split's other controls:

| Control | What it does |
|---------|--------------|
| View | On, the two maps move together: panning or zooming one moves the other. Off, each looks where it likes. |
| ◧ / ⬒ | Switch between side by side and top and bottom |
| A ⇒ B, A ⇐ B | Make one half a copy of the other as it stands now, turns behind it included (`copy`). Both are written A then B, and the arrow is the way the copy goes: A ⇒ B makes B a copy of A, A ⇐ B makes A a copy of B. They stand together in the middle of the strip |
| Drop A, Drop B | Close that branch and play on from the other (`adopt`): the other's session becomes the main one, with every turn it could walk back to. Each stands at its own half's end of the strip, and does what the ✕ of that branch's row does. (They were Keep A and Keep B, named for the half that stays, until 2026-11-22; operator: "keep isnt informing the user of the action of drop") |
| drag the strip | Resize the halves; double-click makes them equal |
| VS | Head to head; see below |

A half that moved because it was following is not followed back.

### Head to head

Head to head is the split used to play two strategies out from one turn. It
is open as a split opens (operator 2026-11-22: "Default the stats window
open"); VS in the strip puts it away and brings it back, and that choice is
kept on the device (`civvis-debug-versus`). Open, the strip is wide enough
to hold:

- a Civ AI for each half, the same choice that stands at the head of the
  half's row in the bar. It is the agent that plays that half's seat
  whenever the half is stepped or run; each half is a session of its own, so
  nothing in the engine is shared between the two choices.
- the two seats' numbers side by side, A then B, with the larger in gold:
  Score, Cities, Population, Military strength, and Science, Culture, Gold and
  Faith per turn. Each half reports its own (`pane-numbers`) whenever they
  change.
- a chart of one of those numbers over the turns since the split, a line for
  each half in the colour of its letter. It charts Score until a number's name
  is clicked. A half that has walked back keeps its line, with a mark where it
  now stands; turns it plays over again replace what the line showed, and a
  copy takes the other half's line with it.
- a turn to play both to. Locked, A is run and B follows it turn by turn, each
  with its own Civ AI. Unlocked, each half is run by itself. The run ends when
  that turn opens, or earlier if a breakpoint stops a half, and the same
  button stops it by hand.
- ↺, which puts both halves back on the turn they split on, locked or not, so
  the same start can be played again with another pair of Civ AIs. Each half
  walks back along its own timeline, so it lands on the nearest turn before
  that one if the turn itself is no longer kept.

The rivals in each half are played by fresh agents from the split onward, and
again after any walk along a half's timeline: a half keeps no agents with its
marks and seats none from them, unlike an unsplit game. So the two halves
start level every time, at the split and at every rematch, and differ only in
what the seat does and in what follows from it. A half that is kept (the
other was dropped) is the game itself again, and keeps its agents from then
on.

### Identical halves are played once

Two halves that are the same world do not both need playing. The engine
knows when they are: each half is on a numbered line, every request that may
change a half puts it on a line of its own, and a copy takes the line of the
half it was copied from (`src/server/panes.rs`). Two halves on one line are
the same world and timeline with the same agents seated, and nothing has been
asked of either since. Reading a half (its state, the rules, a route or a
combat preview) leaves it on its line.

A `POST /debug` request from one half may name the other as its `twin`. When
the two are on one line as the request arrives, it is answered from the half
that was asked, and the twin is made a copy of what that half then holds: one
turn played, and a fork. The answer's `debug.twinned` is true, and the twin's
page only has to read its state. When the halves are not on one line the twin
is left as it is and `twinned` is false, so the page asks the twin to do the
same thing itself. The page names a twin only when it would ask both halves
for the same thing with the same Civ AI.

Measured on the published engine (build `02525709d` in node, 6 seats,
Continents), a step against the fork and state document that stand in for
the second one:

| Turn | Step | Fork and answer |
|------|------|-----------------|
| 10 | 89 ms | 14 ms |
| 40 | 110 ms | 14 ms |
| 80 | 240 ms | 25 ms |
| 120 | 310 ms | 36 ms |

A copy is seated with agents of its own, and an agent keeps plans from turn
to turn. So of two halves played as one, only the half that was asked has
agents that lived through those turns. When the two are then asked to play
apart (a request that may change one of them, with no twin on the same line),
the half that played is seated anew as well. From there nothing but what is
asked of each tells them apart, as when the screen was first split.

## Forks

A fork is the whole engine again on a thread of its own: the same game, the
same turns behind it to walk back along, the same split halves, as they stood
when the fork was made. From there it plays its own history and the page's
engine never hears of it. A split plays two futures of one world side by side
for a person to watch; forks are for playing many of them at once and only
reading how each came out, which is what a search does.

The published build has them (`beta/shim.js`, `beta/worker.js`):

| Request | What it does |
|---------|--------------|
| `POST /forks` `{"count": N}` | N forks (0 to 64) of the engine as it stands now, in place of any there were. Answers `forks`, `memory_bytes` (one engine's memory) and `ms`. A count of 0 lets them all go. |
| `POST /forks` `{"count": N, "pane": "<world>"}` | The same, of that world's own engine on Doctor Strange's page, where each world has one (below): what its Autosearch searches. Elsewhere one engine holds every world, and the name changes nothing. |
| any engine request with `?fork=K` | Answered by fork K (1 to N) instead of the page's engine: `POST /debug?fork=2` steps, jumps and reads fork 2. |

The engine imports nothing and keeps everything it knows in its own memory,
so a second instance of the same module given a copy of that memory is the
same engine. Nothing is saved or loaded, and nothing in the engine is asked to
take part: the copy is made between two requests, by the worker that holds
the engine.

Measured on published build `145fabd` (six seats, seed 7, at turn 60, an
engine of 114 MB; Chrome for Testing on an 18-core computer that was busy
with other work):

- four forks were made in 83 ms, and one more later in 27 ms; a JSON save and
  load of the same world takes seconds;
- five turns on each of four forks took 676 ms together, and five turns on
  the one engine took 671 ms: four futures in the time of one;
- every fork ended on the state document the engine itself reached after the
  same five turns, byte for byte, and each walked back to the same turn 64.

What a fork costs is memory. Each is the engine's memory over again, every
mark on the timeline included (about 385 MB by turn 140 with every turn
kept), so many forks late in a long game are counted in gigabytes; `room`
lowers what the timeline keeps before the forks are made. The desktop
program has no forks yet: its server does not know `/forks`.

### A world's own engine (Doctor Strange's CIVVIS)

On `?mode=strange` the page holds up to a dozen worlds, and one engine held
them all, so a turn of every world, and the futures played ahead of them,
went one world after another on one thread (operator 2026-11-24: "we want
the doctor strange's civ 6 search across the timeline(s) to be blazingly
fast"). There the shim gives each world an engine of its own: a fork of the
page's engine made behind the world's first request, which answers
everything the world asks from then on. A copy of a world on another engine
(a branch, `op: "copy"`) is given a fork of that world's engine with the copy
made in it; a `twin` on another engine is dropped, so the twin's page asks
for its own turn as when the two are apart; a world whose page has gone has
its engine let go; and the split closed onto a world makes that world's
engine the page's. Outside Doctor Strange nothing changes. Each worker puts
what its engine holds and has spent on every answer, so `/machine-metrics`
adds them up (`engine_threads` says how many) without waiting on a world
that is busy, and the page's meter reads the processor as a share of the
computer's cores. `beta/homes_test.py` checks all of it on a toy engine.

Measured on published build `45e3410` with the real engine:

- in node (six seats, seed 7, warm code), three worlds each played twelve
  turns ahead in four-turn `ahead` requests: from turn 5, 869 ms on one
  engine and 316 ms on three; from turn 60, 4,022 ms and 1,515 ms. Every
  world's twelve readings were the same on its own engine as on the shared
  one, byte for byte;
- on the page (Chrome for Testing, three worlds from turn 1, an 18-core
  computer busy with other work): all three asked for twelve turns at once
  answered in 645 ms on one engine and 216 ms on three; twenty seconds of
  Play Time with the futures off went from turn 1 to 57 on one engine and to
  97 on three.

What it costs is memory: a world's engine holds its world and a copy of the
page's engine as it stood when the world was seated, which on this page is
the opening turns. A world's turns grow its engine by about 4.5 MB a turn
late in a six-seat game (55 MB at turn 30, 199 MB at 100, 623 MB at 195 with
every turn kept), and an engine keeps turns up to its memory line (`room`,
1 GB unless told). One engine held every world under one line; an engine a
world would hold a line each. So each world tells its engine its share of
2 GB (384 MB on a small device), never under 128 MB, again whenever a world
is opened or deleted (`strangePaneRoom`): two worlds 1024 MB each, three 682.
Past its line a world gives back its oldest turns first: told 256 MB, one
world's engine stood at 274 MB by turn 150 and 283 MB at the end (turn 195),
keeping 49 of its turns, in the same time as keeping all of them.

A world's whole future is played on one more engine, its scout (operator
2026-11-24: "each branch in doctor strange's civ 6 should simulate out to
the end of the branch. we can multithread this across the processor to
help"). The page asks for it by the world's name:

| request | what the shim does |
|---|---|
| `<engine request>?pane=<world>&scout=new` | forks the world's engine as it stands, in place of any scout the world had, and has the fork answer |
| `<engine request>?pane=<world>&scout=1` | has the world's scout answer, or a new one where it has none |
| `<engine request>?pane=<world>&scout=done` | lets the scout go, and asks no engine anything |

A scout holds the world as it stood when it was forked, so the page asks
for a new one when the world's line has been played another way. One whose
world's page has gone is let go with it, the split closed onto a world lets
them all go, and `/machine-metrics` counts a scout with the engines
(`window.__civvisBeta.scouts` says how many there are, on this page only).
A scout is the world's engine's memory over again for as long as its game
takes to play out. Off the published build `scout` means nothing: the same
request is answered by the world itself.

## What a search costs

A search plays many futures of one world and reads how each came out. These
are the pieces it is made of and what each costs, measured in the browser
build (six seats, seed 7, Continents, node on an 18-core computer busy with
other work; published builds `e188e21` and `6bc58ca`, and the qualification
build of `65cafca` for the rivals, the conditions and the search below).

| Piece | How it is asked | What it costs |
|-------|-----------------|---------------|
| A turn played and only its outcome read | `step` with `"lean": true` | 36 ms a turn over turns 1 to 20, 69 ms over 40 to 60, 107 ms over 80 to 100. It is the seats' turns and nothing else: nine tenths of it is the Civ AIs deciding |
| A run of turns asked for once | `play` with `turns`, `"lean": true` | the same turns, with one request and one answer for all of them: how the seat stood as each opened (`readings`) |
| The same turn with the page's document | `step` | 3 ms more at turn 40, 15 ms and a megabyte at turn 100 |
| Back to a turn already played | `jump`, `back` with `"lean": true` | 0.5 ms |
| The futures side by side | `POST /forks`, then `?fork=K` | 82 ms for four forks of a 79 MB engine; each fork is the engine's memory again (76 MB at turn 30, 140 MB at 60, 268 MB at 100) |
| Stop when something becomes true | `breakpoints` with `conditions` | one reading of the whole table a turn, about a fifth on top of a turn at turns 80 to 105 |
| Cheaper rivals | `rival_strategy` on `/new` | 80 lean turns: 5.8 s against the house fleet, 4.6 s against `stock`, 3.1 s against `oldguard`, 2.9 s against `wheels` |
| Another Civ AI in the seat | `"strategy"` on the step | nothing: the agent is seated for that step |

`tools/simloop/search.mjs` puts them together as the smallest search there
is: from a turn, each of eight Civ AIs holds the seat for ten turns and the
seat's score and lead are compared. It plays it on eight forks at once, and
again on the one engine walking back between agents, and fails if the two
disagree on a score.

| From turn | Rivals | On eight forks | On one engine | Turns played a second, on forks |
|----------:|--------|---------------:|--------------:|--------------------------------:|
| 30 | the house fleet | 0.84 s | 4.9 s | 96 |
| 60 | the house fleet | 1.50 s | 8.9 s | 54 |
| 60 | `wheels` | 0.82 s | 4.3 s | 98 |
| 100 | the house fleet | 2.72 s | 13.8 s | 29 |

The two arrangements agreed every time, so a fork is the engine and a walk
back is the world that was left. Forks buy time with memory; one engine
walking back costs neither, and is what a search that wants more futures
than there are cores, or than there is memory for, falls back to.

## Autosearch

Operator, 2026-11-22: "build out autosearch button in the ai debugger. make
it a bit bigger than the others. autosearch should allow use to use some
fraction of the cores of this machine to each run a search up and down the
timeline. allow a reasonable objective function to be chosen. score or prod
or city or fpt or gpt".

Operator, 2026-11-23, pasting a review of it: "don't build a new system.
Yesterday's Autosearch (#5686) is already this idea with a one-number
objective. Generalize its objective into your four lists, move the search
core from JS into Rust, and use the model outside the simulation loop, never
inside it."

Autosearch answers one question: from the open turn, which Civ AIs should
hold the seat, and when, to meet four lists a chosen count of turns on. Its
button stands in the bar after Breakpoints and opens a panel with these to
set:

- **The four lists.** What the seat **Needs**, what it **Must avoid**, what
  it **Wants** and what it **Doesn't want**. A Need or a Must avoid is a
  number held to a line (`cities >= 6`), with a *when*: at the end of the
  line, always, ever, or by a turn. A Want or a Don't want is a number with
  a weight. Each reads one of the seat's own numbers, by the names its row of
  the standings gives them (Score, Cities, Population, Production, Science,
  Culture, Faith and Food a turn, net Gold a turn), or whether it has won
  the game or lost it (fallen, or beaten by another). The panel opens on win
  (a Need, at the end), do not lose (a Must avoid, always) and the most
  Score (a Want); a choice kept from before the lists is its one number,
  wanted. The engine reads any key of the omniscient observation as well.
- **By**: how far ahead to play, 10 to 100 turns.
- **Cores**: one core, or a quarter, half, three quarters or all of the
  cores the browser says this computer has.
- **Also** and **Genes**: Civ AIs to try beyond the roster's. The seated Civ
  AI aimed at each victory (six more), and the seated Civ AI with each gene
  named switched on (one more a gene). See "Lines, and how the search
  climbs".

A split has one more, **From**: which branch the search begins from. What is
set is kept for the next visit. The panel is a window like the box's others
("Panels are windows"): it is moved by its title, and it is the box's own
and not the deck's, so a split whose halves are unlocked shows it too,
opened from either half's own Autosearch button.

### Lines, and how the search climbs

A line is a plan of who holds the seat: one Civ AI from the turn the search
begins on, and any number of handovers after it, each on a turn. A Civ AI is
a roster name, or one aimed at a victory with gene tags on top,
`jack@science+no-city-limit` (`strategies::build_named`): the roster has a
handful of agents, and a gene is a switch an agent reads as it plays, so the
alphabet a line is written in is as wide as the genes asked for. The engine
refuses a name it cannot build before any line is played.

The climb is the engine's (`src/server/autosearch.rs`, `Search`), and the
page asks it for each line to play and tells it what each came to
(`POST /autosearch`, below). It first plays every Civ AI on offer alone, the
seated one among them. Then it climbs: around the best line it has, and
around the rest of the front (below), at each of a spread of turns along a
line, it hands the rest of the line to each other Civ AI, and also just one
stride of it. The moment a line passes the best it lets what was waiting go
and begins again around that one. The turns are a quarter of the search
apart to begin with, then half that, down to a turn (and to no finer than a
thirtieth of a long search). When nothing at the finest stride is new, the
search is done. It is a climb and not a proof.

Which of two lines is the better (`Search::better`): one that meets every
Need and breaks no Must avoid; then one that fails fewer of them; then the
more of what is Wanted less what is not; then fewer handovers. A Want counts
as percent over the seated Civ AI's line alone, times its weight, so a weight
means the same for Score as for Faith a turn; a Don't want counts the same
way, the other way round. Every line that no other beats on every term at
once is kept (the front), and the climb grows from those as well as from the
best, which takes it past the single handover a climb from one line stops
at. Every term is read on every turn a copy plays (`objective`, and the
`terms` of a `play`). A Must avoid held always, ever or by a turn ends a
line on the turn it is broken, as a breakpoint ends a run; nothing is cut
on a soft number, because `docs/SUPERHUMAN.md` measured that "any
within-review pruning discards real signal".

When no line meets every Need, the panel says so: none of these Civ AIs
does what is needed, which names a behaviour to write a gene for.

Before the four lists the panel made the most of one number, and the ranking
was fixed: a line the seat wins on, then one it does not lose on, then the
number, then fewer handovers. The lists it opens on are that ranking.

### Cores, and up and down the timeline

Each core that is given to the search plays on a fork (see "Forks"): the
whole engine again on a thread of its own, with the timeline behind it. The
page's own game is never asked anything while a search runs, so it can go on
being played, and the copies are let go the moment the search ends or is
stopped, because each is the engine's memory over again. The panel says how
many copies there are and what each weighs.

To play a line a copy walks back down the timeline to the turn where the
line parts from the one it played last (`jump`, with the line's Civ AI for
that turn named) and plays up it from there, a few turns a request (`play`),
reading how the seat stood as each turn opened. Most lines the climb tries
differ from the best line only late, so most of the work is short. The panel
shows each core as a row: the turns it holds of the line it is on, cut back
and grown again.

That walk back has to be exact, or what the search found is not what
playing it gives. It is, for the reason "Turns that are not played again"
gives: the agents kept with a turn sit down again when the turn is put back.
The game itself keeps them with every turn. A half of a split keeps none, so
the copy of a half is asked to keep them from the turn the search begins on
(`remember`, below), and the half on the page plays the line straight from
that turn without walking back along the way. Both begin from the top of the
turn as a walk back to it leaves it: what the seat did by hand in the open
turn is not part of a line. `a_schedule_walked_back_to_is_the_schedule_played_straight`
holds this in the engine's tests, and `tools/simloop/walkback.mjs` measures
it on a real world in the browser build, as the page and a copy each play a
line. On the build of `0ba58f1` (six seats, Continents, seeds 7 and 11, from
turns 20 and 45): nine walks back in different orders, on the game itself
and on a half, each ended on the same print of the world and the same
reading of the seat as the line played straight, where six lines gave six
different worlds.

### What it finds, and playing it

While it runs and after, the panel says the best line: the number it comes
to and on which turn, what that is over the Civ AI seated alone, the line as
its stretches (the turn each Civ AI takes the seat on), the number over the
turns for both as a chart, and what each Civ AI alone comes to.

**Play it** plays the best line on the page, with the page's own controls.
The world walks back to the turn the search began on, each stretch seats its
Civ AI and is run to its last turn as Auto-play runs, so a breakpoint stops
it and so does Stop, and the timeline names each stretch by the Civ AI that
played it. On the one timeline that is this timeline. In a split there is a
button for each half: the half the search began from plays it over the turns
it holds past that turn, and the other half is first made a copy of that
one, so the one searched stays as it was, to set beside the line. Locked,
the other half plays the same turns under its own Civ AI. When the line is
played out the bar says what the seat has beside what the search read
("SCORE 71, as found"), so a line that did not play out as found is seen at
once. A line is played on the world it was found in: after the timeline is
branched, a branch is closed or a new game begins, the search is run again.

Measured in Chrome for Testing on the build of `0ba58f1` (six seats, seed 7,
Continents, an 18-core computer busy with a tournament and a live game, a
quarter of its cores): from turn 12 for ten turns on the one timeline, 559
lines and 3,384 turns in 16.7 s on five copies of 44 MB (200 turns a second),
the best line worth 71 Score against 65 for the Civ AI seated, and played on
the page it came to 71. On all eighteen cores the same search took 8.3 s
(474 turns a second) and found a line of the same worth. From a half of a
split, 273 lines in 12.6 s, 66 against 63, and played in the other half it
came to 66 while the half that was searched played on to 63. Later in a game
a turn costs more and a copy weighs more: from turn 40 on nine cores, 85
turns a second on copies of 128 MB.

Those measurements were of the climb when it was the page's own, with one
number to make the most of; the climb is the engine's now, and the lines it
tries are the same lines.

Autosearch in the page needs forks, so it runs in the browser build. The
desktop program's server does not know `/forks`, and there the panel says
so; `civvis autosearch` (below) is how a search runs without a browser.

### Without a browser: `civvis autosearch`, and one world is not every world

`civvis autosearch --spec spec.json [--seed N] [--cores N] [--offers
plans.json] [--robust 101,102,…] [--fog-honest true|false]` runs the same
climb on this machine's own copies of the engine, one a thread
(`autosearch::run`). The world is the site's opening world on the seed,
played by the seated Civ AI up to the turn the search begins on. The spec is
what the page sends: `{"objective": {"need", "avoid", "want", "dont_want"},
"from", "turns", "letters", "seated"}`. The report is printed as JSON.

A line found on one world is a fact about that world: `docs/GENOME.md`
records three effects that "dissolved under decomposition or resampling",
and that "the only defence is replication on disjoint maps". `--robust`
plays the best line found, and the seated Civ AI alone, on each of the seeds
named, from the same turn, judged by the same lists, and says how often the
best line still met every Need there, how often the seated Civ AI alone did,
and what the line came to against it on average.

On one core a search goes the same way every time. On several, which line
comes back first decides what the climb tries next, so two runs may try
different lines and settle on different ones of equal worth (seed 7, from
turn 10 for six turns: 72 lines and the same line on one core both times; 80
lines and then 77 on four, ending on two different lines that both came to
Score 32).

Measured on this MacBook Air (10 cores) with a release build: the site's
opening world on seed 7, from turn 20 for 20 turns, eight roster Civ AIs,
JackOfAllTrades seated, a Need of six cities by turn 40, do not lose, and the
most Score. On eight cores 1,310 lines in 78 s. No line met the Need. The
best line, SiegeTheDay, then HolyRoller from turn 35, then ByTheBook from turn
38, came to Score 105 against 82 for JackOfAllTrades alone (+28%). Played
again on seeds 11 and 13 (`--robust 11,13`) it came to +1.5% and −4.2%
against JackOfAllTrades alone there: the +28% was a fact about one world.

### What a model is for, and what it is not

A language model is no part of a turn. A turn costs tens of milliseconds and
a model's answer seconds, and a model does not answer the same way twice,
which would break the exact walk back every line rests on. What it can do
is work from outside the loop. It can put lines in to be played and judged
like any other (`offer` to `POST /autosearch`, `--offers` to the runner),
because the simulator verifies what it proposes exactly. It can turn plain
words into the four lists, which the world then checks key by key before a
line is played (`objective` refuses a number the seat does not have). And
when no line meets a Need, it can name the missing behaviour for a gene to
be written and priced by the gene screen. None of that is wired to a model
here.

What a term reads is the omniscient observation, the spectator's view of the
whole table. That is right for a searcher looking at a world, and it says
nothing about what a fog-honest player could have known ("Fog-honest AI").

## Fog-honest AI

Every Civ AI at a site table is fog-honest, always (operator 2026-11-23: "it
should pretty much always be set to true"; 2026-11-26: "remove fog honest ai
from turn box (this should alawys be true)"). It plans its turn on what its
own civilization can see and remembers, and replays the plan on the board
(`ai::player::take_turn`). The turn box offered a **Fog-honest AI** row, True
or False, from 2026-11-23 until 2026-11-26; the page has no way to turn it
off since. The engine still has the other setting, False, in which a Civ AI
plans on the whole board, for the tools and tests that compare the two. Every
world the engine deals opens True, and False lasts for the world on screen.
It is the world's setting
(`POST /fog-honest`, `Session::set_fog_honest`), so each half of a split
keeps its own, a half begins with the setting of the world it was copied
from, and the agents kept with a turn play on under the setting the table
has now when that turn is walked back to. City-states, barbarians and
TrainingWheels are scripted and read the board either way.

What a breakpoint or Autosearch reads is not fog-honest and does not claim
to be: a condition and a search's number are read off the omniscient
observation, the spectator's view of the whole table. That is the searcher
looking at the world, not a player looking at it.

## Doctor Strange's CIVVIS

Operator, 2026-11-23: "it will have a scrolling guitar hero setup. where the
timeline we are on and following is center screen. forward in time is up.
forward will branch out more in the tree. bottom left quad and bottom right
quad, roughly are our world to compare. 2 simulatoins. this is limited at 2
for this demo. the ai debugger we will make to scale to any number of
screens."

`?mode=strange` is the split screen above, shown another way. It is listed
just above AI Debugger in the site list, and it is the same played game: one
engine, a world for each timeline, a Civ AI in each seat. `beta/shim.js`
seats the visitor for `mode=strange` as it does for `mode=play`, and the last
section of `web/assets/app_debugger.js` draws the rest over the split the
debugger makes.

Operator, 2026-11-25: "lets use 4p pangaea maps for doctor strange's civ 6".
On the published build the page opens on four civilizations on a flat
Pangaea, a Tiny world (`MODE_WORLDS` in `beta/shim.js`); a link naming its
own `players`, `map` or `shape` keeps those. New games after it keep the
world on screen, as the setup controls read it back. The desktop build
(`civvis play`) plays whatever world it was started on:
`civvis play --players 4 --map pangaea --shape flat` is the same world
there.

Operator, 2026-11-24: "instead of random lines, we can compute (a degree in
advance) the futures of the paths we are on. if there are 2 screens below,
or 1, or 3. we should be able delete branches and that removes the line and
the window. we should be able to handle scaling windows to a high number. we
should also monitor resource usage - maybe track memory used and cpu %
actively on the screen, in the upper right. the upper right chart and drop
to the lower right of the upper half of the screen", and "an experience i
want for example, is maybe to be simulating along 3 paths. 2 different
player-strategies at the start of the game and 1 branched during the game.
(also, we should default open doctor strange civ 6 with our top strategy on
the right and the challenger on the left. you can try to piece together a
strategy on the left to beat the number 1 on the right). but simulating 3
paths along their current strategy and we should have for example nuclear
war visual forecasts out in the future (we will compute the future of each
path). these should alert on the top section out in the distance."

- **As many worlds as are branched, up to twelve.** A and B are the split's
  own two. Each world past them is a page this section makes
  (`strangeWorldOpen`): a frame named by the next letter (`c-<split>`), whose
  record the split's listener keeps beside A's and B's, and whose world the
  engine keeps as it keeps a half's (`panes.rs` names a pane by any letter).
  It begins as the game the split was made from and is made a copy of the
  world it branches from once it has loaded. The split's own lock joins two
  halves, so it is left open, every half's own panel is put away, and the
  worlds are moved by the page's one hand, by name.
- **A world's box is its map alone.** Its world minimap and the map's
  controls (deck, compass, lenses, zoom) are put away, and the minimap is
  not drawn (`STRANGE_PANE_STYLE`, `drawMini`). Each map still pans by drag
  and zooms by wheel.
- **The page opens on the top strategy and its challenger.** The picker
  shows each player strategy's rating before its name; the highest rated is
  B, on the right, and the next is A, on the left and followed, so that a
  branch is made from the challenger's side. Where the picker shows no
  ratings A keeps the page's own choice and B is given another.
- **What the page costs is in the top right corner** (`strangeMeasure`):
  processor and memory in use, each with the last two minutes drawn, and how
  many worlds are open. The engine's host says what it can
  (`GET /machine-metrics`). A game served by this computer gives the
  computer's own load and memory in use. The published build's engine cannot
  see the computer, so its worker adds what it knows (`beta/worker.js`): the
  memory the engine holds (`engine_bytes`) and the time it has spent
  answering (`engine_busy_ms`), from which the page reads how much of its one
  core the engine is using; the page's own memory is added where the browser
  tells it.

- **The worlds are boxes along the bottom of the window.** Each half puts
  its standings, tracker, seat boxes and its own panel away and is a map,
  under a name plate with its branch's name, its player strategy's picker and
  its exit. Up to three stand in a row, then two rows, then three
  (`strangeGeometry`). The numbers of the worlds are set side by side on a
  card that stands on the line of the open turn at the right, in the lower
  right of the upper half; past three worlds it has room for what each makes
  a turn and not the totals.
- **A branch is named by who played it.** Operator, 2026-11-24: "branches
  should be labeled with their series of player strategies. like
  MoonshotMaven-150-JackOfAllTrades-160-MoonshotMaven would mean moonshot
  maven played the first 1-149 turns, then jackof alltrades from 150-159,
  then moonshotmaven 160 onwards". The name is read from the engine's own
  record of whose play each stretch of turns was (`played_by`, a half's
  `marks.by`) as far as the last turn of the world's line, and the strategy
  seated now is named last, with the turn it plays from, when it is not the
  one that played the last stretch (`strangeSeries`). The name is on the
  world's name plate, and on the tree where the branch ends when that is
  ahead of the open turn. Turns ended by hand are `ByHand`.
- **A box is in the colour of its branch.** Operator, 2026-11-24: "we should
  highlight a box in the lower part of the screen with the color of the
  branch". Every branch made is given the next of eight colours
  (`STRANGE_INKS`); its line on the tree, its name and the border of its box
  are that colour, and a branch that is let go of keeps it. The trunk is the
  line of the world that was branched from. The pointer over a box, or its
  name plate, lights that world's line from the first turn to its last and
  stands the others back ("The cursor over a world on the bottom should
  highlight that world's line").
- **A box has an exit, which deletes its branch.** "we should be able delete
  branches and that removes the line and the window". ✕ takes the world's
  line from the tree and its box from the window, and the boxes that are left
  share the room. The last world stays. A world past the split's two is let
  go of, page and all; one of the two is put out of sight, because the split
  is made of them, and is the next branch's box.
- **The line of the open turn is the divide** between the tree above and the
  boxes below. The page places it so that a box has the shape of a map,
  three high to five wide, within a share of the window (`strangeGeometry`):
  two boxes are narrower and so shorter, and one box is taller and takes the
  line up with it. It never goes above the cards at the top. It can be
  dragged, and a double-click gives it back to the page. The turn and its
  era are written on the line, beside the board ("Move turn count up to the
  center line, as that is the current turn"). Two boxes have the trunk's
  column between them. Three or more leave a strip clear under the line
  where the worlds stand on it, and one box's plate is cut away there.
- **Each yield is a figure a turn and a total so far.** Operator, 2026-11-23:
  "lets track cumulative prod, science, culture, gold, faith next to the
  current turn yields". The card in the top right gives each world two
  columns: what its seat makes a turn, and for Production, Science, Culture,
  Gold and Faith what it has made over the game so far. The totals are the
  engine's: a major civilization's `totals` in `players` is its rows of the
  result screen's record (`turn_stats`) added up, Gold net of upkeep. The
  record is part of the world, so a world walked back or branched carries the
  totals of its own past (`every_civilization_reports_what_it_has_made_so_far`
  in `src/obs/tests.rs`).
- **The timeline is a highway up the middle.** The open turn is a line across
  the centre of the window. The turns played run down from it, between the
  worlds, and the turns to come run up from it. A turn near the line is tall
  and one far from it thin, so the whole game is on the board. Playing
  scrolls it: the line follows the followed world's turn.
- **The highway is a tree** (`strangeTree`). Which worlds are the same is the
  engine's own word, as it is for the split's two (`debugSplitFork`): each
  world says the turns it holds and the engine's print of each. A world is a
  branch of the older world it shares the most turns with, from the last
  turn they share, and the oldest is the trunk. The followed timeline stands
  in the middle and the rest in lanes to its sides, as many lanes away as
  their boxes are boxes away; any can be followed. A city founded is a gem
  on the line that founded it, and an era is named on its first turn.
- **Only what was played is a line, and each world's future is played.**
  "we should also not spew random lines on the timeline", then "we can
  compute (a degree in advance) the futures of the paths we are on". Each
  world is played twelve turns ahead of the open turn along its own player
  strategy (`strangeAheadNext`), by the engine's `ahead`: the world plays on
  from the last turn it holds and comes back to the open one inside the one
  request, so nothing else finds it away. The turns are kept ahead of the
  open turn like any turn played and walked back from, so they are the line
  the tree draws, with the world's name and score where it ends, and time
  running on is shown them instead of playing them again: while Futures is
  on, no world is asked for a turn of its own. Ahead of the last turn played
  there is a dashed way straight on and nothing else.
- **Time runs on without stopping.** Operator, 2026-11-24: "Right now Doctor
  Strange's Civ 6 goes 4 turns and pauses. This should not happen. It should
  be a continuous effort. and blazingly fast." It did: the futures were
  played four turns at a request, one world after another, with time held
  until each had answered, and every turn between was kept on the screen for
  160 ms. Measured on the published page with two worlds, 30 s of Play Time
  from turn 1: every fourth turn stood for 0.4 s by turn 10, 0.8 s by turn
  50 and 1.2 s by turn 90, and the worlds came to turn 93. Four things were
  the matter, and each is put right where it was:
  - *Time waited for the future.* Now every world is asked at once, each
    for its own (`strange.peeks`, one request out a world), and time waits
    only for a world that does not hold the next turn (`strangeAsk`). A
    world that holds it is shown it.
  - *The future was played in fours.* Each turn a world is shown takes one
    from what it holds, so as it is shown the turn it is asked for one more
    (`strangeShow`, `strangeAheadOwed`: the `pane-show` carries `ahead`), and
    its engine plays that while the page draws the turn (`debugPlayAhead`).
    What a world holds stays twelve turns deep, and time runs at the pace
    the worlds are played at. A world that holds fewer, one just branched
    or one whose future a black swan let go of, is asked for two a turn
    until it holds twelve. While the worlds stand still they are asked for
    four at a time, as before, so that whatever is asked of a world next
    waits no longer than that.
  - *The future was not the game the player strategy plays.* A half keeps
    no agents with its turns, so each `ahead` was played by agents that had
    just sat down, with none of what they had planned, and that is another
    game within a turn (`Minds`). Played to turn 100 from the opening turn
    by JackOfAllTrades, a run comes to a score of 149 with seven cities; the
    future in fours, which is what time running showed, came to 112 with
    five, and held the run's world on 5 turns of the 100. It was also slow:
    an agent that has just sat down works out again what one that lived
    through the turn before still holds, and a turn at a time the future
    cost 1.4 times what a run does (43 ms a turn against 34 at turn 80, the
    engine by itself on this Mac). The engine now keeps the agents that
    played a world's last turn ahead, with the print of the world they were
    left on, and they play its next (`Session::debug_ahead`, `Scouts`). The
    future in fours and a turn at a time are both the run's world on 100
    turns of 100, at the run's cost
    (`a_future_played_a_turn_at_a_time_is_the_one_played_in_one_go`).
  - *The page was laid out again a dozen times a turn.* Each world says a
    dozen things a turn, and for each the page measured its bar, which is
    out of sight under the worlds, and its cards. A measure makes the
    browser lay the whole page out: 3.8 s of 12 s of the page's own thread
    while time ran. The bar is not measured while it cannot be seen
    (`debugBarUnseen`, and a world has none), and the cards are measured
    when one changes its shape (`strangeCards`): 0.3 s of 12. A world also
    read every save again on each turn it was shown, for a lobby it does
    not show, and had the engine price a deck of policy cards nobody sees
    whenever a turn stood for a third of a second; it asks for neither.

  What came of it, two worlds and 30 s of Play Time from turn 1. On the
  published engine with only the page changed: no turn stood longer than
  0.67 s in three runs, where every fourth stood up to 1.27 s. On this Mac's
  own engine
  with all of it: a turn every 88 ms through turn 60, where it was every
  208 to 225 ms, and the longest any turn stood was 0.46 s against 0.82 s.
  From there the worlds are the larger game a run plays, and a turn of it
  is more to play. What sets the pace now is the engine playing one turn
  of each world for each turn shown; a world's own engine
  (`beta/shim.js`) plays those side by side.
- **What is coming is put up on the line it is coming on.** The engine
  answers `ahead` with what it saw on the way (`debug_alerts`): every nuclear
  detonation in the world, whatever the played seat was told that was too
  consequential to scroll past, and the end of the game. Each is a sign on
  the world's line at the turn it happens on, out in the distance, which
  comes down the board as the turns are played: ☢ in red with a ring that
  beats, always named; the rest named where there is room.
- **Each branch is played out to the end of its game.** Operator,
  2026-11-24: "each branch in doctor strange's civ 6 should simulate out to
  the end of the branch. we can multithread this across the processor to
  help. The top of each branch should have either: no nuclear war in the
  next N turns, no nuclear war, or nuclear war in N turns (Era). we should
  note wars as well. we should optimize the simulation speed so we simulate
  out pretty quickly. Can build out the most likely paths at all time."
  - *The whole future.* Past the dozen turns a world holds, its future is
    played to the end of its game by the engine's `foresee`
    (`Session::debug_foresee`, `Seer`): a copy of the world at the end of
    its line, seated with the agents that play on from there, played on a
    stretch a request. It is the future the world itself goes on to play:
    the engine is deterministic, and every turn foreseen carries the print
    the world's own turn is given (`debug.worlds`), which is how the page
    knows a future is still its world's (`strangeSightStale`).
  - *It costs the turns and nothing else.* A turn played ahead (`ahead`)
    keeps a mark, megabytes of the world copied wherever the next turn
    writes to it, so that it can be shown and walked back to. The copy keeps
    no turn: it is the only holder of what it holds and a turn writes to it
    in place, so the whole of a game's future takes the memory of one world
    where a mark a turn would take hundreds. It reads nothing of the seat on
    the way and is given no state document. What it keeps is what a
    forecast says: each nuclear detonation and each war declared, with the
    turn and the world era it happens in, how the game ends, and the print
    of each turn.

    Measured 2026-11-24 on the desktop build (`civvis play --players 6
    --seed 11`, JackOfAllTrades in the seat, an 18-core computer busy with
    other work and the job at the lowest priority): a world's whole future
    from turn 1, 380 turns to the win, was foreseen in 84.9 s in twenty-turn
    requests, and the same game played as a run with every turn kept took
    87.8 s. All 380 turns were the same world in both, print for print, and
    both ended on the same turn. A turn cost 15 ms by turn 20, 64 by 100,
    209 by 200 and 336 by 300. A sample of the process through turns 170 to
    260 has all of a forecast's time in the seats' own turns
    (`Session::autoplay_round`): the agents' planning
    (`AdvancedAi::plan_observed_turn`) 54%, the view each plans on
    (`Game::player_decision_view`) 19%, the city-states and barbarians 8%,
    and the copy's own keeping (the prints, what was seen) too little to
    show. A forecast is as fast as the game is played: what makes it come
    sooner is a core of its own, below, and whatever makes a turn cheaper.
  - *On every core.* On the published build each world's future is played
    on an engine of its own, a scout (`beta/shim.js`, "a world's scout"): a
    fork of the world's engine as it stands, on a thread of its own, asked
    nothing else. The page asks every world's scout at once, as many as
    there are cores to spare (`strangeForesee`), so the futures of all the
    worlds are played side by side and a world's own turn never waits
    behind its future. A scout is let go when its game has ended. A game
    served by this computer has one process for every world, so there a
    world's future is played by the world itself. Each world is behind a
    lock of its own on that server (`panes.rs`), so the futures of all the
    worlds are still played at once, as many as there are cores to spare;
    but a world asked for its future is not free to play a turn, so they
    are asked only while the worlds stand still. On the page over `civvis
    play --players 4 --seed 11` (Chrome for Testing, the same busy
    computer), the two worlds' futures were played from turn 13 to their
    ends, on turn 335 and turn 220, in 30.4 s side by side, where one after
    the other they had taken 57.2 s, each sign saying how far its world was
    clear of a nuclear war as it went.
  - *Always.* A world is asked the moment it is open and again each time it
    answers, until its game has ended; and anew, from the end of its line
    as it then is, when its line has been played another way: another
    player strategy in its seat, a black swan, a branch.
  - *The sign over each branch* (`strangeFate`, `.strange-fate`), over the
    far end of its line at the top of the tree, in the branch's colour,
    says one of three things, in turns from the open one so that it counts
    down as time runs: **Nuclear war in N turns (Atomic Era)**, red, with
    the world era it comes in (one that has begun says since which turn);
    **No nuclear war**, when the future has been played to the end of the
    game with none; **No nuclear war in the next N turns**, as far as the
    future has been played so far, N growing as it is played on. Over it
    **On current path,** (operator, 2026-11-25: "add to message: on current
    path, no nuclear war..."): what it says is the future of the world's
    line under the player strategies seated now. Under it
    the next war to be declared, in how many turns and by whom on whom, and
    how many more follow, or that there are none ahead and how the game
    ends. A bar along its foot is how much of the game has been played
    through, and the pointer on it lists every detonation and war ahead and
    how fast it is being played. With many worlds open the signs say less
    and stand in two rows (`strangeFates`).
  - *On the line.* Past the turns a world holds, its line is drawn thin as
    far as its future has been played, and stops where the game ends in it;
    the dashes go on only where nothing has been played. The detonations,
    wars and the end foreseen are signs on it at their turns, as the ones
    seen in the turns held are.
- **Branch is one step.** Operator, 2026-11-24: "when we branch, we should
  select the player strategy and make the branch. in one step. Branch:
  1100-MoonshotMaven (player strat dropdown)". The Branch control is a
  picker that lists what the branch's name would gain, the open turn and
  each player strategy the followed world does not seat (`150-MoonshotMaven`),
  and picking one makes the branch (`strangeBranch`): a new world, in a box
  of its own beside the one it was made from, which is the followed world on
  the open turn (`pane-copy`) with that strategy in the seat and a colour of
  its own.
- **The base timeline is picked before Branch.** Operator, 2026-11-25: "the
  option to branch in the simulator should be preceded by a selection of the
  base timeline". Over the Branch picker, a Base timeline picker lists every
  open world, left to right, by its letter and its name (`C ·
  Stock-60-JackOfAllTrades`), and wears the colour of the one picked. The
  base is the followed world: picking one there follows it, which puts its
  line in the middle of the tree, and a box's Follow button picks it there.
  Branch then offers what the base does not seat and makes the branch from
  it. It costs the title card a row (33 px): in a short window (the test's,
  1400 by 757) the line of the open turn stands on the card's floor with two
  boxes as well as with one.
- **Play Time and Pause Time.** The play button runs time on from the open
  turn and reads Pause Time while it runs.
- **Going back** (the step buttons, To the fork, or a press on the highway at
  a turn's height) puts the worlds back as they stood. The turns they hold
  ahead of the open one stay on the tree, above the line.
- **Locked, the worlds go through the turns together** (operator
  2026-11-23: "the doctor strange tool should step through turns in sync
  when locked (which is the default). always on the same turn. difference
  being only the intentional differentces: like say 2 diferent player
  startegies"). The AI Debugger's own locked run plays A and has B follow
  it, and A asks for its next turn before B has played the last, so B fell
  turns behind. Here the page asks both worlds for the same next turn at once
  (`strangeAsk`, a `pane-follow` with `hold`), each with its own Civ AI, and
  each holds the turn it has played, unshown, until the other has played its
  own (`pane-ready`, `pane-show`): both are shown it together, and only then
  is the next turn asked for. A turn a world has already played along its own
  timeline is walked to as it stood. Pause stops both on the turn they were
  asked for. A world that cannot play the turn (its game is over) ends the
  run and says so. The worlds differ only in what is chosen for each.
- **The lock** is a button on the panel, locked to begin with, and is the
  page's own (`strange.locked`). Unlocked, each world plays and goes back by
  itself, its name plate says its own turn, and the step buttons move the
  followed world. Locking again brings the other worlds to the followed
  one's turn first.
- **Generate Black Swan.** Operator, 2026-11-24: "add a 'Generate Black
  Swan' - this will be a random, unpredicted event that spawns and changes
  the game. Could be instant nuclear war. Bio risk could collapse the
  population of all cities by 50%. Radiation event that starts leaking
  radiation around the world. Leader collapse. Civ blossoms out into powerful
  hostile nuclear armed microstates. stuff like that. black swans on a
  civilizational timeline." The button stands at the top of the sky, over the
  far end of the tree and between the cards, because it is not one of the
  hand's moves on the highway. Beside it is which one: Any, drawn at random
  among those the world has something for, or one by name. A press has the
  engine strike it on the worlds as they stand (`strangeSwan`, `pane-swan`,
  the `black_swan` operation; see "Black swans" below). Locked, the same one
  strikes every world: the followed world is struck first and says which it
  drew, and the rest are struck by that one with the same seed, so worlds
  that are still one world stay one, and worlds that have parted meet the
  same event. Unlocked, the followed world is struck alone, and parts from
  the rest there. What struck is said under the button for a quarter of a
  minute, with the followed world's own account of it, and the sky lights for
  a moment. A struck world holds no turn ahead of the open one any more, so
  its future is played again from the world it now is: what the forecast did
  not see coming changes what is seen coming. The swan is a black disc
  flagged out from the line on the turn it struck, beating while part of it
  is still going on, and a word in the line's name: `MoonshotMaven-🦢150` is
  MoonshotMaven's line with a black swan on turn 150. A world it is still
  going on in says so on its name plate.

`beta/debugger_test.py` (`check_doctor_strange`) holds all of it in a browser
against the toy engine: the opening seats, the layout, the names and colours,
the one-step Branch into a third world, the pointer over a box, going back,
following, the locked run, the divide, deleting a world of each kind and
opening one again, the meter, the futures and their alerts, each world's
future played to the end of its game with the sign over each branch saying
what is coming (every world at once, on the world itself and where each
has a scout), time running
through turns played ahead, a black swan on every locked world and on the
followed one alone, and the narrow window, which has the one timeline
and says why. `beta/homes_test.py` holds what a scout is on the shim as it
ships. The engine's side is `ahead_plays_the_turns_to_come_and_comes_back_to_the_open_turn`,
`foresee_plays_the_future_to_the_end_of_the_game_and_leaves_the_world_where_it_is`,
`a_forecast_is_kept_along_its_line_and_made_again_off_it`,
`a_forecast_notes_the_detonations_and_the_wars_declared_on_the_way`
and `a_black_swan_strikes_the_open_turn_and_lets_go_of_the_turns_ahead`
(`src/server/debugger_tests.rs`), the pane names in `src/server/panes_tests.rs`,
and the swans themselves in `src/game/black_swan_tests.rs`. Two games are
played on through every kind of swan, with the future played ahead after
each, in `a_game_is_played_on_through_every_black_swan`.

### Many worlds on the desktop program

The desktop program's server answers each page on a thread of its own, but a
half of a split was answered holding the lock on every half and the lock on
the main game for the whole of its turn: Doctor Strange's worlds played one
after another there, and the page's own requests waited behind each turn.
Each half now has a lock of its own (`Half::session`). A request to a half
that is already seated, that copies nothing, names no twin and does not part
it from a half it is alike with, takes the halves' lock only to find its half
and is answered holding that half alone (`Panes::alone`, `socket::route`), so
the worlds play at the same time and the main game is free. Seating a half,
a copy, a twin and a save are answered as before. Closing the split waits for
the kept half's last answer (`Panes::adopt`). The published build's engines
each hold one world already ("A world's own engine" under Forks).

### What a turn of many worlds costs the page

Every world is a page in a frame of the same site, so all of them, and the
page that holds them, draw on one thread. Once each world plays on an engine
of its own ("A world's own engine" under Forks), that thread is what a turn
waits on: three worlds from turn 30 kept it 85-90% busy, about 268 ms of it a
turn, and the turns ran at about 300 ms. Where it went (Chrome for Testing,
`Profiler` and `Performance.getMetrics`, a pinned world `?mode=strange&game=7`):

- the host was drawn again for every word a world said, about 21 a turn, and
  each drawing measured the page. The split and the highway are now drawn once
  a frame; what the worlds do next is still worked out at once (`strangeStep`);
- each world framing its own seat's ground at the start of a turn sent its
  camera to every other world, which drew its map again. Only a look the
  person gave a world is shared now, and each world frames its own ground;
- each world drew and measured what its box never shows: the debugger's bar,
  the civilizations' panel, the corner boxes' heights and the fitting of type
  in hidden panels. A world's page no longer does (`STRANGE_PANE`).

Together the thread went from about 268 to about 160 ms a turn and the turns
from about 300 to about 230 ms, with the same worlds on the same turns.

Then the worlds' own motion. At four or five turns a second a turn's units,
floating numbers, labels and camera flight never finish before the next turn
begins, so every world kept its animation running all the time and drew its
map seven or eight times a turn and its chart two or three. While the worlds
move, a world now draws a turn once, when it is handed it, and its camera is
at once where it is going (the page's reduced-motion path); standing still it
moves as any page does, and a person moving a world's map keeps its frames.
Paired runs, each measured against the page before it: three worlds 107 -> 85
and 136 -> 100 ms of the thread a turn; four worlds 182 -> 129 ms (83% -> 64%
of the thread) and the turn 220 -> 202 ms. From three worlds on the turn
waits on the worlds' engines again, not on the page.

Then a played seat's chores at the top of each turn, for a box nobody plays
by hand: picking the unit that wants orders and flying the map to it
(`advanceToNextActionUnit`, a second draw of the map every turn), and the
leader ribbon and pause control a box never shows (`drawSoloHud`,
`paintLivePause`, `placeLivePause`). Profiled with six worlds they were about
30 of the 76 ms a turn the worlds' pages spent. Paired with four worlds and
the futures on (the 4-player Pangaea Doctor Strange opens on): 5.04 and 4.90
turns a second before, 5.61 and 5.96 after, the thread 75-76% busy before and
65-68% after. Six worlds kept pace with three (4.5 turns a second, every
world's future 11-12 turns ahead) with the thread 71% busy before this.

These were measured in pairs, the page before and after one after the other
on the same pinned world, because the computer's other work moves a single
reading by a fifth: a change that looked like a third off once came to nothing
in pairs.

A branch is a new world's page, and a page is the whole viewer again: it took
about half a second of the shared thread to make, so a branch waited for it
(0.6 to 0.8 s from the pick until the new world stood on its turn at turn
60) and, with time running, so did every other world. Now a page for the next
branch is made ahead of time, out of sight, once the worlds have stood still
for a second and a half (`strangeSpare`); the branch opens on it as on a box
closed before, and only the copy of the world it branches from is left to
wait for: 0.27 to 0.42 s at turn 60. One page waits at a time.

### Autosearch: a path found is a branch

Operator 2026-11-25: "add the autosearch stuff to doctor strange's civ 6. we
should be able to efficiently search paths to try to find a desirable path".

A path is what a branch's name says, which player strategy held the seat
from which turn, and that is what [Autosearch](#autosearch) searches for. So
it is the same search, from the same panel. Its button is on the deck under
Branch, a size larger than the deck's other buttons but Play Time and in
gold, and lit with the paths played counted while a search runs. The panel
is the AI Debugger's own, shown over the tree beside the deck's card (the bar
it belongs to stays unseen); it is moved by its title like any of them.

It searches from the base timeline, the world followed and picked over
Branch, on its open turn. Each world plays on an engine of its own, so the
copies are made of that world's (`POST /forks` naming it) and every request
to them names it; each is asked to keep its agents with its turns
(`remember`), as a half of a split is. No world is asked anything while it
runs, and no world's future is begun beside it (`strangeForesee`): the
cores it was given are its own, and the futures carry on when it is over.

**Branch it** makes the best path a branch: a new world, a copy of the base
timeline on the turn the search began on, with the path's first strategy in
the seat. It plays the path from there as the copies did (`debugPlayPlan`,
each stretch seating its strategy), and says what it came to beside what the
search read ("SCORE 71, as found"). Its name is the path's
(`JackOfAllTrades-40-MoonshotMaven`), its box is in a colour of its own and
the sign over its far end says how the game goes on from its last turn.
Locked, the other worlds then play on to that turn too, each with its own
player strategy, so all of them stand on one turn again with the path beside
them, as a half locked to one that plays a found line does. The branch is
not walked back to the turn it was made on: a world walks forward only along
turns its seated strategy played, so a path of several would be played
again from its first handover. A search is of one world on one turn, so the
button waits for that world to stand on that turn; nothing beats a base
timeline playing its own strategy alone, it says so and makes no branch.

### Black swans

A black swan is an event nothing in the world saw coming. No rule of the game
rolls one and no Civ AI can order one: it is struck from outside the game, on
the world as it stands, by the `black_swan` operation (`Game::black_swan`,
`src/game/black_swan.rs`). Which one it is and where it lands are drawn from
the seed it is struck with and from nothing else, so one seed strikes two
copies of a world alike. Each writes the world with the consequences the
rules already have for what it does.

| Swan | What it does |
|------|--------------|
| Instant nuclear war (`nuclear_war`) | Every civilization is at war with every other, whatever friendship or treaty stood between them. Each fires a nuclear device on the largest city, not yet struck, of each of its two strongest rivals (a thermonuclear one where it knows Nuclear Fusion). A detonation is the blast of a launched device: fallout over the radius, half of the city's people, its walls. Each is on the strike ledger, so the map shows it and a forecast names it. |
| Pandemic (`pandemic`) | Every city in the world loses half its people. A city of one keeps its one. |
| Radiation leak (`radiation_leak`) | Four cities around the world, drawn apart from one another, start leaking. Each loses a citizen and takes a reactor accident's damage, the ground a tile around it is contaminated at once, and the contamination spreads a ring a turn for 4 turns, to 5 tiles out. Ground it reaches pays nothing for 20 turns. |
| Leader collapse (`leader_collapse`) | The leader of the civilization with the highest score falls. Its government waits out 10 turns of Anarchy, and no city of it keeps more than 25 Loyalty (50 in its capital). Which cities hold and which declare independence is the Loyalty rules' to say from there. |
| Microstates (`microstates`) | A civilization with more than one city, drawn among those there are, breaks up. Every city but the one it is governed from becomes a Free City, which the whole world is at war with, stands up four melee units and holds one nuclear device. On each of the next 40 turns a microstate that is still a Free City and still holds its device fires it with odds of one in five, on the most populous city of a living civilization within 12 tiles. One that is taken, or comes home, has no arsenal. |

A swan that this world has nothing for is refused and says why (no second
city to break away, one civilization left). One drawn at random is drawn
among those that are not.

Two of the five are not over when they land, so a struck swan is part of the
game and of its save (`Game::black_swans`): the turn's `process_black_swans`
spreads a leak's next ring and rolls for each armed microstate. In a world no
swan has struck it writes nothing and draws nothing from the world's random
stream, and that world's save does not carry the field, so every game played
without one is the game it was.

Striking one moves the world's random stream on by one draw. That is what
tells a struck world from the one it was: the print of the open turn's world
reads the stream (`world_print`), so a timeline that was one world with this
one up to here has parted from it on the turn before. The debugger marks the
open turn again as the world it now is and lets go of the turns kept ahead of
it, which were played in a world this did not happen in. Who played into the
turn still did. What the seat had done in the turn stands and is no longer
taken back: the turn now begins where the swan struck. Gone back to an
earlier turn and walked forward along the kept turns, the world on the swan's
turn is the world it struck; played forward again from an earlier turn, the
turn is played over and no swan has struck it.

## Edge of Tomorrow

Operator, 2026-11-25: "above civ 6 and below ai debugger add: Edge of
Tomorrow. This should be civ 6 with a jump back (like the concept from the
movie)".

`?mode=tomorrow` is the played game once more, listed between AI Debugger and
Civ 6 in the site list. The seat is in the person's own hands, on the
Civilization VI screen, and the page adds one thing: a jump back. The world
goes back to the turn the person woke on, exactly as it stood, and they play
it again knowing what they saw. Each time round is a loop, and the loops are
counted. `beta/shim.js` seats the visitor for `mode=tomorrow` as it does for
`mode=play`, and the last section of `web/assets/app_debugger.js` is the
rest. Nothing in the engine is new.

Operator, 2026-11-25: "2p Duel maps for edge of tomorrow being built out.
Goal of Edge of Tomorrow is to try to 1v1 beat an AI given a back button
(which effectively reveals the map)". On the published build the page opens
on a Duel: the person and one Civ AI, on the Duel-sized world two seats make
(44 by 26), laid flat, on Continents, Civilization VI's own first map type
(`MODE_WORLDS` in `beta/shim.js`). A link's own `players`, `map` or `shape`
still wins, and new games keep the world on screen. On the desktop build,
`civvis play --players 2 --map continents --shape flat` is the
Duel.

- **The page is the Civilization VI screen from its first frame** (`SOLO_UI`),
  whatever view the AI Debugger was last left in, and does not keep that
  choice for the AI Debugger to find. The debugger's own bar is out of sight
  (`html.tomorrow #debugbar`) and is not measured (`debugBarUnseen`). No Civ
  AI is seated when the game opens: the seat's Auto-play stays in the left
  panel, where a played game has it, with the Civ AI last picked.
- **The bar** stands where the debugger's does, across the top of the window
  from where the map area begins, with the map area beneath it (`--tmr-left`,
  `--tmr-top`). It has the name and the loop this is, a line that is the
  whole game, Jump back and Wake here. With little room the line goes under
  the other two.
- **Jump back** asks the engine for the turn woken on (`op: "jump"`), so
  nothing is played again to get there. The engine is deterministic and keeps
  the agents beside each mark (`Minds`), so the rivals play the same game
  again until the person plays a different one. The turns that were ahead
  stay marked, as after any step back, until the first turn played replaces
  them.
- **A lost game jumps back by itself.** When the seat's civilization falls,
  or the game is over and somebody else won it, the page's result card would
  offer another game and count down to one. Here that count is called off
  (`clearFinaleCountdown`) and the card is the loop's: how it ended, the turn
  the world goes back to, a count of `TOMORROW_COUNT` seconds to the jump,
  and Look around first, which stops the count and puts the card away until
  Jump back is pressed in the bar. The page draws its own card again
  whenever the result's signature changes, so the loop's card is put back
  after every state (`tomorrowCard`). A game the seat won keeps the page's
  own card, which then says which loop won it.
- **What was seen stays on the line.** The line runs from the first turn to
  the last the game may have. On it are the turn woken on, the open turn,
  how far any loop has reached, where each earlier loop ended (✕ a lost one,
  ↺ one jumped back from by hand) and, ahead of the open turn, what earlier
  loops saw done to the seat: a war declared on it, a city of its taken or
  razed, a nuclear strike, its conquest. Those are read from the game's own
  record of its wars (`state.wars`, each war's `highlights` whose `subject`
  is the seat), not from the event log, which is mostly the seat's own
  doing. The bar says the next of them in words.
- **Wake here** moves the turn woken on to the world as it stands. That
  world is kept as a save point of the engine's under the page's one name
  (`op: "save_point"`, `TOMORROW_SAVE`), which is never thinned away as an
  old turn's mark may be, and a jump back goes to it (`op: "goto_save"`).
  Pressed while standing on a wake point that was moved, the button moves it
  back to the first turn (`op: "drop_save"`), so one set by mistake is not
  for good. A wake point whose save point the engine no longer holds is the
  first turn again.
- **A step back takes attendance again.** The banners over the map compare
  each turn with the one before it (`rivalMoments`, `boostMoments` in
  `app_moments.js`). Going back from a war to before it began announced a
  peace, in this page and in the AI Debugger's Single player view alike;
  `debugForgetStandingOrders` now has them start over on the turn gone to,
  and takes down the banners still up or waiting, which are of the turn
  that was left.

The loops, the wake point and what was seen are this page's own count over
the timeline every played game keeps. They are gone with the page, as save
points are, and another game begins at loop 1.

`beta/debugger_test.py` (`check_edge_of_tomorrow`) drives all of it in a
browser against the toy engine.

## `POST /autosearch`

The climb, for a page that plays its lines on copies of the engine
(`autosearch::route`; the browser build answers it, from the page's own
engine). `op` is one of `begin` (a new search: the four lists as
`objective`, `from`, `end` or `turns`, `letters`, `seated`, `forks`), `next`
(the line for a copy to play, given the line it holds as `holding` and the
turn it stands on as `at`, with `from`, the turn they part on, and `done`
when there will be none), `took` (`plan`, `series`, the `terms` of each turn
it played, and `last`), `failed` (`plan`), `offer` (`plans`, lines put in
from outside the climb), `report` and `end`. Every answer but `next`'s is
the report: the best line, the seated Civ AI's line alone, the front, each
Civ AI alone, and how many lines meet every Need.

## `POST /debug`

`{"op": ...}` and the operation's fields. The answer is the state document
after the operation, with `error` set when it was refused; a refused
operation leaves the world as it was.

| `op` | Fields |
|------|--------|
| `back`, `forward`, `restart`, `undo` | none |
| `jump` | `turn` |
| `save_point` | optional `name` |
| `goto_save`, `drop_save` | `id` |
| `room` | `megabytes`: where the memory line is, 128 to 3072 |
| `step`, `step_action`, `assist` | optional `strategy`, the Civ AI to seat first |
| `play` | `turns`, 1 to 100: that many `step`s, asked for once; optional `strategy`. It stops early where a run stops (a breakpoint, the end of the game, a Must avoid broken for good), and the answer carries `readings`: how the seat stood as each turn it played opened, in the keys of a lean answer's `seat`; and, once an `objective` is set, `terms` and `killed` |
| `walk` | optional `turn`, the furthest to go; optional `era`: true to stop where a new era begins; optional `strategy`. Goes on along the kept turns the seated Civ AI would only play again, plays nothing, and is never refused: with none to take the world stays where it is |
| `status` | optional `strategy`: seat that Civ AI and play nothing, which is how the page says a pick |
| `foresee` | `turns`, 0 to 100; optional `strategy`, `anew`, `known`. Plays the world's future on towards the end of its game, that many turns of it, on a copy the world keeps for it (`Seer`): the world at the end of the seat's line, with the agents that play on from there, which keeps none of the turns it plays. The world and its kept turns are left as they stand. The first request makes the copy and each one after plays it further; it is made again when it is no longer this world's future (another Civ AI seated, a turn of the line that is another world than the one foreseen, something done in the open turn it was made on) or the request says `anew`. The answer carries `foresight`: `from` and `base` (the turn it was made on and that world's print), `seat`, `turn` (how far it has been played), `done` (the game has ended in it), `limit`, `played`, `worlds` (the print of each turn it opened, as `debug.worlds` gives a mark's), `eras`, `seen` (each detonation and each war declared, on record when it was made or seen since, and how the game ends: `turn`, `kind` of `nuclear`, `war`, `won` or `over`, `text`, `era`, `era_name`), `detonations`, `victory` and `anew` (this request made it). `known` is the turn the page was last told it stood on: `worlds` and `seen` then leave out what it was told before, and `since` says so. No turns only reads. Refused in the middle of a round |
| `black_swan` | optional `kind`, one of `debug.swan_kinds` by its `id`; optional `seed`. Strikes a black swan on the world as it stands (see "Black swans"): the one named, or one drawn by the seed among those the world has something for; a request with no seed is given one off the world's own random stream. The open turn is marked again and the turns kept ahead of it are let go of. The answer carries `black_swan`: its `turn`, `kind`, `name`, `text` (what happened, with its numbers), `sites`, the `player` it struck where it struck one, `until` and whether it is still `ongoing`. Refused in the middle of a round, and where the world has nothing for the swan named |
| `step_seat`, `finish_round` | none |
| `reveal` | `on`: true or false |
| `branch` | none. Lets go of every turn kept ahead of the open one: what a half of a split that is a new branch is asked, so that it begins on the turn it was made on |
| `objective` | `objective`: the four lists (`{"need", "avoid", "want", "dont_want"}`, each a list of `{"seat", "key", "test", "value", "when", "turn"}` or `{"seat", "key", "weight"}`), or `null` to clear them. Each term is read back off the world first, and one the seat has no number for is refused. From then on a `play` answers with `terms`, what each turn read of each list, and `killed`, the turn a Must avoid broken for good ended the play on (see "Autosearch") |
| `remember` | none. A half of a split keeps its agents with every turn it marks from here on, and none with the turns it has marked already: what a fork made to search asks of the half it will play (see "Autosearch"). The game itself keeps them already and is asked nothing |
| `copy` | from a half of a split: `from`, the other half's name |
| any, from a half of a split | optional `twin`, the other half's name: do it once for both if the two are the same world (see "Identical halves are played once") |
| `adopt` | from the host of a split: `pane`, the half to keep |
| `layers` | `layers`: the ids to switch on; all others are switched off |
| `breakpoints` | `turns`, `categories`, `phrases`, `important`, `every_seat`, `conditions`; whatever is left out is cleared |
| any | optional `lean`: true to be answered without the state document (see below) |

Every state document of a played game carries `debug`: the open `turn`, the
marked `turns`, `first` and `last` of the seat's own line, the print of each
of those turns' worlds (`worlds`, in the order of `turns`; see "One timeline
until the worlds differ"), the Civ AI seated
(`seat`, once one has been named), the kept turns ahead that another Civ AI
played (`beyond`: `from`, `to`, and `by`, who played the first of them; null
when there are none), whose play each stretch of the kept turns was
(`played_by`: `from`, `to`, and `by`, null for turns ended by hand), the
last turn ahead that playing on would only play
again (`played_ahead`, null when there is none), how many actions the seat has taken this
turn (`acted`), `can_undo`, `can_back`, `can_forward`, the `breakpoints` and
the categories they may name, the `layers`, how many decisions the layers
took this turn (`assisted`), why the last step `stopped`, what the last step
in did (`stepped`: the action and how many are `left`), whether the view
is `reveal`ed, the `memory_line` in megabytes if the page set one, and the
save points (`saves`: `id`, `name`, `turn`, `acted`,
and `auto` for the one the debugger keeps), the black swans that have struck
the seat's line as far as it is kept (`swans`, each as `black_swan` answers
it, `ongoing` as of the open turn) and the ones there are to strike
(`swan_kinds`: `id`, `name`, `about`). The answer to a request that
named a `twin` also says whether the twin was made a copy of it (`twinned`).

A watched game keeps no timeline and its state carries no `debug`.

An operation sent with `"lean": true` does exactly what it does without it
and is answered with a small document in place of the state: `lean`, `turn`,
`current`, `seed`, `finished`, `draw`, `winner`, `error`, the same `debug`,
`scores`, one of `id`, `alive` and `score` for every civilization, met or
not, and `seat`, the played seat's row of the standings: `turn`, `alive`,
`score`, `cities`, `population`, `gold_per_turn` and `yields` (`food`,
`production`, `gold`, `science`, `culture`, `faith`), the same numbers the
state document gives the seat in `players`. The state document is the map as the seat sees it, every city and unit
and everything the seat may do next. At turn 80 of a six-seat game on
published build `145fabd` it took 9 ms to build, of a jump's 20 ms and a
step's 130 ms. Something that plays a world forward only to read how it came
out asks lean and reads the scores.
