# Sim debugger

The Sim debugger is the played game (`docs/SINGLE_PLAYER.md`) with a
debugger's controls on it. Playing every decision by hand is its lowest
level; everything here is a layer of automation above that, or a way to move
through the game's own history.

The engine half is `src/server/debugger.rs`, served as `POST /debug` by the
native server and the browser build alike. The page half is
`web/assets/app_debugger.js`, the bar at the foot of the map.

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
stay whole, and between them the oldest turns go first, the odd ones before
the even: a very long game keeps its early turns one in two, then one in four,
and never fewer than one in eight. Past the memory line each further 32 MB
takes a tenth of the room away.

A thinned turn is not played again to bring it back. A turn played by hand
cannot be played by an agent, and an agent decides things its logged actions
do not carry, so the replay would be another world.

| Control | Operation | What it does |
|---------|-----------|--------------|
| ⏮ | `jump` to the first mark | Go to where the timeline begins |
| ◀ | `back` | The top of this turn if you have acted in it, otherwise the turn before |
| ↶ (`Ctrl`/`Cmd`+`Z`) | `undo` | Take back your last action this turn |
| slider | `jump` | Go to any marked turn; a turn with no mark answers with the nearest one before it |
| ▶ | `forward` | Forward along turns already played, without playing anything |
| ▸ | `step_action` | Step in: the seat's Civ AI takes its next action, and ends the turn when it has none left |
| ▶\| | `step` | The seat's Civ AI plays one whole turn |
| ▶▶ Run | `step`, repeated | Play turns until a breakpoint stops it, or Stop |
| ◉ | `reveal` | Show the whole map and every civilization, not only what the seat can see |
| ⤓ | `POST /save`, `POST /load` | Saves: keep this turn under a name, or load one that was kept |
| Split | | Two simulators from this turn; see below |

Going back forgets nothing. The turns ahead stay marked, and ▶ walks them
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

A run stops by itself when:

- one of the listed turns opens;
- the turn added an event in a chosen category (`EVENT_CATEGORIES`);
- the turn added an event whose text holds a chosen phrase;
- the turn added an event the engine marks important, if that is on;
- the game ends or the seat is eliminated.

Only the played seat's own events are read, which is what a person playing
it is shown. "Every civilization's events" reads the others too.

The stop is reported on `state.debug.stopped` and in the bar, with a Show
button when the event has a place on the map.

## Split screen

Split puts two simulators in one window, side by side or one above the other.
Both start from the world as it stood when the screen was split, and from
there each plays its own history: play one by hand and step the other, or
try two answers to the same turn.

Each half is the whole page again in a frame, with a simulator of its own:

- On the published build each page has its own engine. The half asks the
  host page for its world by message before it asks its engine anything
  (`beta/shim.js`, `pane-boot` / `pane-world`), so it never shows another
  game on the way.
- On a socket server both pages would reach the one session the process
  plays, so a request from a half is answered from a session kept for it
  (`src/server/panes.rs`). The half names itself in its address
  (`?pane=a-<split>`) and its requests carry that in `Referer`. Its session
  is a fork of the main one (`Session::fork`): the same world, the same
  agents, and the timeline behind it to walk back along.

The strip between the halves holds the split's own controls:

| Control | What it does |
|---------|--------------|
| Linked / Separate | Linked, the halves stay on the same turn. Whatever moves one is repeated in the other the same way: an ended turn ends the other's turn as it stands, a step steps it, a walk along the timeline walks it. Separate, neither follows the other. |
| ◧ / ⬒ | Switch between side by side and top and bottom |
| A → B, B → A | Make one half a copy of the other as it stands now |
| Keep A, Keep B | Close the split and play on from that half |

A half that moved because it was following is not followed back. Linking two
halves that have drifted apart brings B to A's turn.

On the published build a half starts from a save, so its timeline begins at
the split; on a socket server it keeps the turns before the split too. Either
way, the game that is kept when the split closes starts a timeline of its own.

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
| `reveal` | `on`: true or false |
| `layers` | `layers`: the ids to switch on; all others are switched off |
| `breakpoints` | `turns`, `categories`, `phrases`, `important`, `every_seat`; whatever is left out is cleared |

Every state document of a played game carries `debug`: the open `turn`, the
marked `turns`, `first` and `last`, how many actions the seat has taken this
turn (`acted`), `can_undo`, `can_back`, `can_forward`, the `breakpoints` and
the categories they may name, the `layers`, how many decisions the layers
took this turn (`assisted`), why the last step `stopped`, what the last step
in did (`stepped`: the action and how many are `left`), whether the view
is `reveal`ed, the `memory_line` in megabytes if the page set one, and the
save points (`saves`: `id`, `name`, `turn`, `acted`,
and `auto` for the one the debugger keeps).

A watched game keeps no timeline and its state carries no `debug`.
