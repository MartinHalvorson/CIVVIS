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
whatever it wants is unreachable. It holds ground for
`LIVELOCK_STAND_DOWN_TURNS`, fortified — strictly better than another lap, and
it stops paying for a route search that keeps returning the same answer. The
record is wiped as it starts, so the retry afterwards is unencumbered and the
world it re-plans against has moved on. A stand-down suppresses a unit's own
plans and never its part in a fight: a unit with an enemy within two tiles has
something concrete to do, and takes its ordinary turn.

## Keeping the measurement honest

`audit` reports a `motion` line per game and in the totals:

```
motion    unit-turns=25112 livelock=1361 (5.42%) idle-field=5333 (21.24%) picket=8078 (32.17%)
```

`picket` — stood still, fortified, outside its own city — is there so that a
"fix" that merely converts circling into fortified stillness shows up as a
transfer rather than as a win. Read the three columns together.
