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
eras named on the timeline and Era start, save points and their flags, a
breakpoint picked from the timeline, a timeline with turns thinned out of it,
the Civ AI picker, Auto-play with its count and its Stop, the breakpoints
form with auto-play stopped by a turn, by a new era and by hand, layers,
reveal, the Single player switch, undo, and a round stepped a seat at a time.
It then splits the screen head to head, each half the same page with a toy
engine of its own: a Civ AI is picked for each half, from the strip and from
the head of its lane, both are played to a turn locked and then unlocked,
each half and then both are moved by the lanes in the host's box, and the
test reads which Civ AI each half's steps named, the numbers set side by
side, a copy, and the half that is kept. A last part holds the lock: that
Branch Timeline opens the split at once, that the lock stands between the two
lanes and closes only on one turn, that the host's deck is the one panel of a
locked split and plays both halves, that each half shows its own panel again
when the lock opens, that two identical halves are asked for once, and that
a lane shows its half's breakpoint turns and save points and goes to the one
that is clicked. A control added to the box or the strip is added there.

## The bar

The debugger's controls are one header box at the top of the window. It
begins at the right side of the left panel and runs to the window's right
edge, so the left panel keeps the window's whole height. The map area stands
beneath the box: where the map area begins and how tall the box is are
measured and published as `--dbg-left` and `--dbg-top`, and the map is fitted
again whenever either changes (the left panel folded away, brought back or
dragged wider). A watched game has no box and is not moved.

The timeline is the top of the box, and under it is one row in three parts.

In the middle, under the middle of the timeline, is the turn that is open, in
a box that also goes to a turn typed into it, with the game's last turn
beside it. The turn buttons stand either side of it, in this order:

- **Turn 1** goes to where the timeline begins;
- **Era start** goes back to the first turn of the era that is open, and from
  there to the first turn of the era before;
- **10** and **1** back;
- the turn box;
- **1** and **10** on. These walk the turns already played, and past the last
  of them the seat's Civ AI plays on. They do not play over what was played:
  ten on from turn 3 with nine turns played walks to turn 9 and plays four
  more. (Dragging the timeline ahead from an earlier turn is the way to play
  those turns again, and it says so under the pointer.)

At the left end is Auto-play, which used to be in the left panel: the Civ AI
that plays the seat, how many turns (1 to 250, or all; it is set to 10 to
begin with), and the button. It plays that many turns from the turn that is
open and stops early when a breakpoint does. While it plays, the button
counts the turns (`7/20`, or just `7` when it plays all) and is its own Stop.
A turn played to from the timeline or by the turn buttons is counted and
stopped the same way. Beside it are Layers, Breakpoints and **Branch
Timeline** (the split screen).

At the right end are the rest of the tools: step in, step a seat, undo, save
points, Reveal and the Single player switch.

The words on the tools are shown while both ends fit in two rows. When they
do not, the words on Turn 1 and Era start go first and then every tool's,
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
- save points as flags along the top edge and turn breakpoints as red marks
  along the bottom, each to click;
- under the pointer, the turn there and what letting go would do: "play to
  here", "play on from turn 7 to here" when the open turn is behind the last
  one played (the turns after it are played over), or the turn a thinned one
  would land on.

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
| Era start | `jump` | Back to the first turn of the era that is open, and from there to the first turn of the era before |
| ◀◀ 10 | `jump` | Back ten turns, or to where the timeline begins |
| ◀ 1 | `back` | The top of this turn if you have acted in it, otherwise the turn before |
| turn box | `jump`, or `step` repeated | Go to a turn by its number, the same way. Up and down in the box walk one played turn. Its tooltip says which turns were played and how many are still kept |
| 1 ▶ | `forward`, or `step` | Next turn: forward along the turns already played, and past the last of them the seat's Civ AI plays one whole turn |
| 10 ▶▶ | `jump`, then `step` repeated | Ten turns on: forward along the turns already played, and past the last of them the Civ AI plays the rest |
| timeline | `jump`, or `step` repeated | Go to any turn of the game. One that was played is put back as it stood; one that is no longer kept answers with the nearest before it, and the bar says so. One still ahead is played to by the seat's Civ AI, stopping early if a breakpoint does. The mouse wheel over it walks one played turn at a time, and the arrow keys move it a turn |
| Civ AI | | Which Civ AI plays the seat when a turn is auto-played, stepped or played to. It is the page's own picker (`#autoplaystrategy`), shown beside Auto-play |
| Turns, ▶▶ Auto-play | `step`, repeated | Play the turns picked (1 to 250, or all; 10 unless changed) from the turn that is open, stopping early at a breakpoint. The button counts the turns as they are played and is the Stop. Each turn is asked for the moment the one before it is answered, so the engine plays it while the page draws that one; a Stop pressed while a turn is being drawn lands a turn later |
| ↶ (`Ctrl`/`Cmd`+`Z`) | `undo` | Take back your last action this turn; the note says so, and how many more can be taken back |
| ▸ | `step_action` | Step in: the seat's Civ AI takes its next action, and ends the turn when it has none left |
| ▸\| | `step_seat` | Step one seat: end your turn as it stands, then one civilization's turn at a time |
| ◉ | `reveal` | Show the whole map and every civilization, not only what the seat can see |
| ⚑ | `save_point`, `goto_save`, `drop_save` | Save points: keep this world to come back to, go to one, or let one go. Each stands on the timeline as a flag at its turn; clicking the flag goes there |
| ⚑ | `POST /save`, `POST /load` | Saves, in the same panel: keep this turn on the device under a name, or load one that was kept |
| Branch Timeline | | Two simulators from this turn, side by side and locked on the same turn; see Split screen below |

Going back forgets nothing. The turns ahead stay marked, and 1 ▶ walks them
again exactly as they were played. They are replaced only when the world is
played forward differently from an earlier turn: by ending a turn by hand, or
by a step.

Undo replays the turn. It puts the turn's mark back and applies every action
the seat took except the last. The engine is deterministic and the random
stream is part of the game, so the replay is the same world.

A walked-to world gets fresh agents, as a loaded save does: their plans belong
to the future the world was taken out of. The page drops its own standing
orders (a unit's journey, a city's queue) for the same reason, so a turn that
was walked to is shown as it stood.

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

The stop is reported on `state.debug.stopped` and in the bar, with a Show
button when the event has a place on the map.

A listed turn stands on the timeline as a red mark. Clicking it goes to that
turn: back to it if it was played, and on to it if it is still ahead.

## Split screen

Branch Timeline puts two simulators in one window. It asks nothing first:
the screen splits at once, side by side, and the two halves begin locked on
the same turn (the strip between them can stack them instead). Both start
from the world as it stood when the screen was split, and from there each
can play its own history: play one by hand and step the other, or try two
answers to the same turn.

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
both halves, and its timeline becomes two lanes, A and B. Each lane is that
half's history: gold up to the turn of the split, the half's own colour from
there, and the half's open turn as its pill. A lane is the hand on its half,
and on both halves while they are locked. Letting go of it on a turn every
half concerned has played walks them there; on a turn ahead of any of them it
plays them there, each with its own Civ AI. At the head of each lane is the
Civ AI that half seats, to pick there.

A lane also carries what stands on its half's timeline, as the timeline of an
unsplit page does: a red dot on each breakpoint turn and a flag on each save
point. A half has no timeline of its own to show them on, so it tells the
host whenever they change (`pane-marks`), locked or not. Clicking a dot goes
to that turn the way letting go of the lane there does. Clicking a flag takes
that half to its save point; locked, the other half is brought to the same
turn. Unlocked, each lane's marks are its half's alone.

### The lock

A lock joins the two lanes, on a bracket that runs from the middle of one
rail to the middle of the other. It says whether the two halves are one.

- **Locked** (how a split opens): the bracket is whole and the lock is shut.
  The halves stay on the same turn. Whatever moves one is repeated in the
  other the same way: an ended turn ends the other's turn as it stands, a
  step steps it, a walk along the timeline walks it. What one is asked that
  leaves its turn where it was is asked of the other too: a layer, a
  breakpoint, Reveal, one action or one seat stepped, an undo, a save point.
  While both stand on one turn a link joins their two markers.
- **Unlocked**: the bracket is broken and the lock is open. Neither half
  follows the other.

Clicking the lock opens it. It closes only on two halves that stand on the
same turn, which is where it then keeps them; while they stand on different
turns it cannot be pressed, and its tooltip says why. (Walk one half's lane
to the other's turn first, or make one a copy of the other.)

Locked, the two halves are one thing to play, so there is one control panel:
the host's deck, under the lanes. Each half puts its own bar away and tells
the host what was on its deck (`pane-deck`). The host draws its deck from
half A's, and a press or a change on it is passed to A's own deck
(`pane-press`, `pane-change`), which does what it would have done had it
been pressed there; B follows. So every control on the panel means what it
means for one world, and applies to both. The Civ AI is not on the panel: it
is picked at the head of each lane, one for each half. Saves to the device
are one world's and are kept from a half's own panel, unlocked.

When two halves are locked after playing apart, B takes A's layers,
breakpoints and Reveal, so that the one panel is true of both.

Unlocked, the host shows the two lanes and no deck, and each half shows its
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
| A → B, B → A | Make one half a copy of the other as it stands now, turns behind it included (`copy`) |
| Keep A, Keep B | Close the split and play on from that half (`adopt`): its session becomes the main one, with every turn it could walk back to |
| drag the strip | Resize the halves; double-click makes them equal |
| VS | Head to head; see below |

A half that moved because it was following is not followed back.

### Head to head

Head to head is the split used to play two strategies out from one turn. VS
in the strip widens it to hold:

- a Civ AI for each half, the same choice that stands at the head of the
  half's lane in the bar. It is the agent that plays that half's seat
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

The rivals in each half are played by fresh agents from the split onward, as
they are after any walk along the timeline, so the two halves differ only in
what the seat does and in what follows from it.

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
| `POST /forks` `{"count": N}` | N forks (0 to 16) of the engine as it stands now, in place of any there were. Answers `forks`, `memory_bytes` (one engine's memory) and `ms`. A count of 0 lets them all go. |
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

## `POST /debug`

`{"op": ...}` and the operation's fields. The answer is the state document
after the operation, with `error` set when it was refused; a refused
operation leaves the world as it was.

| `op` | Fields |
|------|--------|
| `status` | none |
| `back`, `forward`, `restart`, `undo` | none |
| `jump` | `turn` |
| `save_point` | optional `name` |
| `goto_save`, `drop_save` | `id` |
| `room` | `megabytes`: where the memory line is, 128 to 3072 |
| `step`, `step_action`, `assist` | optional `strategy`, the Civ AI to seat first |
| `step_seat`, `finish_round` | none |
| `reveal` | `on`: true or false |
| `copy` | from a half of a split: `from`, the other half's name |
| any, from a half of a split | optional `twin`, the other half's name: do it once for both if the two are the same world (see "Identical halves are played once") |
| `adopt` | from the host of a split: `pane`, the half to keep |
| `layers` | `layers`: the ids to switch on; all others are switched off |
| `breakpoints` | `turns`, `categories`, `phrases`, `important`, `every_seat`, `conditions`; whatever is left out is cleared |
| any | optional `lean`: true to be answered without the state document (see below) |

Every state document of a played game carries `debug`: the open `turn`, the
marked `turns`, `first` and `last`, how many actions the seat has taken this
turn (`acted`), `can_undo`, `can_back`, `can_forward`, the `breakpoints` and
the categories they may name, the `layers`, how many decisions the layers
took this turn (`assisted`), why the last step `stopped`, what the last step
in did (`stepped`: the action and how many are `left`), whether the view
is `reveal`ed, the `memory_line` in megabytes if the page set one, and the
save points (`saves`: `id`, `name`, `turn`, `acted`,
and `auto` for the one the debugger keeps). The answer to a request that
named a `twin` also says whether the twin was made a copy of it (`twinned`).

A watched game keeps no timeline and its state carries no `debug`.

An operation sent with `"lean": true` does exactly what it does without it
and is answered with a small document in place of the state: `lean`, `turn`,
`current`, `seed`, `finished`, `draw`, `winner`, `error`, the same `debug`,
and `scores`, one of `id`, `alive` and `score` for every civilization, met or
not. The state document is the map as the seat sees it, every city and unit
and everything the seat may do next. At turn 80 of a six-seat game on
published build `145fabd` it took 9 ms to build, of a jump's 20 ms and a
step's 130 ms. Something that plays a world forward only to read how it came
out asks lean and reads the scores.
