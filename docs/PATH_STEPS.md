# The path through one game

How many steps one game of CIVVIS is, how big that is, and how few steps it
could be.

The project owner asked for it on 2026-11-24: "we should try to reduce the
path of civvis through civ 6 down to the computatoinal irreducability and
should have a chart tracking the size/num of steps."

The chart is on the
[CIVVIS Performance page](https://civvis.ai/charts/civvis-performance/#perf-path).
The readings are `data/performance/path_steps.json`, and
`tools/path_steps.py` takes them.

This page is about the path. The other half of the same question, the
smallest state a game can be played from, is
[`docs/CORE_STATE.md`](CORE_STATE.md).

## What the path is

A game is one function applied over and over: `next(world, action) -> world`.
The **path** is the list of actions it was applied to, from the first turn to
the end. The engine keeps that list as it plays (the action log), so the path
is counted, not estimated.

A **step** is one action in the log: a unit moves one hex, a city is told
what to build, a seat ends its turn.

A step is **reducible** if the game reaches the same final world without it,
either dropped or folded into its neighbours. The irreducible path is what is
left when no step can go. Nobody can compute that minimum for a game this
size. What can be done is to find a class of steps that can go, show by
replay that the world comes out the same, remove it, and count again. The
chart tracks that count.

## What a reading counts

`tools/path_steps.py` plays whole games with `civvis core-probe record`: six
players on Continents at Online speed, 250 turns at most, the stock agent on
every seat. The seeds are fixed (the games-per-minute benchmark's), so two
builds are counted on the same worlds. A reading is the mean over its games.

| Figure | What it is |
| --- | --- |
| Steps | The actions in the log. |
| Bytes | The log written down, each action as compact JSON. |
| Walks | A unit's consecutive one-hex moves inside one seat's turn are one walk. |
| A walk as one step | The steps there would be if every walk were one step: `steps − one-hex moves + walks`. |
| Instructions to play | What the processor retired to play the game, agents and rules together. |
| Instructions for the path alone | The same actions applied to the same opening world with no agent in the loop. |

The path is the same on any machine for a given build and seed. The
instructions and the seconds belong to the machine. Instructions do not move
with how busy the machine is, which seconds do.

The path alone is measured with `civvis core-probe ablate` stopped at once.
That is two replays of the whole game and nothing else, so the figure is half
of what it retires. The probe holds the first replay against the world the
game itself ended on. A reading is recorded only if no action was refused and
nothing differs but the agents' own notes among the players' counters
(`commit:made`, `raid_wars` and the like), which the agents write outside any
action. Which keys are notes is the engine's own list, `is_agent_note` in
`src/game.rs`, and the tool reads it from the source.

## The first reading

Taken 2026-11-24 on build `f55bc35436`, the mean of three games.

| Figure | Reading |
| --- | ---: |
| Steps in one game | 75,273 |
| Turns | 249 |
| Steps a turn | 302 |
| Written down | 2.98 MB, 40 bytes a step |
| One-hex moves | 56,442, in 31,478 walks |
| A walk as one step | 50,309 steps, 33% fewer |
| Instructions to play | 196 billion |
| Instructions for the path alone | 37 billion, 19% of playing it |

Three quarters of the path is units walking one hex at a time:

| Kind of step | Steps | Share |
| --- | ---: | ---: |
| One-hex moves | 56,442 | 75% |
| End of turn | 3,747 | 5.0% |
| Fortify | 3,437 | 4.6% |
| Choose what a city builds | 1,802 | 2.4% |
| Slot a policy | 1,271 | 1.7% |
| Take a policy out | 1,187 | 1.6% |
| Ranged attacks | 871 | 1.2% |
| The other 54 kinds | 6,518 | 8.7% |

The three games ran 80,347, 66,935 and 78,536 steps, so one game is not a
reading. Two of the three ran to the turn limit.

## What can go, in the order the reading shows it

Nothing has been removed yet. This is the plan the first reading supports.

1. **A walk as one step.** A third of all steps are the second and later
   hexes of a walk already begun. The engine already has a go-to action
   (`move_to`), and the agents use it for under one step in a hundred. Each
   hop reveals fog and can meet a zone of control, so whether one go-to ends
   on the same world as its hops has to be shown by replay before the agents
   switch. Worth 24,964 steps a game.
2. **Steps that change nothing.** A fortify order to a unit already
   fortified, a policy slotted and taken out in the same turn. The log cannot
   show these: it needs the engine to compare the world before and after a
   step. That is the next thing to add to the probe, and it gives the first
   real count of steps that can simply be dropped.
3. **Policy cards.** Slotting and taking out are 2,458 steps a game, about
   ten a turn. Some of that is a seat rebuilding its whole government to
   change one card.
4. **End of turn.** One per living seat per turn, majors and city-states
   alike. This is the skeleton of the game and it stays.

The instructions tell the other half. Four fifths of the work of a game is
the agents deciding what to do, and one fifth is the rules doing it. So the
path itself costs about 37 billion instructions, and fewer steps and cheaper
rules lower that. The deciding is a separate lever: it is what the speed work
in [`docs/SIMULATOR_PERFORMANCE.md`](SIMULATOR_PERFORMANCE.md) pushes on.

## Taking a reading

```bash
cargo build --release --bin civvis
tools/path_steps.py --binary target/release/civvis            # measure and print
tools/path_steps.py --binary target/release/civvis --record   # and add the reading
```

Three games take about a minute and a half on one core. `--games N` plays
more. `--summarise DIR ...` counts games already recorded with
`civvis core-probe record`.

A recorded reading needs the build's commit (the binary's source tree, or
`--commit`), the standard seeds, and replays that come out the same. Take one
after any change meant to shorten the path, and whenever the agents change
how they move.

On macOS the instructions come from `/usr/bin/time -l`. Elsewhere a reading
has the steps and the bytes and leaves the instructions out.
