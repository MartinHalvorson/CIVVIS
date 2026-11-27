# Clock margin: the graded live measure of the victory race

Three in four Deity losses are a rival's Science Victory, landing at turn
p10 173 / median 186 / p90 199. At 19% a five-point win-rate change needs about
960 games a side (80% power, two-sided 5%), so a change to the race cannot be graded on
wins. Every game, lost or won, says how far our lane was from finishing and how
far the rival was. `tools/clock_margin.py` reads that from the boards the
control mod already exports.

## The reading

For each lane (`religious`, `diplomatic`, `science`), at the run's last board:

| field | Religious | Diplomatic | Science |
|---|---|---|---|
| shortfall | `majors_short` (ourselves included) and `cities_short`: cities that must turn for our religion to hold a strict majority in each | `points_short` of 20 | `launches_short` of four, and `distance_short` of the Exoplanet Expedition once it flies |
| `progress` (0..1) | majors whole, plus each holdout's share of its majority | points / 20 | (launches + flight share) / 5 |
| `turns_short` | cities short at our pace over the last 20 turns | points short at that pace | flight left at its speed, or launches left at our launch pace plus a flight at base speed |

`rival_clock` is the fewest turns any rival still needed by the lanes a board
can time (its expedition, its `culture_turns_to_victory`); 0 and the winner
when a rival won. `margin = rival_clock.turns - turns_short` (positive: we were
ahead) and each clock's `finish_turn`. A win by any victory is progress 1 and
0 turns short. `turns_short` is None when we made no progress in the window:
there is no honest estimate, and `progress` still grades the game.

Where it lives:

- every new `summary.json` carries `clock_margin` (all three lanes;
  `civ6_ladder.SUMMARY_MARKS`), so it reaches the `ledger` branch with the run;
- `python3 tools/live_ledger.py kpis` shows `lane%`, `short` and `margin` per
  game, and `screen <gene>` grades both arms on lane progress, margin and the
  rival's finish turn with Welch intervals;
- `python3 tools/clock_margin.py --root RUNS --since 2026-11-22 --difficulty DEITY`
  reads any runs directory (a pulled ledger included) and prints the batch.

A status report (`tools/live_status.py`) needs one call per game summary:
`clock_margin.lane_margin(summary)` returns the assigned lane's reading with the
rival's clock beside it, or None for a summary written before this reading
(`clock_margin.run_margin(run_dir)` reads that one from its event log), and
`clock_margin.batch_summary(readings)` gives medians, means and the losses within
10 and 20 turns.

## Deity, 2026-11-22 to 11-27 (244 live games with a lane)

| lane, outcome | games | progress (median) | turns short (median) | rival clock (median) |
|---|---:|---:|---:|---:|
| Religious, rival Science | 69 | 0.46 | 100 (28 not estimable) | 0 |
| Science, rival Science | 79 | 0.20 | 35 (44 not estimable) | 0 |
| Religious, won Religious | 19 | 1 | 0 | 30 |
| Science, won Diplomatic | 22 | 1 | 0 | 18 |
| Religious, eliminated | 20 | 0.25 | - | - |
| Science, rival Culture | 18 | 0.0 | - | 0 |

Of 167 losses, 7 were within 10 turns of our lane finishing and 13 within 20.

## Rival Science losses: the race (143 games with a recorded victory)

**The clock.** The winner landed its Satellite at a median t141, we first saw a
completed Spaceport of its at t142 (123 of 143), it launched the Exoplanet
Expedition at t171 and arrived at t186 (launch to arrival median 11 turns).
Nothing at a pad stops a launched expedition.

**The pack.** In 96 of the 143 at least one other rival had already launched
when the winner arrived; the next would have arrived a median 9 turns later. In
128 another rival had three or more launches. Holding back the eventual winner
alone buys at most the gap to the next racer.

**Pillaging the winner.** Successful Disrupt Rocketry on the winner before its
launch, against its Satellite-to-Expedition time and the win turn
(correlational: a long race also leaves more time to pillage):

| pillages | games | Satellite to Expedition (mean) | win turn (median) |
|---:|---:|---:|---:|
| 0 | 77 | 28.5 | 186 |
| 1 | 38 | 32.7 | 185.5 |
| 2 | 12 | 36.2 | 183.5 |
| 3+ | 16 | 37.8 | 190.5 |

Each pillage goes with three to four turns on that racer's launch and almost
nothing on when somebody wins: the next racer lands instead.

**Our Spies.** Spy capacity in 135 of 143 games (first slot median t110). 780
Disrupt Rocketry missions, 473 landed (61%); 374 against the winner, 208
landed; 98 games disrupted the winner, first a median 14 turns after its pad was
first seen. Of 7,562 Spy-turns after that sighting, 49% stood in the winner's
cities, 39% in other rivals' and 13% at home. The host accepts the orders: in
30 Tournament 8 logs `civ6_events.py failures` lists no refused Spy operation
(8 of 105 Spy builds `producing=nothing`, 32 `SPY_TRAVEL_NEW_CITY` refusals),
and the World Congress's Espionage Pact passed its option A, Disrupt Rocketry
two levels stronger, every time it named the operation (18; never the ban).

**Empty slots on the live lane.** Spies on the map against capacity, first
board of the turn, in games holding a slot:

| turn | Religious seats: capacity / on map / training / empty | Science seats |
|---:|---|---|
| 130 | 1.53 / 0.88 / 0.29 / 0.37 (49 of 102 have a slot) | 2.55 / 1.44 / 0.32 / 0.79 |
| 150 | 1.85 / 1.05 / 0.32 / 0.48 (66 of 95) | 2.92 / 1.50 / 0.34 / 1.08 |
| 170 | 2.15 / 1.19 / 0.29 / 0.66 (62 of 78) | 3.61 / 1.87 / 0.39 / 1.35 |

The Religion seat, the only lane Tournament 8 plays, holds about one Spy
through the race, and 29 of 95 held no slot at all at t150 (getting the slot
earlier is the spy-civics work of #5932, not this change). `denial-spies-fill-every-slot-2` is forced on the live seat
and reads nothing on a Religion or Science aim; version three, built and
fires-proven in #5547, is the one that reaches them and was never armed.

**War with the winner.** 39 of 143 were at war with it at some point, 19 in its
last 30 turns.

## The Religious lane's last 30 turns (68 losses to rival Science)

65 had founded. At the end 4 were one major short, 9 two, 13 three and 42 four
or more. Over the last 30 turns 31 of the 65 gained no major and 9 lost ground;
cities short fell by a median 1 against a median 30 spreads, 20 Missionaries
and Apostles on the map, 200 Faith a turn and 651 banked. The lane was not
waiting on Faith or on votes (Diplomatic Victory is off); the spreads did not
hold. Charged religious units spent a charge on 4.5% of their turns in these
losses and 4.4% in the 19 Religious wins, so walking is the lane's normal pace,
not what separates a win from a loss. From two majors short a win took a median
12 turns (1 to 33, 19 wins).

So a ten-turn delay of the rival converts few of these: by our own pace 2 of
the 68 were within 10 turns and 7 within 20.

## The change: `denial-spies-fill-every-slot-3` armed

Added to `deploy/live-force-on.txt`. It trains a Spy for every free slot from
the first slot, side by side, on every aim, keeping a Religion seat's religious
units and a Science seat's launch at the head of their queues. More Spies at
once is the lever the pack leaves: one Spy can slow one racer.
`the_live_list_fills_a_religion_seats_spy_slots`
(`src/ai/advanced/denial_spies_fill_every_slot/tests.rs`) seats the versioned
list on a Religion seat and fails without the tag.

**Expected movement.** About one more Spy on the map from t130 on a Religion
seat; more pads pillaged across the pack; the rival's finish turn +1 to +3
turns; the Religious lane's margin up by the same; Deity win rate +0 to +2
points, which no affordable sample shows on wins. The instrument is the rival's
finish turn (sd 11 turns over 148 games): +3 turns needs about 215 games an arm
at 80% power, +5 about 77. Spy count on the map at t150 and t170 should move in
the first 20 games; if it does not, the claim is not reaching the live boards.

**How the live seat is armed.** The supervisor plays the first of
`CIVVIS_WITH_FILE`, `~/.civvis-live-force-on` (when non-empty) and this file.
The live Deity seat does not play this file: its recorded `--with` list holds
622 tags, 281 of this file's 283 and some 340 more, and gains genes the day they
merge (`seen-pad-starts-the-denial` from 11-26 15:38Z,
`religion-lane-outlasts-the-denial` from 11-27 03:18Z). Until whoever keeps
that list adds `denial-spies-fill-every-slot-3`, this arm reaches seats that
track the repository list only. A game is armed when its summary's
`genome_treatments.forced` names the tag.
