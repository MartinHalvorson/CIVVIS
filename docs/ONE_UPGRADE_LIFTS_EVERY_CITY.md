# One upgrade lifts every city

**The question.** In Civilization VI the first unit into a new tier lifts the
defence or the ranged strike of every city at once. Live, the seat leaves
that upgrade waiting for a war. Does taking it ahead of the treasury floor
(`one-upgrade-lifts-every-city`, opt-in) fire in the simulator, and what does
it do to cities and units lost?

**The answer.** _(filled in from the rounds below)_

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
shard's worlds), as #5247 did.

_(rounds, verdict and what is not verified follow)_
