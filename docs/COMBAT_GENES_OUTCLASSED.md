# Combat genes when the seat is outclassed

Until 2026-11-13 every combat gene was priced in simulated worlds where our
seat was not outclassed: the lane recipe's rivals played at a lower rung, our
seat traded two to five kills a loss, and the rival majors barely struck it
on the Rome recipe (`docs/EDGE_LEDGER.md`). Live, 69-71% of the rival majors'
blows on our units come from units 10 or more Combat above ours, a blow there
kills 0.6 of the time, and our seat trades 1.7-1.8 kills a loss. So an armed
gene could cost units or cities exactly where the live seat fights, and no
earlier screen could have seen it.

**The question.** Which of the ALREADY ARMED combat genes cost our seat units
or cities in a world where it is outclassed the way the live seat is?

**The answer.** None that this harness can see. Fourteen armed tactical
genes were each turned off against the genome the live seat plays, on the
gap-faithful Gran Colombia recipe (24 pairs), and every flag was played
again on fresh seeds (32 pairs on each recipe, then 48 for the three that
kept pointing one way). No gene's removal cut units or cities lost by 1.5
standard errors in two independent rounds, and none was confirmed to save
them on Gran Colombia either. `capital-defense-holds` leans toward costing Gran
Colombia's seat about two units a world (three rounds, one at 1.5 standard
errors) and `safest-stand` toward saving Rome's about two to three (two
rounds); both are within what 48 pairs cannot separate from zero. The
largest effect found is not a gene: simulating the force-on list alone, as
the earlier drivers did, leaves 101 ledger defaults the live seat plays
switched off, and that moves units and cities lost more than any gene here,
in opposite directions on the two recipes.

## How it was measured

**Recipes.** The gap-faithful settings of `docs/EDGE_LEDGER.md`, with
`--rival-host-tactics` (`docs/HOST_ENEMY_MODEL.md`) and no `--rival-rung`:

- Gran Colombia domination: `--players 4 --width 60 --height 38
  --city-states 6 --map pangaea --difficulty deity --rivals firaxis-faithful
  --handicap rivals --rival-chairs 3 --rival-pace --target-mix domination
  --rival-host-tactics`, Online speed, to the end of the game.
- Rome / Score: `--players 6 --width 74 --height 46 --city-states 9 --map
  continents --difficulty deity --rivals firaxis-faithful --handicap rivals
  --rival-chairs 5 --rival-pace --rival-war-pace 100 --target-mix score
  --seat-civ Rome --turns 170 --rival-host-tactics`.

**Genome: the one the live seat plays, not the force-on list read alone.**
The live seat plays the deployment genome (the gene ledger's defaults) with
the armed set (`~/.civvis-live-force-on`) on top; a live run's `why.log`
header lists the result. `--live-genome ~/.civvis-live-force-on` does not
play that: it turns every gene on iff the list names it, so every
ledger-default gene the list does not repeat is OFF. Against the header of
live run `civvis-20261113T090930Z` (708 tags) the list alone (571 tags)
leaves out 137 tags the live seat plays, 101 of them ledger defaults this
build carries: `objective-board`, `unit-objective-memory`, `doomed-blow-veto`,
`hostile-memory-3`, `strike-opening`, `war-economy`, `bounded-recovery`,
`reinforce-before-stall`, `threatened-city-reserve`, `early-archers` among
them. The edge ledger (#5232) and the declaration gate (#5236) played the
list alone. Here the control plays that run's header (every armed tag is in
it; the 35 host-only tags it names are not genes of this build), and the
list read alone plays as one more arm, `alternative-genome`, so the
difference is measured rather than assumed. Without the objective board,
for one, `capital-defense-holds` cannot act at all: its only reader is the
board's row assessment.

**Leave one out.** Each gene arm is the control with ONE armed gene turned
off. It plays the control's seeds and game indices, so the rival chairs sit
where they sat, and the two games of a pair differ in that gene alone. Every
arm of a seed is played in the same process on the same build, so no pair
crosses a shard, a build or a round. Δ is **gene OFF minus control**, per
world, mean ± standard error over the pairs: **a negative Δ on units or
cities lost means the gene costs them** (taking it out saved them); a
positive one means the gene saves them. `diverged` counts the pairs whose
two games differ at all (the measured seat's whole row, wall time and genome
left out, plus the length of the action log): a gene whose pairs never
diverge never fired, and is inert here, not neutral. Round 0 (the force-on
list alone, 32 seeds, seeds 64700000-31) played its control twice: 0 of 32
pairs diverged, so a diverged pair is the gene, not the harness.

**Power.** For a gene that fires in every pair, units lost per world has a
paired standard deviation of about 10 to 18 units, so 24 pairs read units
lost to about ±2-3.5 (one standard error) and 32 pairs to about ±2-3. A
gene has to move a world's losses by about 5 units to show at 1.5 standard
errors in one round. Anything under that is noise at this size, and the
operator's rule (a small run is not grounds to cut a gene) is why every
screen flag gets a second round on fresh seeds before it is called.

**Driver.** `src/bin/gene_screen/combat_genes_outclassed_tests.rs`, an
ignored `gene_screen` test (see "Reproduce"). The rounds below ran on
GitHub's runners through a temporary workflow on PR #5247: one build, the
seeds sharded over four to six runners (every arm of a seed in one shard),
then the driver's report over every shard's worlds. The report blocks below
are as printed, except that rounds 0-3 headed them "the armed set" where
the final driver says "the control genome" and "the control". Each world's
row is kept in that run's artifacts (`worlds-*`, 14 days).

## Where our units die (the control)

Round 1's control: the live genome on 24 Gran Colombia worlds, as the driver
printed it:

```text
== gran-colombia: control (the control genome), 24 worlds, seeds 64710000..64710023
   per world: units lost 30.46±2.95 (majors 24.38±2.71, city-states 2.38±0.83, barbarians 3.62±0.52), killed 44.12±4.23, civilians lost 3.25±0.48
   per world: cities lost 3.46±0.54, cities taken 0.04±0.04, score 246.46±24.12, score share 0.07±0.01, wins 0.00±0.00, alive 0.79±0.08, last turn 141.46±2.45
   the gap: major_blows_per_game 26.792, major_share_ge10 0.642, major_death_per_blow_ge10 0.535, strength_gap_100 14.687, kill_loss 1.449, cities_lost_per_game 3.458
   deaths by the step that last ordered the unit (1120 counted, 46.67 a world; within 2 = ended its last turn within two tiles of a visible hostile):
     step                    deaths   a world   share  within 2
     turn_start                 314     13.08   28.0%    100.0%
     siege_doctrine             136      5.67   12.1%     74.3%
     rotation                   131      5.46   11.7%     68.7%
     battle_planner             114      4.75   10.2%     86.0%
     healing                    114      4.75   10.2%     81.6%
     coordinated                 59      2.46    5.3%     33.9%
     retreat                     50      2.08    4.5%     78.0%
     positions                   40      1.67    3.6%     60.0%
     escort                      38      1.58    3.4%     76.3%
     explorer                    38      1.58    3.4%     55.3%
     barbarian_response          30      1.25    2.7%     40.0%
     stand_down                  16      0.67    1.4%     50.0%
     tactical_attack             14      0.58    1.2%     92.9%
     unordered                    7      0.29    0.6%     42.9%
     peacetime                    4      0.17    0.4%     50.0%
     staging                      4      0.17    0.4%      0.0%
     reinforcement                3      0.12    0.3%     33.3%
     tactical                     3      0.12    0.3%     33.3%
     camp                         1      0.04    0.1%      0.0%
     civilian_capture             1      0.04    0.1%    100.0%
     doctrine_action              1      0.04    0.1%      0.0%
     guns_out_of_reach            1      0.04    0.1%    100.0%
     pillage                      1      0.04    0.1%      0.0%
   units gone by distance from our nearest City Center at the start of the turn they went (822 gone; the ledger's units lost 731):
     centre 109 (13%), 1-2 450 (55%), 3-5 177 (22%), 6-9 57 (7%), 10+ 28 (3%), no city 1 (0%)
   units gone by kind: archer 133, pikeman 111, warrior 74, heavy_chariot 69, scout 63, crossbowman 60, horseman 34, spearman 34, trebuchet 34, pike_and_shot 33, catapult 30, knight 29
```

Round 0 played the force-on list alone (the genome the earlier drivers
played) on 32 other seeds: 21.8 ± 2.7 units lost a world (15.4 to majors,
4.5 to barbarians), 2.3 ± 0.4 cities lost, kill:loss 1.98, 68% of the rival
majors' blows at +10 or more; its steps in the same order (`turn_start`
33%, `siege_doctrine` 16%, `rotation` 11%, `battle_planner` 9%, `healing`
8%), 87% of the units that went within five tiles of a City Center.

`turn_start` is the step the census credits for orders given before the
unit pass of a planning frame: the war finishing volley
(`ai/finishing.rs`, strikes that finish wounded enemies before ordinary
movement) and the command actions. Every one of those deaths was in
contact (100% within two tiles of a visible hostile), so these are units
that struck at the start of a frame and died then or on the enemy's turn.
It is the largest single bucket in every round: 28-33% of deaths.

Where: 90% of the units that went were within five tiles of our nearest
City Center at the start of the turn they went (live: three quarters of
the deaths in wars declared on us), and 13% stood on a City Center (units
lost with a city). Which: Archers, Pikemen, Warriors, Heavy Chariots and
Scouts, as live. The observer counts every military unit that went, corps
merges and disbands included, so it reads about 12% above the ledger's
units lost.

The gap reads close to live on blows (64% of the rival majors' blows at +10
or more, live 71%; a blow there kills 0.54 of the time, live 0.55) and
kill:loss (1.45, live 1.68), but the live genome loses **3.5 cities a world
here against 1.6 live**: the simulated seat is, if anything, more
outclassed in cities than the live one.

## The genes screened, and why

All fourteen are in the armed set and on in the control. Steps are the
order steps their code orders units through (`ai/advanced/last_orders.rs`);
shares are the round-1 control's.

| gene | ledger default | acts through | why it is here |
| --- | --- | --- | --- |
| `battle-planner-2` | off | `battle_planner`, `rotation`, `positions` (22% of deaths) | the plan that orders most engaged units |
| `safest-stand` | off | `rotation` (12%) | the heal rotation's fallback stand when no tile is safe |
| `shared-danger` | on | `rotation`, doomed-blow checks, siege staging | reads a hostile's blow as shared among our units: a lighter danger estimate exactly where we are outclassed |
| `ranged-hp-reserve` | on | `healing` (10%) | Archers die most; keeps 20 hp in hand on lethal tiles |
| `siege-train` | on | `siege_doctrine` (12%) | the step that last ordered the most deaths in the out-teched worlds of #5196 / #5211 |
| `siege-commitment` | off | the war's objective | keeps a siege on its city; the war loses units there |
| `siege-needs-a-breaker` | on | `siege_doctrine`, siege rows | when a siege waits for its breaker |
| `anvil` | on | the threatened city's formation | 90% of deaths are within five tiles of our cities |
| `camp-party` | off | peacetime army on barbarian camps | 3.6-4.5 units a world lost to barbarians |
| `ranged-finish-on-the-mean` | off | `turn_start` (28%), the finishing volley | the only armed gene on the step with the most deaths |
| `capital-defense-holds` | on | the board's Defend row | 3.5 cities lost a world |
| `defenders-hold-the-walls` | off | `rotation`, `battle_planner` near our cities | keeps defenders on their posts under danger |
| `threatened-city-buys-its-defender-2` | off | purchases | buys a defender of the city's era before it is hit |
| `recon-replacement` | off | `explorer` (3%) | Scouts are among the units that die most |

Considered and left out: `ranged-before-melee` and `generals-lead-the-army`
(live bridge only: their code is in `civvis_orders`, which a simulated seat
never runs); `war-reinforcement` (its `reinforcement` step last ordered 0 of
1,084 deaths in round 0 and 3 of 1,120 in round 1); `march-uses-its-moves`
(its `staging` step, 0.4-1.5% of deaths; replaced by
`ranged-finish-on-the-mean` once round 0 showed the finishing volley's
share); `opening-scout-comes-back`, `garrison-under-fire` and
`guns-stay-out-of-reach` (smaller steps; the list stops at fourteen).

## Screen (round 1): Gran Colombia, 24 pairs

Seeds 64710000-23, merge of PR head `0387ab7ad0` with main, 384 games. As
the driver printed it:

```text
== gran-colombia: one armed gene OFF minus the control, paired by seed (mean ± se a world; * = |d| >= 1.5 se on a loss column)
   gene off                             pairs diverged      units lost       to majors  lost <=5 tiles          killed       kill:loss     cities lost    cities taken           score
   alternative-genome                      24    24/24     -5.92±3.27*     -6.21±2.95*     -7.25±2.65*     -1.00±3.30      +0.31±0.19      -1.04±0.58*     -0.04±0.04    +14.96±18.31
   battle-planner-2                        24    24/24     +0.04±3.49      -0.50±3.58      -2.54±3.15      -7.46±3.76      -0.25±0.19      +0.12±0.63      -0.04±0.04    -28.79±22.65
   safest-stand                            24    22/24     -0.88±3.15      -0.83±3.13      -0.58±2.33      -1.25±1.94      +0.00±0.15      -0.96±0.54*     +0.00±0.06    +10.58±14.78
   shared-danger                           24    23/24     -4.21±2.88      -3.62±2.53      -3.75±2.88      -1.21±2.82      +0.19±0.15      -0.21±0.59      +0.04±0.04     -1.79±15.95
   ranged-hp-reserve                       24    15/24     -2.29±2.41      -1.75±2.46      -2.83±2.17      -2.21±1.45      +0.04±0.11      -0.21±0.55      +0.12±0.12     +5.46±13.10
   siege-train                             24    20/24     +1.00±2.39      +1.12±2.27      -0.04±2.25      +0.75±1.37      -0.02±0.10      +0.00±0.34      -0.04±0.04    -17.50±10.38
   siege-commitment                        24     5/24     +0.04±0.24      +0.38±0.40      +0.12±0.41      +0.21±0.40      +0.00±0.01      +0.21±0.18      +0.00±0.00      -8.33±5.93
   siege-needs-a-breaker                   24     9/24     +0.75±0.55      +1.33±0.63*     -0.29±0.56      +0.38±0.91      -0.02±0.03      +0.17±0.10*     +0.04±0.04      -4.25±2.85
   anvil                                   24    24/24     -4.21±3.22      -3.67±3.03      -2.75±2.98      -1.33±3.18      +0.18±0.23      -0.46±0.63      -0.04±0.04     +8.79±15.77
   camp-party                              24    24/24     -2.79±3.78      -2.17±3.61      -2.42±3.30      -3.12±3.26      +0.03±0.21      -0.54±0.64      +0.00±0.06    +14.21±16.92
   ranged-finish-on-the-mean               24    22/24     +3.92±2.08*     +3.71±2.07*     +3.38±1.90*     +0.58±2.23      -0.15±0.14      -0.12±0.44      +0.04±0.04     -1.33±10.80
   capital-defense-holds                   24    20/24     -1.54±1.79      -1.79±1.84      -0.62±2.02      +0.96±1.25      +0.11±0.09      -0.92±0.54*     +0.00±0.00    +13.21±11.62
   defenders-hold-the-walls                24    24/24     -0.88±2.98      -0.58±2.80      +0.38±2.63      -1.08±3.72      +0.01±0.17      -0.25±0.72      +0.00±0.06     +1.38±23.80
   threatened-city-buys-its-defender-2     24    13/24     -0.79±3.01      -0.96±2.93      -2.75±2.64      -3.75±1.89      -0.09±0.18      -0.04±0.43      -0.04±0.04     -7.50±14.66
   recon-replacement                       24    24/24     -2.96±2.96      -0.88±2.66      -0.54±2.69      -4.71±2.93      -0.02±0.16      -0.04±0.63      -0.04±0.04    +19.33±18.72
```

Where the deaths moved (the five largest changes, summed over the 24 pairs):

```text
== gran-colombia: deaths by last order step, gene OFF minus the control, summed over the pairs (largest five moves)
   alternative-genome                   turn_start +68, rotation -54, retreat -50, battle_planner -48, healing -39
   battle-planner-2                     rotation -131, battle_planner -114, healing +108, positions -40, siege_doctrine +30
   safest-stand                         rotation -32, healing +25, siege_doctrine -16, barbarian_response -12, battle_planner -11
   shared-danger                        turn_start -123, rotation -38, battle_planner -31, escort -13, healing +10
   ranged-hp-reserve                    turn_start -37, rotation -21, healing -17, barbarian_response -15, battle_planner -14
   siege-train                          siege_doctrine -26, turn_start -18, rotation +14, coordinated +11, battle_planner +8
   siege-commitment                     turn_start +24, coordinated +7, retreat +5, battle_planner +4, healing -3
   siege-needs-a-breaker                coordinated +12, healing -10, turn_start -8, battle_planner +7, positions -7
   anvil                                siege_doctrine -80, turn_start -31, coordinated +19, battle_planner -18, escort -17
   camp-party                           battle_planner -46, siege_doctrine -33, coordinated +19, turn_start -17, barbarian_response -13
   ranged-finish-on-the-mean            turn_start +58, coordinated +27, retreat +25, healing +18, siege_doctrine +13
   capital-defense-holds                turn_start +93, rotation -33, healing +20, siege_doctrine -19, staging +11
   defenders-hold-the-walls             healing -46, rotation +34, siege_doctrine -30, positions +28, turn_start -23
   threatened-city-buys-its-defender-2  turn_start +55, battle_planner -22, barbarian_response -11, healing -11, siege_doctrine -11
   recon-replacement                    siege_doctrine -41, coordinated +31, battle_planner -26, retreat +23, explorer -17
```

`lost <=5 tiles` is the observer's count of units that went within five
tiles of our nearest City Center (corps merges and disbands included), not
the ledger's.

**Flagged for confirmation** (|Δ| of about 1.5 standard errors or more on
units lost or cities lost), five genes and the genome contrast:

- `ranged-finish-on-the-mean`: off, +3.92 ± 2.08 units lost (+3.71 ± 2.07 to
  majors): the gene SAVES units. With it off, 58 more deaths were last
  ordered by the finishing volley.
- `siege-needs-a-breaker`: off, +1.33 ± 0.63 units lost to majors and +0.17 ±
  0.10 cities lost: the gene saves both, in the 9 of 24 pairs where it fired.
- `safest-stand`: off, -0.96 ± 0.54 cities lost: the gene may COST cities
  (units lost -0.88 ± 3.15, noise).
- `capital-defense-holds`: off, -0.92 ± 0.54 cities lost: the gene may cost
  cities (units -1.54 ± 1.79, noise).
- `shared-danger`: off, -4.21 ± 2.88 units lost (1.46 standard errors): the
  gene may cost units; with it off, 123 fewer deaths were last ordered by
  the finishing volley.
- `alternative-genome` (the force-on list alone: roughly the live genome
  without the 101 ledger defaults the list does not repeat): -5.92 ± 3.27
  units lost and -1.04 ± 0.58 cities lost against the live genome.

Everything else is inside 1.5 standard errors on both loss columns. The
largest of those, `anvil` (-4.21 ± 3.22 units) and `recon-replacement`
(-2.96 ± 2.96), are noise at this size; `battle-planner-2` reads zero on
units but -7.46 ± 3.76 on units killed (2.0 standard errors) and -28.8 ±
22.7 on score, so taking it out costs kills, not losses.

## Confirmation (round 2): Gran Colombia, 32 fresh pairs

Seeds 64720000-31, the same build for both recipes (PR head `dd02a9b0b9`
merged with main), 224 games. The control (live genome):

```text
== gran-colombia: control (the control genome), 32 worlds, seeds 64720000..64720031
   per world: units lost 28.53±3.74 (majors 23.78±3.79, city-states 0.88±0.30, barbarians 3.44±0.41), killed 39.72±3.17, civilians lost 3.59±0.45
   per world: cities lost 2.91±0.43, cities taken 0.06±0.04, score 251.34±16.97, score share 0.08±0.01, wins 0.00±0.00, alive 0.94±0.04, last turn 134.50±2.22
   the gap: major_blows_per_game 32.469, major_share_ge10 0.706, major_death_per_blow_ge10 0.522, strength_gap_100 16.991, kill_loss 1.392, cities_lost_per_game 2.906
```

```text
== gran-colombia: one armed gene OFF minus the control, paired by seed (mean ± se a world; * = |d| >= 1.5 se on a loss column)
   gene off                             pairs diverged      units lost       to majors  lost <=5 tiles          killed       kill:loss     cities lost    cities taken           score
   alternative-genome                      32    32/32     -6.03±3.54*     -6.22±3.55*     -5.56±3.42*     -4.56±3.66      +0.17±0.18      -0.53±0.35*     +0.00±0.06    -10.41±12.14
   safest-stand                            32    29/32     -3.75±2.87      -3.84±2.72      -2.22±2.62      +1.62±1.94      +0.28±0.19      -0.50±0.35      +0.00±0.08    +17.28±10.19
   shared-danger                           32    30/32     -1.72±2.20      -1.41±2.19      -1.16±2.08      -0.56±2.94      +0.07±0.11      +0.00±0.22      +0.00±0.08     +3.94±11.18
   siege-needs-a-breaker                   32    14/32     -0.38±0.83      -0.41±0.92      -0.16±0.82      -0.19±1.03      +0.01±0.06      -0.28±0.17*     -0.06±0.04      +9.28±4.68
   ranged-finish-on-the-mean               32    27/32     +0.41±2.12      +1.31±2.07      +1.22±2.10      +2.00±1.83      +0.05±0.11      -0.22±0.29      -0.06±0.04      +5.62±7.30
   capital-defense-holds                   32    22/32     -2.22±1.28*     -2.25±1.30*     -2.00±1.12*     -1.06±1.33      +0.08±0.07      +0.25±0.29      -0.06±0.04      -8.50±7.97
```

```text
== gran-colombia: deaths by last order step, gene OFF minus the control, summed over the pairs (largest five moves)
   alternative-genome                   retreat -82, healing -76, turn_start -69, battle_planner -42, siege_doctrine +34
   safest-stand                         battle_planner -38, siege_doctrine -25, rotation -19, turn_start -15, coordinated +14
   shared-danger                        turn_start -44, healing -35, coordinated +28, battle_planner -24, siege_doctrine -18
   siege-needs-a-breaker                positions +16, battle_planner -14, stand_down -10, unordered +8, explorer -7
   ranged-finish-on-the-mean            rotation +21, coordinated +20, retreat +15, battle_planner -10, siege_doctrine -10
   capital-defense-holds                turn_start -96, battle_planner -32, siege_doctrine -25, healing -24, coordinated -19
```

Its control's steps (32.5% `turn_start`, 12.5% `siege_doctrine`, 12.1%
`healing`, 9.1% `battle_planner`, 8.9% `rotation`) and places (88% within
five tiles of a City Center) repeat round 1's.

**Against round 1, on fresh seeds:**

- `ranged-finish-on-the-mean`: +0.41 ± 2.12 units lost (round 1 +3.92 ±
  2.08). Not repeated.
- `siege-needs-a-breaker`: cities lost -0.28 ± 0.17, the opposite sign to
  round 1's +0.17 ± 0.10; units -0.38 ± 0.83. Not repeated: noise in both
  rounds, on the 9-14 pairs where it fired at all.
- `safest-stand`: cities lost -0.50 ± 0.35 (round 1 -0.96 ± 0.54), units
  -3.75 ± 2.87 (round 1 -0.88 ± 3.15). The same direction twice, under 1.5
  standard errors this time.
- `capital-defense-holds`: units lost -2.22 ± 1.28 (1.7 standard errors;
  round 1 -1.54 ± 1.79), cities lost +0.25 ± 0.29 (round 1 -0.92 ± 0.54).
  The cities flag did not repeat; the units read points the same way in
  both rounds.
- `shared-danger`: -1.72 ± 2.20 units (round 1 -4.21 ± 2.88). Same
  direction, well inside noise.
- `alternative-genome`: -6.03 ± 3.54 units and -0.53 ± 0.35 cities (round 1
  -5.92 ± 3.27 and -1.04 ± 0.58). Repeated: on this recipe the force-on list
  read alone loses about six units a world fewer than the genome the live
  seat plays.

## Rome / Score (round 3): 32 pairs

Seeds 64730000-31, the same run as round 2, 224 games. Every Rome control
game ended on a rival's Science victory (31) or Culture victory (1) at about
turn 136, before the 170-turn limit. The control:

```text
== rome: control (the control genome), 32 worlds, seeds 64730000..64730031
   per world: units lost 12.66±2.37 (majors 9.59±2.42, city-states 0.16±0.08, barbarians 2.88±0.31), killed 33.69±2.46, civilians lost 2.00±0.29
   per world: cities lost 1.47±0.31, cities taken 0.00±0.00, score 356.66±19.26, score share 0.07±0.00, wins 0.00±0.00, alive 0.97±0.03, last turn 136.28±1.54
   the gap: major_blows_per_game 7.125, major_share_ge10 0.627, major_death_per_blow_ge10 0.671, strength_gap_100 13.941, kill_loss 2.662, cities_lost_per_game 1.469
   deaths by the step that last ordered the unit (693 counted, 21.66 a world; within 2 = ended its last turn within two tiles of a visible hostile):
     step                    deaths   a world   share  within 2
     turn_start                 232      7.25   33.5%    100.0%
     battle_planner              78      2.44   11.3%     83.3%
     rotation                    72      2.25   10.4%     50.0%
     healing                     71      2.22   10.2%     74.6%
     siege_doctrine              51      1.59    7.4%     49.0%
     coordinated                 43      1.34    6.2%     16.3%
     explorer                    29      0.91    4.2%     44.8%
     barbarian_response          23      0.72    3.3%     52.2%
     positions                   23      0.72    3.3%     52.2%
     escort                      18      0.56    2.6%     55.6%
     retreat                     16      0.50    2.3%     81.2%
     unordered                   13      0.41    1.9%     30.8%
     stand_down                   9      0.28    1.3%     44.4%
     tactical_attack              5      0.16    0.7%     60.0%
     peacetime                    3      0.09    0.4%     66.7%
     condemn                      2      0.06    0.3%     50.0%
     tactical                     2      0.06    0.3%    100.0%
     air                          1      0.03    0.1%      0.0%
     civilian_capture             1      0.03    0.1%    100.0%
     staging                      1      0.03    0.1%      0.0%
   units gone by distance from our nearest City Center at the start of the turn they went (501 gone; the ledger's units lost 405):
     centre 56 (11%), 1-2 256 (51%), 3-5 115 (23%), 6-9 41 (8%), 10+ 32 (6%), no city 1 (0%)
   units gone by kind: archer 92, crossbowman 54, scout 46, pike_and_shot 42, warrior 36, heavy_chariot 35, horseman 33, man_at_arms 18, cuirassier 16, galley 16, knight 16, field_cannon 15
```

```text
== rome: one armed gene OFF minus the control, paired by seed (mean ± se a world; * = |d| >= 1.5 se on a loss column)
   gene off                             pairs diverged      units lost       to majors  lost <=5 tiles          killed       kill:loss     cities lost    cities taken           score
   alternative-genome                      32    32/32     +5.41±2.29*     +3.06±2.08      +7.16±2.29*     -1.94±2.86      -0.90±0.36      +1.22±0.34*     +0.00±0.00    -57.28±15.98
   safest-stand                            32    22/32     +2.03±1.44      +1.91±1.41      +2.50±1.00*     -2.28±1.78      -0.52±0.27      +0.28±0.24      +0.00±0.00     -11.66±7.91
   shared-danger                           32    29/32     +0.38±1.68      -0.41±1.55      +0.72±1.42      -0.75±1.96      -0.13±0.34      +0.09±0.22      +0.00±0.00      -3.00±7.49
   siege-needs-a-breaker                   32    11/32     +0.44±0.89      +0.50±0.85      +0.72±0.75      +1.22±1.05      +0.00±0.21      -0.12±0.19      +0.00±0.00      +6.88±8.11
   ranged-finish-on-the-mean               32    26/32     +0.50±1.61      +0.12±1.62      +1.09±1.29      +0.31±1.32      -0.08±0.34      +0.06±0.25      +0.00±0.00      -1.78±9.72
   capital-defense-holds                   32    17/32     +2.56±1.12*     +2.25±1.14*     +2.53±1.01*     +0.28±0.93      -0.43±0.18      +0.09±0.29      +0.03±0.03      -2.91±7.57
```

```text
== rome: deaths by last order step, gene OFF minus the control, summed over the pairs (largest five moves)
   alternative-genome                   turn_start +104, siege_doctrine +88, tactical +48, barbarian_response +36, battle_planner +26
   safest-stand                         coordinated +22, turn_start +22, retreat +16, escort +9, unordered +9
   shared-danger                        turn_start +29, escort +17, retreat +15, barbarian_response -13, healing +11
   siege-needs-a-breaker                turn_start +18, coordinated +16, healing -7, peacetime +7, positions +7
   ranged-finish-on-the-mean            unordered +25, battle_planner -20, retreat +16, healing +13, siege_doctrine +13
   capital-defense-holds                turn_start +34, siege_doctrine +22, retreat +15, rotation +12, coordinated +10
```

On Rome the live genome's seat is struck far less than live (7.1 rival-major
blows a game, live 11.3) and trades 2.7 kills a loss (live 1.8), so this
recipe is the weaker test of outclassed combat. It is also where two Gran
Colombia reads turn round:

- `capital-defense-holds`: off, +2.56 ± 1.12 units lost (2.3 standard
  errors): on Rome the gene SAVES units, where on Gran Colombia both rounds
  read it costing them.
- `safest-stand`: off, +2.03 ± 1.44 units lost and +2.50 ± 1.00 lost within
  five tiles: on Rome the gene saves units near home.
- `alternative-genome`: +5.41 ± 2.29 units, **+1.22 ± 0.34 cities lost**
  (3.6 standard errors) and -57 ± 16 score: on Rome the force-on list read
  alone is much WORSE than the live genome, the opposite of Gran Colombia.

## A third look (round 4): 48 fresh pairs of each recipe

Rounds 1 and 2 pointed the same way on Gran Colombia for `safest-stand`,
`capital-defense-holds` and `shared-danger` without both reaching 1.5
standard errors, and round 3 turned two of them round on Rome, so all three
were played again on 48 fresh seeds of each recipe (Gran Colombia
64740000-47, Rome 64750000-47; PR head `7a97e664f7` merged with main; 384
games). The controls repeat the earlier rounds: Gran Colombia 28.6 ± 3.1
units and 2.75 ± 0.35 cities lost a world, kill:loss 1.35, 67% of the rival
majors' blows at +10 or more; Rome 15.8 ± 1.8 units and 2.48 ± 0.45 cities,
kill:loss 2.36, 9.3 rival-major blows a game. As printed:

```text
== gran-colombia: one armed gene OFF minus the control, paired by seed (mean ± se a world; * = |d| >= 1.5 se on a loss column)
   gene off                             pairs diverged      units lost       to majors  lost <=5 tiles          killed       kill:loss     cities lost    cities taken           score
   safest-stand                            48    45/48     -1.46±2.47      -0.92±2.45      -1.12±2.58      +2.27±1.56      +0.16±0.13      +0.42±0.32      -0.02±0.04     +0.50±10.41
   shared-danger                           48    45/48     -1.29±2.70      -1.50±2.77      -0.44±2.79      +2.29±1.55      +0.15±0.15      +0.04±0.32      -0.02±0.02     +3.12±10.85
   capital-defense-holds                   48    38/48     -1.71±1.23      -1.44±1.24      -1.88±1.37      +0.83±0.65      +0.12±0.07      -0.29±0.21      +0.00±0.03      +4.77±5.54

== gran-colombia: deaths by last order step, gene OFF minus the control, summed over the pairs (largest five moves)
   safest-stand                         turn_start +57, siege_doctrine -46, rotation -44, retreat +41, healing +33
   shared-danger                        battle_planner -35, coordinated -29, retreat +19, explorer -18, stand_down +18
   capital-defense-holds                coordinated -22, siege_doctrine -22, rotation -19, battle_planner -12, retreat -12
```

```text
== rome: one armed gene OFF minus the control, paired by seed (mean ± se a world; * = |d| >= 1.5 se on a loss column)
   gene off                             pairs diverged      units lost       to majors  lost <=5 tiles          killed       kill:loss     cities lost    cities taken           score
   safest-stand                            48    43/48     +2.77±1.82*     +2.46±1.69      +1.08±1.48      -1.35±1.39      -0.42±0.21      +0.29±0.34      +0.02±0.06    -26.04±12.96
   shared-danger                           48    47/48     +1.44±1.34      +1.50±1.39      +0.60±1.17      -1.17±1.69      -0.26±0.16      +0.15±0.37      -0.02±0.02    -20.94±12.40
   capital-defense-holds                   48    28/48     +0.50±0.86      +0.27±0.80      +0.33±0.68      +2.48±0.85      +0.08±0.14      -0.12±0.21      +0.00±0.03      -5.23±6.61

== rome: deaths by last order step, gene OFF minus the control, summed over the pairs (largest five moves)
   safest-stand                         healing +40, siege_doctrine +27, battle_planner +14, turn_start +13, retreat +11
   shared-danger                        battle_planner +32, healing +31, rotation +25, barbarian_response -16, stand_down +13
   capital-defense-holds                turn_start +71, coordinated -35, barbarian_response +16, rotation +15, escort +12
```

## Verdicts

Every round is reported on its own; no round is pooled with another. Δ is
gene OFF minus the live genome, per world. Units lost:

| gene | GC round 1 (24) | GC round 2 (32) | GC round 4 (48) | Rome round 3 (32) | Rome round 4 (48) |
| --- | --- | --- | --- | --- | --- |
| `capital-defense-holds` | -1.54 ± 1.79 | **-2.22 ± 1.28** | -1.71 ± 1.23 | **+2.56 ± 1.12** | +0.50 ± 0.86 |
| `safest-stand` | -0.88 ± 3.15 | -3.75 ± 2.87 | -1.46 ± 2.47 | +2.03 ± 1.44 | **+2.77 ± 1.82** |
| `shared-danger` | -4.21 ± 2.88 | -1.72 ± 2.20 | -1.29 ± 2.70 | +0.38 ± 1.68 | +1.44 ± 1.34 |
| `ranged-finish-on-the-mean` | **+3.92 ± 2.08** | +0.41 ± 2.12 | - | +0.50 ± 1.61 | - |
| `siege-needs-a-breaker` | +0.75 ± 0.55 | -0.38 ± 0.83 | - | +0.44 ± 0.89 | - |
| force-on list alone | **-5.92 ± 3.27** | **-6.03 ± 3.54** | - | **+5.41 ± 2.29** | - |

Cities lost:

| gene | GC round 1 | GC round 2 | GC round 4 | Rome round 3 | Rome round 4 |
| --- | --- | --- | --- | --- | --- |
| `capital-defense-holds` | **-0.92 ± 0.54** | +0.25 ± 0.29 | -0.29 ± 0.21 | +0.09 ± 0.29 | -0.12 ± 0.21 |
| `safest-stand` | **-0.96 ± 0.54** | -0.50 ± 0.35 | +0.42 ± 0.32 | +0.28 ± 0.24 | +0.29 ± 0.34 |
| `shared-danger` | -0.21 ± 0.59 | +0.00 ± 0.22 | +0.04 ± 0.32 | +0.09 ± 0.22 | +0.15 ± 0.37 |
| `siege-needs-a-breaker` | **+0.17 ± 0.10** | **-0.28 ± 0.17** | - | -0.12 ± 0.19 | - |
| force-on list alone | **-1.04 ± 0.58** | **-0.53 ± 0.35** | - | **+1.22 ± 0.34** | - |

(bold: 1.5 standard errors or more)

**Confirmed harmful: none.** No armed combat gene's removal cut units lost
or cities lost by 1.5 standard errors or more in two independent rounds on
the same recipe. Every round-1 flag that pointed at harm (`safest-stand` and
`capital-defense-holds` on cities, `shared-danger` on units) failed to
repeat on fresh seeds.

**Confirmed helpful (removal hurt): none on Gran Colombia.**
`ranged-finish-on-the-mean` (+3.92 ± 2.08 units in round 1) read +0.41 ±
2.12 on fresh seeds, and `siege-needs-a-breaker`'s cities read changed sign.
On Rome, `safest-stand` off cost units twice (+2.03 ± 1.44, then +2.77 ±
1.82, with -26 ± 13 score): the nearest thing to a confirmed effect here,
small, at the edge of the threshold, and only on the recipe where the seat
is least outclassed.

**Leaning, unresolved: `capital-defense-holds` on Gran Colombia.** Three
independent rounds all read it costing units (-1.54, -2.22, -1.71 a world),
only one of them at 1.5 standard errors; its cities reads do not agree with
one another, and on Rome it read +2.56 ± 1.12 (saving units) once and +0.50 ±
0.86 the next time. If it costs Gran Colombia's seat anything it is about
two units in 28, which this harness cannot separate from zero at 48 pairs.

**Inert: none outright.** Every screened gene changed the game in some
pairs. Nearly inert: `siege-commitment` (5 of 24 pairs diverged, every Δ
within ±0.4). Firing in only some worlds: `siege-needs-a-breaker` (9 of 24,
14 of 32; 11 of 32 on Rome), `threatened-city-buys-its-defender-2` (13 of
24) and `ranged-hp-reserve` (15 of 24), all inside noise. The rest fired in
at least 20 of 24 pairs; those not discussed above read inside noise:
`battle-planner-2`, `siege-train`, `anvil`, `camp-party`,
`defenders-hold-the-walls`, `recon-replacement`.
`battle-planner-2` off read zero on losses but -7.46 ± 3.76 on units killed
(2.0 standard errors, one round): it trades, it does not save.

**The genome matters more than any one gene.** The force-on list read alone
(the genome the edge ledger and the declaration gate simulated) is not the
live seat: on Gran Colombia it loses about six units a world fewer than the
live genome in both rounds (and 0.5-1.0 fewer cities), and on Rome 5.4 more
units and 1.22 more cities (3.6 standard errors), at -57 ± 16 score. The 101
ledger defaults the list leaves off are worth more, one way or the other,
than any armed combat gene screened here, so a simulated combat result
should name which of the two genomes it played.

**Power, plainly.** One round of 24-48 pairs reads units lost to ±1.2-3.5
(one standard error) for a gene that fires in every pair, against 28-30
units lost a world on Gran Colombia and 13-16 on Rome. An effect under
about 3 units a world (10% of losses) is noise at these sizes; so is a
cities-lost Δ under about 0.5. Every number in the tables above that is not
bold, and most that are, is noise.

## What this is not

- **Not a verdict on the win rate.** Our seat won none of the 104 Gran
  Colombia or 80 Rome control games; round 1's Gran Colombia controls ended
  on a rival's Science (18 of 24) or Culture (6) victory at turn 112-165, as
  in `docs/EDGE_LEDGER.md`. The screen reads units, cities and score.
- **Simulated seat, simulated rivals.** The rivals are Firaxis-faithful
  chairs with the host's fitted tactics near our seat, not the host. The
  simulated seat loses more cities than the live one.
- **The census's step names are the last order before a death**, not its
  cause. A gene can move deaths from one step to another without changing
  the total: `battle-planner-2` off takes 285 deaths out of the planner's
  own steps (`rotation`, `battle_planner`, `positions`) over 24 pairs, puts
  108 into `healing` and the rest elsewhere, for a net Δ of +0.04 ± 3.49.
- **Leave one out reads each gene with every other armed gene on.** Genes
  that cover for each other (the siege family, the defence family) can
  each read null alone.
- **Disarming is not the same as off.** Six of the fourteen are also ledger
  defaults; taking them out of the armed set would leave them on. And
  `battle-planner-2` off leaves the battle-planner family empty here, while
  disarming it live would hand the family to the ledger's `battle-planner-3`.
- **One live genome header** (`civvis-20261113T090930Z`, a Rome run; the
  genome does not differ by lane apart from the Score seat's own genes,
  which a Domination seat does not run).

## Reproduce

A round is one ignored test, run where the armed set or a genome header can
be read:

```bash
CIVVIS_LIVE_GENOME=<a live why.log, or a file holding its first line> \
CIVVIS_COMBAT_ALT=$HOME/.civvis-live-force-on \
CIVVIS_COMBAT_RECIPE=gran-colombia CIVVIS_COMBAT_WORLDS=24 CIVVIS_COMBAT_SEED=64710000 \
  cargo test --profile ci --features developer-tools --bin gene_screen \
  combat_genes_outclassed_driver -- --ignored --nocapture
```

`CIVVIS_COMBAT_GENES` names the genes to leave out (default the fourteen
above; `none` plays the control alone), `CIVVIS_COMBAT_REPEAT=1` adds the
control a second time, `CIVVIS_COMBAT_SHARD=k/K` plays one shard,
`CIVVIS_COMBAT_OUT` appends each finished world to a file, and
`CIVVIS_COMBAT_ROWS=a.jsonl,b.jsonl` prints the report over such files
without playing. Each finished world is also written to stderr as a
`COMBAT-WORLD` line.
