# Play Firaxis Civ 6 with computer control

The lobby's Game mode select offers two worlds, and both of them are ours: an
AI-only simulation and a single-player game, played in the engine in
`src/game.rs`. This is the third entry, and the world it starts is not ours at
all — it is a real game of Sid Meier's Civilization VI, running on this
computer, with CIVVIS occupying a seat and playing it.

`tools/civ6_play.py` has been able to do that since #541. What it has never had
is a relationship with the screen a game is set up on: it is a terminal command
with its own vocabulary, and a person who has just chosen a difficulty and a map
in the lobby has to translate both by hand into `--difficulty
DIFFICULTY_EMPEROR --map Continents.lua` and run it somewhere else. The mode is
that translation, made selectable.

## What kind of mode it is

The operator's framing, 2026-07-31: *"a mix of single player and AI simulation
in a way"*. That is exactly right, and it decides every question below.

It is **single player** in the way that matters to the game: there is one human
seat, it is ours, and the difficulty setting is real. Civilization VI hands its
handicap bonuses to *human* seats only — which is why `docs/GROUNDING.md`'s
autoplay measurements cannot say anything about difficulty at all, and why the
ladder in `docs/CIV6_COMPUTER_CONTROL.md` is only climbable from a seat. Taking
that seat is the entire point.

It is an **AI simulation** in the way that matters to the person watching:
nobody is at the keyboard. You do not end turns, choose production, or move a
unit. You pick the game and watch CIVVIS play it, exactly as you watch a
spectated simulation.

So the settings follow single player — a difficulty is chosen, and it means
something — and the controls follow the simulation: there is nothing to press
once it starts.

## The settings are Civilization VI's, not ours

Selecting the mode re-points the setup panel at the other game's vocabulary,
because that is the game being configured. Three of the four settings that
survive the switch are ones Civilization VI and CIVVIS already agree on, which
is not a coincidence: `src/setup.rs` was built from Civilization VI's own tables.

| Lobby setting | Civilization VI | Carried by |
| --- | --- | --- |
| Difficulty | `DIFFICULTY_SETTLER` … `DIFFICULTY_DEITY` | `--difficulty` |
| Map | the registered map scripts, e.g. `Continents.lua` | `--map` |
| World size | `MAPSIZE_DUEL` … `MAPSIZE_HUGE` | `--map-size` |
| Game speed | `GAMESPEED_ONLINE` … `GAMESPEED_MARATHON` | `--speed` |
| Turn limit | — | `--max-turns`, from the speed |

The map list is **replaced**, not filtered. Civilization VI's roster and ours
overlap but neither contains the other: Grand Canals is a CIVVIS world with no
counterpart in the other game, and Shuffle, Terra, Primordial, Splintered
Fractal and Tilted Axis are Civilization VI worlds with no counterpart here. A
choice that exists in both — Continents, Pangaea, Lakes, Inland Sea, Island
Plates, Small Continents, Archipelago — survives switching modes; one that does
not falls back to Continents, which is the game's own default and the first row
of its list.

The map roster is not read from the installation. It is the `Maps` rows of
`Base/Assets/Configuration/Data/StandardMaps.xml` plus
`DLC/Expansion1|2/Config/*_StandardMaps.xml`, transcribed with their
`SortIndex` order into `src/civ6.rs` and checked against the `.lua` files
present on this install by `tools/civ6_setup.py`. Reading them live would make
the lobby's contents depend on a game being installed on whatever machine
serves the page, which is exactly backwards: the mode is offered, then refused
with a reason.

Everything else in the panel is hidden while the mode is selected, because
nothing carries it:

- **Leader** — the controller cannot choose one. Civilization VI's Create Game
  screen has the dropdown, but the run is configured through
  `MapConfiguration`/`GameConfiguration` from `CivvisControlSetup.lua`, which
  sets ruleset, map, size, difficulty and speed and nothing else. The seat's
  civilization is whatever the game deals, every run. `--fixed-seed` does not
  change this ([[civvis-civ6-fixed-seed-is-inert]]): map, start *and* civ are
  random per run.
- **World shape**, **thermal distribution** — CIVVIS map-generator settings
  with no equivalent. Civilization VI wraps east-west and that is all.
- **Teams**, **victory conditions**, **start era**, **leader pool** —
  Civilization VI has all four; `CivvisControlSetup.lua` sets none of them, so
  offering them here would be a lie about what the run will be.

Each one is a row that could be added later, and adding it means teaching the
setup mod, not the lobby.

## The mode can be refused, and says why

Every other mode in the list works on every computer that can open the page.
This one needs Civilization VI installed *on the machine serving it*, needs
this repository's `tools/` beside the binary, and needs the game not already
being driven by somebody else. So the mode is always offered and the start is
answered with a reason:

- **`GET /civ6`** reports what this host can do — where the installation is,
  where the controller is, and who holds the game right now.
- **`POST /civ6/start`** either starts a run or refuses with one sentence.

Refusing loudly is the whole design, and it is a lesson rather than a
preference. A dead Steam client burned eleven of twenty-four ladder attempts,
each one of them recorded as a *loss* rather than as an attempt that never
happened ([[civvis-civ6-a-blocked-attempt-is-not-a-loss]]), and a two-city
ceiling that looked like a strategy defect for days was a controller whose
refusals nothing ever read ([[civvis-civ6-host-refusal-feedback]]). A mode that
silently does nothing when Steam is down would reproduce both.

The refusals, and what each one means:

| Refusal | Cause |
| --- | --- |
| `Civilization VI is not installed on this computer` | no install at either standard path and no `$CIV6_INSTALL` |
| `the CIVVIS tools directory is not beside this server` | binary served from somewhere without `tools/civ6_play.py`; set `$CIVVIS_TOOLS` |
| `python3 is not on this server's PATH` | the controller is Python |
| `<tag> already holds the game (pid <n>, since <t>)` | one writer at a time — `tools/civ6_control/gamelock.py` |

The lock is the important one. There is a single installation, a single mod
directory inside it, a single log file and a single process, and two harnesses
driving that do not conflict loudly — they conflict silently, and the result
reads as a flaky game. `gamelock.py` exists because that happened. This mode
reads the same lock directory before it starts anything, so a run started from
the lobby cannot walk over a run started from a terminal, and the lobby says
which run is in the way rather than failing to start.

## What a started run is

`POST /civ6/start` spawns

```
python3 tools/civ6_play.py --tag civvis-<UTC stamp> \
    --difficulty <D> --map <script> --map-size <SIZE> --speed <SPEED> \
    --max-turns <N>
```

detached, with its output in the run directory under
`~/civvis-civ6-runs/control/<tag>/`. The server does not wait for it: bringing
the game up takes about three minutes on this install, and a browser request is
not the right thing to hold open for that. `GET /civ6` reports the run's
progress from the same directory the controller writes it to, so the lobby
tracks a run it started and a run it did not equally well — which is what
happens the first time somebody starts one from a terminal out of habit.

Killing the server does not kill the run. That is deliberate: the run holds the
game lock and drives a window for hours, and tying it to the lifetime of a page
somebody might close is how you lose a Deity attempt at turn 180.

## What this does not do yet

**The viewer does not show the real board.** Starting a run from the lobby
starts a real game in the real window; the CIVVIS page it was started from still
shows whatever world it was showing. Mirroring the board into the CIVVIS viewer
is a separate line of work — `src/mirror.rs`, PR #683 — and this mode is the
lobby half of the same feature. When both are in, selecting the mode and
pressing start gets you the real game on the left and CIVVIS's reading of it on
the right.

Until then the mode is worth having on its own: it is the difference between a
ladder attempt being a terminal command somebody has to remember the flags for
and a game you can set up and start on the screen you set up every other game
on.

**Nothing is rated.** A Civilization VI run is not a CIVVIS game, so it produces
no `matches.csv` row and no Elo. `docs/CIV6_LADDER.md` is where a run's outcome
is recorded, and it is recorded by hand.
