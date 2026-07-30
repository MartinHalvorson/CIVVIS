# Unit livelock

A unit in a livelock looks busy. It moves every turn, every individual move is
the best one its scorer can see, and the turn after it undoes that move for
exactly the same reason. Nothing in a single turn's reasoning is wrong, so
nothing in a single turn's reasoning can catch it.

The auditor could not catch it either. `audit`'s unit check asks whether a unit
has *stopped* — `unit_still_since` reports a unit that has not changed tile for
twenty-five turns and is not fortified. A Scout pacing two hexes of coastline
for eighty turns changes tile every single turn and was invisible to it.

## What it cost

Six 200-turn six-player games on seeds 0–5, before any of this landed:

| | |
|---|---|
| unit-turns played | 156 201 |
| spent in a confirmed livelock | 8 047 (**5.15%**) |
| distinct episodes | 432 |

By unit, the episodes went: `warrior` 150, `scout` 86, `slinger` 60, `galley`
30, `builder` 23, `settler` 17, then a long tail. Explorers, warring units, and
builders — the whole spread, not one broken subsystem.

## Recognising one

A livelock is a property of a unit's history, never of a decision, so it is
judged once per turn from a window of the unit's own recent past
(`UnitMotion`, `src/ai.rs`). A unit is *circling* when all of:

- it has a full window of turns on record (`LIVELOCK_WINDOW`, six);
- those turns cover between two and `LIVELOCK_FOOTPRINT` (three) distinct
  tiles — two, because a unit that has *stopped* is the separate stall the
  auditor already reports, and three, because a unit reaching a fourth tile is
  covering ground and is judged afresh from there;
- nothing about the unit changed in any of them.

That last clause is what keeps working units out of it. A Builder improving two
tiles beside a city occupies exactly the footprint of a Builder stuck between
them; the difference is that it spends a charge. The *work mark* —
`(charges, xp, hp, promotions, album_sales)` — is every field that moves when a
unit improves, builds, spreads, fights, takes a hit, heals, promotes, or plays.
When it moves, the record restarts.

## Breaking one

Three responses, each taking over where the last one failed.

**Price the loop.** `livelock_penalty` charges `LIVELOCK_ESCAPE_VALUE` against
every tile of the footprint, the one underfoot included, so any tile outside
the loop is worth eight points more than any tile inside it. Both tactical
scorers add it (`BasicAi::tactical_step`, `AdvancedAi`'s force-group search).
Eight points buys about two hexes of positional error, and stays well under the
fifteen-plus a real threat scores: a stuck unit is redirected, never ordered to
its death.

**Refuse the retread.** `path_move` already declined to traverse an edge
backward within one turn — the same round trip spread over two turns was
invisible to it. It now also declines a step into any tile of a confirmed
footprint. There is nothing to trade off on a plain pathing step, so this one is
absolute; it lasts only until the window slides off the loop or the stand-down
below fires.

**Stand the unit down.** After `LIVELOCK_STAND_DOWN_AFTER` (two windows) the
tabu has had every chance and the unit is still going in circles, which means
whatever it wants is unreachable. The record is wiped, which is the part that
does the work: the tabu lifts and the unit re-plans for
`LIVELOCK_STAND_DOWN_TURNS` against a world that has moved on. It still takes
its whole turn every one of those turns; only if a turn produces nothing at all
does it dig in and fortify, which is strictly better than standing in the open
and is time it was going to spend standing there anyway. It must never run
*instead* of the unit's step — see below for what that cost when it did.

## Keeping the measurement honest

`audit` reports motion per game and in the totals. The first line is the whole
world; the next three hold the population constant by controller role:

```
motion all        unit-turns=25112 livelock=1361 (5.42%) idle-field=5333 (21.24%) picket=8078 (32.17%)
       major      unit-turns=...
       city-state unit-turns=...
       barbarian  unit-turns=...
```

`picket` — stood still, fortified, outside its own city — is there so that a
"fix" that merely converts circling into fortified stillness shows up as a
transfer rather than as a win. Read the three columns together.

Read the `major` line when judging a rated controller. City-state forces are
deliberately confined to a defense radius and barbarians have no empire to
develop; mixing them into the headline can make a controller look idle, or
make two runs incomparable merely because they spawned different numbers of
minor units. The `all` line remains for engine-wide capacity and regression
comparisons.

The harness accepts the same `--speed` profile as the deployment tools. Its
default turn limit and its war/peace duration checks are derived from that
speed's effective rules. For example, a deployment audit should say
`--speed online --turns 250`; a negotiated war lasting eight turns is illegal
under the ten-turn Standard minimum but legal when Online scales that duration
to eight.

## What it bought

The same six games, re-run with all three responses in place:

| | before | after | |
|---|---|---|---|
| livelock | 5.15% | **1.71%** | −3.44 pts |
| idle-field | 22.55% | 16.08% | −6.47 pts |
| picket | 33.43% | 37.25% | +3.81 pts |
| **all three** | **61.13%** | **55.04%** | **−6.10 pts** |
| unit-turns | 156 201 | 162 254 | |

A **67% reduction** in the share of the game spent going in circles. The bottom
row is the one that matters, because the first three trade against each other:
six points of unit-turns stopped being spent on nothing at all. Empires also
field about 4% more units over the same 200 turns, which is what less waste
looks like from the outside.

Episodes fell from 432 to 318, and the turns each one spends *past* the ten-turn
reporting threshold fell from 18.6 to 8.7. Nothing here stops a loop from
*starting* — a loop only exists once a unit has been in one for a window — so
most of what the three responses buy is breaking one about twice as fast once it
exists.

Neither run produced a rule violation, every game reached its turn limit, and
total symptoms per game fell from 130–144 to 93–122.

None of it costs anything measurable. The per-turn record is one six-entry deque
per unit, and the tactical scorers — which ask about every candidate tile of
every unit — read a verdict settled once when the window closed rather than
re-deriving it. Two paired `audit --games 2 --turns 120` runs came out at 7.6s
before and 7.5s after.

### What the picket column caught

The third response originally ran *instead of* a stood-down unit's step, holding
the unit and guessing — from an enemy-in-reach test and a foundable-ground test
— at what it might otherwise have wanted. Measured, that version moved `picket`
up 7.30 points while `idle-field` fell only 6.58: it was absorbing turns units
would have spent doing something, and the aggregate got **worse** than leaving
the stand-down out entirely.

| | livelock | idle-field | picket | all three |
|---|---|---|---|---|
| baseline | 5.15% | 22.55% | 33.43% | 61.13% |
| pre-empting the turn | 1.90% | 15.97% | 40.73% | 58.60% |
| digging in after it | 1.71% | 16.08% | 37.25% | 55.04% |

So it now runs after the unit's own step and only when that step produced
nothing — time the unit would have spent standing in the open regardless. The
record-wipe stays, because that was always the part doing the work. This is the
column's whole purpose: without it, the first version's 1.90% livelock share
would have read as a win.
