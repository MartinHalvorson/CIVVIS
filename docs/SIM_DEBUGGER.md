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
with its clones, so a mark costs the turn's difference. The newest 60 turns
keep a mark each and older ones keep one turn in ten.

| Control | Operation | What it does |
|---------|-----------|--------------|
| ⏮ | `jump` to the first mark | Go to where the timeline begins |
| ◀ | `back` | The top of this turn if you have acted in it, otherwise the turn before |
| ↶ (`Ctrl`/`Cmd`+`Z`) | `undo` | Take back your last action this turn |
| slider | `jump` | Go to any marked turn; a turn with no mark answers with the nearest one before it |
| ▶ | `forward` | Forward along turns already played, without playing anything |
| ▶\| | `step` | The seat's Civ AI plays one whole turn |
| ▶▶ Run | `step`, repeated | Play turns until a breakpoint stops it, or Stop |
| ⤓ | `POST /save` | Save this turn under its number |
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
| `step`, `assist` | optional `strategy`, the Civ AI to seat first |
| `layers` | `layers`: the ids to switch on; all others are switched off |
| `breakpoints` | `turns`, `categories`, `phrases`, `important`, `every_seat`; whatever is left out is cleared |

Every state document of a played game carries `debug`: the open `turn`, the
marked `turns`, `first` and `last`, how many actions the seat has taken this
turn (`acted`), `can_undo`, `can_back`, `can_forward`, the `breakpoints` and
the categories they may name, the `layers`, how many decisions the layers
took this turn (`assisted`), and why the last step `stopped`.

A watched game keeps no timeline and its state carries no `debug`.
