# One upgrade lifts every city

**The question.** In Civilization VI the first unit into a new tier lifts the
defence or the ranged strike of every city at once. Live, the seat leaves
that upgrade waiting for a war. Does taking it ahead of the treasury floor
(`one-upgrade-lifts-every-city`, opt-in) fire in the simulator, and what does
it do to cities and units lost?

**The answer.** It fires and it closes the gap it was written for. Over two
rounds (32 then 48 fresh pairs a recipe, 320 games) the seat's first
tier-lifting upgrade moves from about turn 70-85 to about turn 22, and the
share of turns with a city value a tier short falls from 36-38% to 20-25%
for Rome and from 22% to 16% for Gran Colombia. The outcome is smaller
than the mechanism. Rome lost about 2.5 fewer units a world in both rounds
(1.7 and 1.2 standard errors) and half a city fewer in the second round
only. Gran Colombia shows no effect that repeats: its first round read 4.8
more units lost and its second 2.4 fewer. The gene stays opt-in and is not
armed; it is a candidate for a seat that is defending (the Rome lane), not
for one that attacks.

## The rule

A city's ranged strike is the best ranged strength its owner has ever owned,
and its defence is built from the strongest unit it has ever owned less 10.
The simulator models this in `src/game/city.rs` (`city_ranged_strength`,
`city_strength`) through the player counters `strongest_ranged_built` and
`strongest_unit_built`, which `src/game/units.rs` raises on a spawn and
`src/game/unit_upgrades.rs` on an upgrade. So the first unit into a tier
lifts every city; the second adds nothing to them.

The gene (`src/ai/advanced/one_upgrade_lifts_every_city.rs`), immediately
before the ordinary pass in `upgrade_units_preserving_air_wing`
(`src/ai/advanced/air_resource_allocation.rs`), never for the barbarians or
a seat without a city:

1. **Basis.** Best melee = the larger of `strongest_unit_built` and the
   strongest military unit owned now. Best ranged = the larger of
   `strongest_ranged_built`, the best ranged unit owned now and, where an own
   city carries an observed strike, that strike less the cards'
   `city_ranged`. The maximum guards a resumed live mirror whose counters
   start empty.
2. **Lift.** For every Gold upgrade the board offers
   (`unit_gold_upgrade_offer`) that keeps the air wing's material
   (`air_resource_upgrade_preserves_wing`): `max(0, to.strength - best
   melee) + max(0, to.ranged_strength - best ranged)`. It qualifies when
   either term is at least `CITY_LIFT_MIN` (5).
3. **Floor.** The treasury after the bill stays at or above 0, and the
   seat's `gold_per_turn` is not negative. Nothing else: the pass runs ahead
   of the 120-Gold peacetime floor and the upkeep reserve by design.
4. **Order.** Highest lift per Gold first (ties: cheaper, then lower unit
   id), through `Action::UpgradeUnit`; the basis is read again and the pass
   repeats until nothing qualifies.
5. **Journal.** `One upgrade lifts every city` (Military, Decision), naming
   the Gold, the units and the defence and strike lift.

Off, the pass returns before it reads anything. Unit tests in
`src/ai/advanced/one_upgrade_lifts_every_city/tests.rs`: the first
Crossbowman is taken and the second is not, the pass goes ahead of the
ordinary floor where the ordinary pass does not, the treasury may reach 0
and no lower, a negative (or unknown) income takes nothing, off takes
nothing, and a tier already owned (now, in the counters only, or as an
observed city strike) lifts nothing.

## Live evidence (115 games, 2026-11-12/13)

From the operator's brief, not re-derived here:

- City strike equals the best ranged strength ever owned in 93% of 12,374
  state rows, and upgrades count.
- From turn 30, the best melee strength the seat ever owned is 10 or more
  below what it has unlocked with the material in stock on 38.6% of game
  turns (stretches of 21 turns, median); best ranged is 10 or more short on
  19.5% of turns.
- When a war is declared on us, defence is 10 or more short in 42% of those
  wars and strike in 25%.
- Cause: the ordinary pass (`BasicAi::modernize_army_with_filter`) ranks by
  strength gain per Gold and never spends below 120 Gold at peace (30 at
  war), raised further by `upgrade_treasury_floor`; the median live treasury
  is 79 / 111 / 144 Gold at turns 50 / 75 / 100.

## Method

**Recipes.** The two gap-faithful recipes of
`docs/COMBAT_GENES_OUTCLASSED.md`, with `--rival-host-tactics` and no
`--rival-rung`:

- Gran Colombia domination: `--players 4 --width 60 --height 38
  --city-states 6 --map pangaea --difficulty deity --rivals
  firaxis-faithful --handicap rivals --rival-chairs 3 --rival-pace
  --target-mix domination --rival-host-tactics`, Online speed, to the end of
  the game.
- Rome / Score: `--players 6 --width 74 --height 46 --city-states 9 --map
  continents --difficulty deity --rivals firaxis-faithful --handicap rivals
  --rival-chairs 5 --rival-pace --rival-war-pace 100 --target-mix score
  --seat-civ Rome --turns 170 --rival-host-tactics`.

**Genome.** The full genome the live seat plays: line 1 of live run
`civvis-20261113T112747Z`'s `why.log` (a Rome Score run, 709 tags), not the
force-on list read alone (which would switch off the ledger defaults it does
not repeat). The gene is not in it, so the control plays it off; the gene
arm is the same genome with the gene on.

**Pairs.** Each seed is played twice in one process, control and gene on,
with the same game index, so a pair differs in the gene alone. Δ is **gene
on minus control** per world, mean ± standard error over the pairs; a `*`
marks |Δ| of 1.5 standard errors or more. `diverged` counts the pairs whose
two games differ at all (the measured seat's row without wall time and
genome, plus the length of the game log). Rounds are reported on their own
and never pooled.

**What a world records.** The measured seat's gene-screen row (units lost
and killed from `Game::combat_ledger`, cities lost and taken, score); the
turn of every upgrade the new pass took (its journal lines, read from the
seat's `--journal` file); and an observer's look at the start of every turn:
the seat's upgrades (a unit that changed kind), the ones that lifted its
best melee or ranged strength ever owned by 5 or more (`tier-lift
upgrades`, whichever pass took them), the turns from turn 30 on which that
best sat 10 or more below the strongest land unit it had unlocked with the
material in stock (the live gap, same definition as the live census), and
the kills no unit made (`kills` less every `kill_with:*` counter: the city
and district strikes).

**Driver.** `src/bin/gene_screen/one_upgrade_lifts_every_city_tests.rs`, an
ignored `gene_screen` test, run on GitHub's runners through a temporary
workflow on PR #5251 (one build, four shards a recipe, one report over every
shard's worlds), as #5247 did. The workflow and the genome copy were removed
before the PR was made ready; to rerun, restore both from commit e82cea8be3.

## Rounds

Round 1 is 32 pairs a recipe (seeds 64760000.. Gran Colombia, 64770000..
Rome, run 43420492669); round 2 is 48 fresh pairs a recipe (64780000..,
64790000.., run 43423709541). Every figure is gene on minus control, a
world.

**The pass fires.** Its first upgrade comes at turn 21-23 (median) in all
four cells; the control's first tier-lifting upgrade comes at turn 67-85.
Round 2 counts the upgrades the board carried out (round 1 counted journal
lines, which include plans the turn did not keep, so its count is not
comparable):

| round 2 | worlds the pass acted in | upgrades a world | Gold a world |
|---|---|---|---|
| Gran Colombia | 41 of 48 | 1.60 ± 0.17 | 113 ± 16 |
| Rome | 45 of 48 | 2.75 ± 0.24 | 244 ± 29 |

Two upgrades carry most of it: Slinger to Archer (30 Gold, strike +10; 69
of the 207 carried out in round 2) and Archer to Crossbowman (about 100-125
Gold, strike +15; 43). The rest are single melee tiers (Warrior to Legion,
Legion to Man-at-Arms, Spearman to Pikeman) and, for Gran Colombia, Galley
to Caravel (9, 160 Gold; see "Ships count").

**The gap closes.** Share of the seat's turns from turn 30 with a city
value 10 or more below what it had unlocked (`*` = 1.5 standard errors):

| | round | defence short | strike short | either short |
|---|---|---|---|---|
| Rome | 1 | 0.183 to 0.147 (-0.036 ± 0.027) | 0.209 to 0.068 (-0.141 ± 0.037*) | 0.352 to 0.202 (-0.150 ± 0.042*) |
| Rome | 2 | 0.183 to 0.126 (-0.058 ± 0.021*) | 0.244 to 0.141 (-0.103 ± 0.030*) | 0.373 to 0.244 (-0.129 ± 0.030*) |
| Gran Colombia | 1 | 0.112 to 0.124 (+0.012 ± 0.024) | 0.116 to 0.044 (-0.072 ± 0.027*) | 0.226 to 0.168 (-0.058 ± 0.038*) |
| Gran Colombia | 2 | 0.120 to 0.103 (-0.017 ± 0.017) | 0.120 to 0.056 (-0.064 ± 0.024*) | 0.217 to 0.156 (-0.061 ± 0.028*) |

The strike half repeats in all four cells. The defence half repeats for
Rome only.

**The outcome.**

| | round | units lost | cities lost | units killed | score |
|---|---|---|---|---|---|
| Rome | 1 | -2.56 ± 1.53* | 0.00 ± 0.38 | +0.78 ± 2.61 | -10.4 ± 13.8 |
| Rome | 2 | -2.44 ± 2.06 | -0.54 ± 0.30* | -0.17 ± 1.98 | +20.6 ± 13.8 |
| Gran Colombia | 1 | +4.84 ± 2.83* | +0.78 ± 0.60 | +0.16 ± 2.40 | +0.8 ± 17.4 |
| Gran Colombia | 2 | -2.38 ± 2.10 | +0.15 ± 0.27 | -3.56 ± 1.97* | -10.0 ± 9.6 |

City-strike kills did not rise in any cell (+0.44, -0.48, +0.50, -0.88, all
inside one and a half standard errors): the stronger strike is not turning
into more kills by the city. No seat won in any arm.

## Verdict

- **Mechanism: confirmed.** The pass fires in 85-100% of worlds, takes the
  first tier-lifting upgrade 45-60 turns sooner and cuts the tier-short
  share of turns by a third to a half. Both rounds, both recipes.
- **Rome (defending seat): a lean, not a result.** Units lost fell by about
  2.5 a world in both rounds, with the same sign and size, but neither
  round alone clears two standard errors and cities lost moved in round 2
  only. That is the same size as the largest effects any unit-handling gene
  showed in `docs/COMBAT_GENES_OUTCLASSED.md`, at a cost of about 240 Gold a
  game.
- **Gran Colombia (attacking seat): no effect.** The round 1 flag (more
  units lost) reversed in round 2. Round 2 adds a flag of its own, 3.6 fewer
  kills and 2.0 fewer upgrades seen in total, which would fit the pass
  spending Gold the ordinary pass would have put into the army later; it is
  one round and is not confirmed.
- **Recommendation.** Opt-in, unarmed. Worth a live trial on the Rome lane,
  where the live gap (38.6% of turns) is as large as the sim control's
  (36-38%); not on a domination seat.

## Ships count

The simulator's `strongest_unit_built` counter takes any military unit, so
the pass bought Galley to Caravel 9 times for Gran Colombia as a defence
lift. The live logs agree with the simulator (117 games, the first turn a
ship's strength passes the best land unit ever owned by 5 or more, every
city present on both sides of that turn): defence rose in 245 of 254 city
rows over 46 games, by the ship's margin over the land best (5 to 30), and
the ranged strike rose in 88 of 131 city rows over 14 games (by 10, 15 or
30). So a ship upgrade is a real city lift and the pass is right to take it.

## What is not verified

- **No local build.** A live game shared the host, so nothing was compiled
  locally and the single-gene fires probe was not run
  (`tools/gene_fire_waivers.json`). CI built and ran the unit tests and the
  driver.
- **Live.** The gene has not played a real game. A live seat's counters
  start empty on a resumed mirror, which the basis guards by reading the
  observed city strike and the units owned; that guard is unit-tested, not
  seen live.
- **Kills by cities.** The stronger strike produced no more city-strike
  kills in the simulator. Live, our strikes on majors average 14-19 damage
  for 239 kills in 115 games, so the live pay-off may differ in either
  direction.
- **Power.** 32 and 48 pairs a recipe. Under about 3 units or 0.5 cities a
  world is not resolved in either round.
