# Edge unit takes the soft city

`edge-unit-takes-the-soft-city` (opt-in, off by default, not armed).
Code: `src/ai/advanced/edge_unit_takes_the_soft_city.rs`. Driver:
`src/bin/gene_screen/edge_unit_takes_the_soft_city_tests.rs`. PR #5267; version 2 PR #5292.

## Question

Can the Rome seat use the one window where it is stronger than its
neighbour (a Legion, 40, against an expansion city that defends at about 25
around turn 40) to take that city, by researching, supplying, raising and
mustering the unit early and declaring only when a capture forecast says the
city falls?

**Answer.** Not in the simulator, and the simulator cannot settle it. As
first specified the gene never declared (round 0: 0 of 16; round 1: 0 of
32, and 1 of 32 with the Legion's Iron waived), because the decision comes
too late: the window opens around turn 29, Iron Working lands at turn 48-80,
Legions at 64-100, and the city stops being soft by turn 60. With the
second flag, `edge-unit-opens-from-the-start`, and three mechanical fixes
found in the journals, the whole chain now executes: research from turn 1,
Iron decided at Bronze Working, Legions by turn 44 (Iron waived), a muster,
a denouncement, a declaration and an assault actually fought (walls
broken from 100 to 7 at Tonsberg; Ostia struck from 120 to 20 health). It
executed through the assault in 6 of 32 lane worlds and 4 of 32 walls-0
worlds with the Iron waived, and in 2 of 32 lane worlds with real Iron. It
captured nothing in rounds 0-4: of the 7 assaults read, 5 cities raised
their walls within two turns of the declaration (a sixth within three, the
seventh was walled already), before the Legions closed the last tiles, and
Legions without a Ram barely dent a walled city. The from-the-start opening costs about 16% of Science at turn
50 (-2.3 ± 1.1, lane) and 0.4 cities founded by turn 50. The simulator's
rival cities are walled in 50-100% of games from turn 40 against 13-18% live,
and its chairs wall up behind a declaration; both cut against this gene, so
a simulator miss is weak evidence. A live trial on one carrier is the only
reading that can price it.

## The rule

1. **Edge unit.** The strongest land melee unit (military, no ranged
   strength, no bombard, not siege, not recon) the seat can train now or
   after at most 3 more technologies, a unique unit standing in for the unit
   it replaces. For Rome at the start: Mining, Bronze Working, Iron Working,
   the Legion (40, 10 Iron). The unit is chosen the first turn the gene reads
   the board and held for the game while it stays within reach: asked later,
   a seat that knows Pottery, Writing, Currency and Mathematics finds the
   Pikeman (45, Military Tactics) one technology away and would leave the
   Legion's research for a Medieval unit (the unit test
   `the_unit_chosen_first_is_held_for_the_game` pins this).
2. **Soft city.** A met rival major's city whose centre we have explored,
   within 10 tiles of one of our cities, a legal war target
   (`campaign_target_legal`: never a friend or ally), where the edge unit's
   strength less the city's defence (`Game::city_strength`) is at least 8.
   Behind standing walls it also needs a Battering Ram or Siege Tower the
   walls admit and we can train; without one it is not soft. City-states are
   out of scope.
3. **Window.** Opens on a soft city up to turn 55 on Online (scaled by speed
   as `city_state_harvest::harvest_floor` scales), with at least two cities,
   until the game's one attempt is spent. The named city is kept while it
   stays soft. *Changed after round 0:* a city named by turn 55 stays the
   target, mustered on and declared on, while it stays soft, up to turn 70
   (as first specified the window shut at turn 55, before any force could
   stand at the city). *Version 2:* an unwalled named city stays the target
   below the +8 edge (see "Version 2" below). While the window is open:
   - **Research.** When the edge unit needs a strategic resource the seat
     neither holds (10 or more) nor earns, the technology that reveals it is
     the research goal (Bronze Working reveals Iron). Once it is known, the
     edge unit's own technology becomes the goal only if a source is visible
     inside our borders or on unowned ground within 3 tiles of one of our
     cities; with none the window waits, journals why, and spends no Science.
     The goal sits in `advanced_research`'s forced chain behind the walls,
     the opening Archery and the first-capital openings, ahead of every lane
     beeline, and a technology in progress still finishes.
   - **Resource.** A Builder improves a source inside our borders before any
     other job; a source one tile outside them is bought (down to an empty
     treasury while income is not negative).
   - **Force.** Edge units, up to 6 owned and queued, while the resource
     allows, trained in the capital and the one other city nearest the target
     (an idle-queue claim in `run_idle_queue_claims`, so the live production
     hint asks it too), plus one breaker when the walls stand. A Gold upgrade
     of an owned unit into the edge unit (Warrior to Legion) is taken ahead of
     the treasury floor, down to 0 Gold while income is not negative. The
     window needs two cities, so the first Settler is never deferred.
     *Version 2:* three cities, ahead of a deferrable item, and the upgrade
     bill held back from other purchases (see "Version 2" below).
   - **Muster.** Before the war the edge units (and the breaker) walk to a
     free stand 3-4 tiles from the city outside any border closed to us (3-6
     when none is free) and hold it. *Changed after round 0:* the walk runs at
     the start of the turn with every move the unit has and fortifies it on
     the stand, which leaves it no move for a later step to spend; the unit
     pass's staging hook takes any further step. The objective board's
     Reserve gathers at the stand nearest our capital, and the board keeps
     the force out of distant skirmish rows, as it does for the shipped
     opening's force.
   - **Declare.** When at least 3 edge units at 70+ health stand within 4
     tiles of the city (and a breaker that opens its walls, if they stand), no
     other major is at war with us, and the capture forecast
     (`assault_forecast_at_peace` over our land soldiers and breakers within
     4 tiles, calibrated by `calibrated_within`) reads the city taken within 4
     turns at 0.5 or more, the seat declares through the shipped
     `preferred_war_opening`. On a major with no casus belli that function
     returns a Denounce and offers the Formal War five standard turns later;
     *changed after round 0*, the denouncement goes out once 3 edge units are
     owned (*version 2:* once the first is), so the Formal War is legal when
     the force musters, and a Denounce
     offered at the muster is applied and held. From the declaration the
     plan's target player and city are that city's
     (`apply_edge_unit_to_strategy`), under the lane's own strategy; the
     objective board's Siege row reads it.
   - **After a capture.** On to the same owner's next city if it is within 8
     tiles of the force and the edge is still 5 or more; otherwise peace is
     asked and the window closes. Peace is also asked after 20 war turns
     without a capture and when fewer than 2 edge units remain.
4. **Journal.** Every stage writes a line whose detail begins
   `edge-unit-takes-the-soft-city: <stage>:` (unit, waiting, opened, research,
   resource, force, hold, denounced, declared, captured, continue, peace,
   closed) and bumps an `edge_unit:<stage>` counter on the board the seat
   plans on. A hold names its reason, and a muster that does not form says
   where the rest of the force stands.
5. **Off.** Every entry point returns before it reads the board.
6. **At war** (*changed after round 3*). The gene presses its own assault at
   the start of each war turn: breakers step to a free tile beside the city,
   then each edge unit with a move steps beside it and, at 35 health or more,
   strikes it (`Action::Attack` on the city tile). One `siege:` line a turn
   records the city's health and walls and the units beside it. A
   declaration that never reached the board (no war on record since it) is
   asked again, up to three times.
7. **Muster reach** (*changed after round 2*). When the rival's border has
   closed every stand 3-4 tiles from the city, the muster counts the force
   within 6 tiles, the ring the walk falls back to.

### `edge-unit-opens-from-the-start` (second opt-in flag, needs the first)

For a seat whose edge unit is its civilization's own unique unit (decided
on turn 1: Rome's Legion; a generic Swordsman or Horseman does not qualify):

- **From turn 1** the research goal is the path to the unit, before any city
  is named or a second city founded: the technology that reveals its
  resource first (Mining, then Bronze Working).
- **At Bronze Working** the existing resource rule decides: a source in reach
  makes the unit's own technology (Iron Working) the goal; none stands the
  opening down and research returns to normal (two cheap technologies
  spent). *Version 2:* the check first repeats every turn for 10 Online
  turns. The first gene's ordinary window goes on and opens later if a
  source comes into reach.
- **With the source in reach**: a Builder improves it first (also Iron
  Working's boost); the capital and the city nearest the target (the
  capital alone before a city is named) train up to three bodies that
  upgrade into the unit (Warriors) while the unit itself cannot be trained;
  and their upgrade bill is held back from every discretionary Gold
  purchase (`purchase_upkeep_floor` returns at least that bill while the
  opening is live). The upgrades then go ahead of the treasury floor as in
  the first gene.
- **Naming and deadlines**: the target is named when a soft city is
  explored, as in the first gene. No soft city within 10 tiles known by turn
  45, or no declaration by turn 60 (Online, scaled by speed), stands the
  opening down and journals it; the edge units stay as defence.
- Everything after the muster is the first gene (denounce, forecast gate,
  declaration, assault, continue or peace).

## Version 2, from the first live game (PR #5292)

`civvis-20261113T205538Z` (Rome, both flags): no Iron in reach at Bronze
Working (turn 19), so the opening stood down, yet Iron was in stock from
about 23. Opened at 27 on Mbamba (defence 17, unwalled, 8 tiles); Legions
owned 1 at 36 (a Warrior upgraded for 75 of 76 Gold, walking at 50 health
to 48), 3 at 54, 5 at 75. Denounced at 54; closed at 56 "no longer soft"
at edge +6 (defence 34); Mbamba was never walled to turn 81.

**Why nothing was queued from 33 to 44.** The claim took only idle queues,
in Rome and Antium; the claim chain waits for the opening book, which held
Rome to 37 (an Archer at 34, a Builder at 37); the early Settler floor,
ahead of the claim, took Rome at 39 and Antium at 40; the Granary claim,
after it, put Granaries ahead of queued Legions (45, 50).

Changes (same two tags, still opt-in):

1. **Unwalled target on the forecast.** Past naming, an unwalled city below
   +8 stays the target while the declaration's forecast, with every edge
   unit owned posed beside it at its health, takes it within 4 turns at
   0.5+; before 3 are owned, while the edge is -4 or better (`kept:`).
   Walled: the old rule. The close names why.
2. **Force.** The capital and the two cities nearest the target train it at
   once, ahead of a deferrable item (never one finishing this turn, a
   soldier, Walls, a wonder, a project, a District with work in it, or a
   city's first Settler: only the capital's gives way); the claims that
   respect a reserved army build leave it queued; in the opening book, once
   a city is named, the turn start fills idle queues with the edge unit.
3. **Gold** for the upgrades is held whenever the window is open.
4. **Denounce** with the first edge unit, never before one exists.
5. **Heal.** Under 70 health (above the withdraw line) an edge unit holds,
   or steps into a city of ours within 2 tiles, before it walks (`heal:`).
6. **Recheck.** From the start, no source at the revealing technology is
   checked every turn for 10 Online turns (`start: recheck:`) first.
7. **Upgrade passes.** The edge pass (`maintain_edge_unit_window`) runs
   first, before `upgrade_units_preserving_air_wing`, whose first step is
   `one-upgrade-lifts-every-city`; while Gold is held that pass takes only
   an upgrade into the edge unit or one leaving the held Gold. A Warrior to
   Legion is bought once, by either, and the edge force counts it.

**Sanity round** (16 pairs, 110 turns, lane recipe, seeds 52670500-15;
worlds reaching each stage, in brackets of those with Iron in reach at
Bronze Working; round 4 on the same seeds, `main` has moved since):

| | goal | resource | 3 Legions | muster | denounced | declared | assault pressed | captured |
|---|---|---|---|---|---|---|---|---|
| round 4, gene-start | 16 (4) | 12 (4) | 5 (3) | 1 (0) | 1 (0) | 1 (0) | 1 (0) | 0 |
| v2, gene-start | 16 (4) | 10 (4) | 7 (3) | 1 (0) | 4 (0) | 0 | 0 | 0 |
| round 4, free Iron | 16 (4) | 10 (4) | 15 (4) | 4 (1) | 6 (1) | 3 (1) | 3 (1) | 0 |
| v2, free Iron | 16 (4) | 12 (4) | 16 (4) | 4 (1) | 6 (2) | 2 (0) | 2 (0) | 0 |

CI 43511045918 found a regression, fixed for the table's run (CI
43514717689): the book-phase claim trained Warriors from turn 1 in place
of the book's Scout and Settlers (free Iron: 4 windows opened, not 7).
In it the real-Iron arm's one muster (Susa) came 3 turns before round 4's
(Iron found on the recheck, denounced at the first Legion) and met walls
with no Ram; round 4 declared on Susa unwalled, walled two turns later.
Cities founded, paired: -0.56 ± 0.27 by turn 50 (round 4 -0.25), -0.19 by
100.

## Live evidence (Rome, Score lane)

The two census scripts (`window.py`, `iron.py`) read every run of 11-12/13
whose `why.log` header says `"victory": "score"` and whose event log seats
Rome: 114 runs, 96 of them with an event log of 8 MB or more (the scripts'
own floor). n below is the games with a reading at that turn.

The nearest rival major city to any of ours:

| turn | n | tiles | defence (p25, p75) | walls up | pop | Legion 40 minus defence: median, share at +5 | our best land unit |
|---|---|---|---|---|---|---|---|
| 40 | 59 | 6 | 25 (22, 27) | 14% | 3 | +15, 97% | 20 |
| 50 | 72 | 5 | 27.5 (23, 37) | 18% | 4 | +12.5, 75% | 25 |
| 60 | 85 | 5 | 39 (35, 44) | 34% | 5 | +1, 33% | 28 |
| 70 | 85 | 5 | 45 (42, 50) | 46% | 6 | -5, 8% | 36 |
| 80 | 82 | 5 | 51 (47, 57) | 60% | 6 | -11, 5% | 36 |

The rival's public military stood at 309 against our 125 at turn 40 and 344
against 163 at turn 50. The nearest city-state city was walled in 95-99% of
games at every mark and defended at 30 / 40.5 / 48 at turns 40 / 50 / 60.

Iron and the Legion (96 games):

| turn | n | Iron Working | Iron 10+ | Iron income | any Legion | 3+ Legions |
|---|---|---|---|---|---|---|
| 40 | 86 | 33% | 1% | 5% | 0% | 0% |
| 50 | 86 | 64% | 9% | 17% | 1% | 0% |
| 60 | 88 | 90% | 26% | 28% | 9% | 1% |
| 70 | 88 | 100% | 38% | 39% | 6% | 2% |
| 80 | 84 | 100% | 40% | 52% | 5% | 2% |

Iron Working came at a median turn 47 (96 games), the first Iron stock or
income at a median turn 64 (69 games), and a first Legion in 13 games
(median turn 55).

The peer session `-93` measured the same lane today (its readers and tables
are `census.tsv` / `census_read.py` in that session's scratchpad, and the
closing comment of PR #5237): the assault census on 40 live Rome mirrors read
a rival major's city takeable (calibrated 0.5 or better within 8 turns) in
0-5% of games at turns 30-60 with the army held, 33-40% with 6 Legions and a
Ram added, and 57-68% at turns 40-60 with 8 Legions, a Ram and 2 Catapults;
city-states read about 0% in every arm. Iron lay within 3 tiles of one of
our cities at turn 40 in 40% of games. A local sim of a 4-8 Legion corps
with the Iron waived (60 pairs) raised military by 54 at turn 75, saved no
city, cost 21% of Science at turn 50 through the research goal, and the
campaign board named no city in any arm.

## Method

A paired whole-world driver, `src/bin/gene_screen/edge_unit_takes_the_soft_city_tests.rs`
(an ignored test, modelled on #5251's), played on GitHub's runners through a
temporary workflow on this PR: one build, worlds sharded by seed, every arm
of a seed in one process, one report over every shard.

- **Recipe** (the Rome lane, war-faithful): `--players 6 --width 74 --height 46
  --city-states 9 --map continents --difficulty deity --rivals
  firaxis-faithful --handicap rivals --rival-chairs 5 --rival-pace
  --rival-war-pace 100 --rival-host-tactics --target-mix science --seat-civ
  Rome`, Online speed, no `--rival-rung` (native competitions on, as #5251
  played it). Round 1 adds a labelled sensitivity, the same recipe with
  `--rival-walls 20` (see Round 0).
- **Genome**: line 1 of `civvis-20261113T160115Z/why.log`, the newest live run
  of 11-13 with a header (711 tags, `"victory": "science"`).
- **Arms**, same seeds and game index: `control`; `gene` (control + this
  gene); `stack` (control + `early-conquest-opening`,
  `conquest-takes-the-soft-city`, `conquest-opening-stays-near`,
  `first-capital-strikes-the-open-window` and `first-capital-before-the-walls`,
  which the strike requires, and without `early-conquest-stands-down`, which
  holds every opening shut on the live seat; the two first-capital genes act
  only on a Domination seat, so on this Science seat the stack is the
  opening with the soft-first ranking and the near reach); `gene-free-iron`
  (the gene, with the Legion's Iron cost and requirement removed from that
  world's rules: can Legions take the city at all).
- **Reading**: an observer reads the Roman seat at the start of every turn
  (Bronze and Iron Working, Iron in reach at Bronze Working, first Iron and
  first Legion, Legions owned, the nearest rival major city to ours, cities,
  techs, Science and military at marks, cities founded, every city taken or
  lost, every war with a major and who declared it). The gene's stages come
  from the seat's journal: the simulated seat plans each frame on a decision
  view and only its logged actions reach the board, so a counter the gene
  writes never reaches the observed game (round 0 read zero everywhere until
  the driver read the journal instead).
- **Scaffolding**: the temporary workflow
  (`.github/workflows/edge-unit-takes-the-soft-city.yml`) and the genome
  copy (`src/bin/gene_screen/edge_unit_live_genome.txt`) were removed in the
  PR's final commit. Restore them with `git checkout 2a37abe5a0 --
  .github/workflows/edge-unit-takes-the-soft-city.yml
  src/bin/gene_screen/edge_unit_live_genome.txt` (the round-4 workflow;
  earlier rounds' settings are in the commits titled "WIP experiment";
  version 2's sanity round is commit `593f2b3027` of #5292).
- **Statistics**: mean ± standard error of arm minus control, paired by seed,
  `*` at 1.5 standard errors; rounds are never pooled (a PR's CI plays the
  merge with current `main`, so a seed is a different world after `main`
  moves).

## Results

### Round 0: fidelity and firing (16 worlds, 100 turns, seeds 52670000-15, CI run 43467247559)

The nearest rival major city to ours in the control arm, against the live
census:

| turn | sim defence (p25, p75) | live | sim walls up | live | sim pop | live | sim capital | Legion minus defence, at +8 (sim) |
|---|---|---|---|---|---|---|---|---|
| 40 | 29 (26, 34) | 25 (22, 27) | 88% | 14% | 5 | 3 | 25% | +11, 62% |
| 50 | 35 (31, 37) | 27.5 (23, 37) | 100% | 18% | 6 | 4 | 25% | +5, 31% |
| 60 | 36 (32, 39) | 39 (35, 44) | 94% | 34% | 7 | 5 | 25% | +4, 31% |
| 70 | 44 (36, 54) | 45 (42, 50) | 100% | 46% | 7 | 6 | 25% | -4, 6% |

The control seat knew Bronze Working at a median turn 82 (16 worlds) and Iron
Working at 88 (10 of 16 by turn 100); it had a first Iron at 78 (8 of 16)
and a first Legion at 92 (4 of 16). That matches the live Science lane:
the first six live Science games of 11-13 (16:01Z-17:10Z starts, read by the
coordinator) took Bronze Working at turns 59-82 (median about 75), Iron
Working at 62-101 (median 76), Writing at 24-33, and owned no Legion. (The
Score-lane census above, Iron Working at 47, is the earlier lane.)

**The simulator's nearest rival cities are not the soft cities the live seat
sees.** They are walled in 88-100% of games from turn 40 (live 14-46%), a
few points stronger at turns 40-50, larger, and a quarter of them are
capitals (live 2-4%). This is the CIVVIS chairs' known habit (`veto_walls` in
`src/bin/gene_screen/rival_war_pace.rs`: "the CIVVIS chairs wall far faster
than the live Firaxis field"), and the coordinator's recipe leaves
`--rival-walls` unset. The gene still finds soft cities (a city other than
the nearest, or one not yet walled), but what the sim says about taking them
is about walled cities that the live seat mostly does not face. This is the
one fidelity gap that stands: the seat's own research is as late as the live
Science lane's.

What the arms did (16 worlds each):

- **gene**: the window opened in 11 of 16 worlds (median turn 26), against
  cities at defence 10-29 and edges of +11 to +30. It pulled Bronze Working
  from turn 82 to 42 and Iron Working from 88 to 48 (15 of 16). It waited
  for want of an Iron source in 2 worlds, bought an Iron tile in 2, and a
  Builder mined Iron in 1. Iron, not research, set the pace: a first Legion
  in 9 of 16 worlds at a median turn 67, mostly Warriors upgraded for 75
  Gold. **It declared nowhere.**
- **gene-free-iron**: a first Legion in 15 of 16 at a median turn 48, 1.4
  Legions a world at turn 50 and 3.2 at turn 60; 13 worlds held 3-6 Legions
  by turns 44-55. **It declared nowhere either**: no muster of three Legions
  at the city formed in any world, and the window as specified (open to turn
  55) closed at turn 56 on every one of them.
- **stack**: the opening named a city in 7 of 16 worlds and declared in 4
  (turns 39-45); all 4 yielded within 2-10 turns because the target walled up
  and no breaker was held. No city taken.

Paired against the control (n = 16):

| | gene | gene-free-iron | stack |
|---|---|---|---|
| Science t50 | -0.73 ± 0.36* (of 15.1) | -0.61 ± 0.33* | +0.44 ± 0.45 |
| Science t75 | -3.12 ± 1.44* (of 47.0) | -4.72 ± 1.81* | -1.83 ± 1.68 |
| Science t100 | +0.18 ± 3.11 (of 95.3) | -4.07 ± 3.61 | -0.30 ± 3.04 |
| Legions t60 | +0.31 ± 0.18* | +3.19 ± 0.63* | +0.06 ± 0.06 |
| military t75 | -27 ± 21 (of 375) | -42 ± 26* | -37 ± 27 |
| cities founded by t75 | +0.06 ± 0.27 (of 7.1) | +0.00 ± 0.24 | -0.38 ± 0.20* |
| cities lost by t100 | +0.06 ± 0.17 (of 0.44) | -0.13 ± 0.13 | -0.13 ± 0.16 |
| score (t100) | -4.2 ± 6.6 (of 286) | -6.4 ± 7.7 | -4.4 ± 4.2 |

**Why the gene did not fire, from the journal**, and what changed for round
1 (commit `696be3fe2a`):

1. *The window shut before the force stood at the city.* Legions arrive at
   turns 44-55 even with the Iron free, and a window shut at turn 55 left no
   time to walk 7-9 tiles. A city named by turn 55 now stays the target while
   it stays soft (edge 8 or more, re-read each turn), up to turn 70.
2. *No muster formed at all.* With 7-15 turns in hand, 13 worlds' 3-6 Legions
   never stood three within four tiles of the city; the unit pass's staging
   step alone did not get them there. At the start of each turn the force now
   walks with every move to a free stand 3-4 tiles from the city and holds it
   fortified (a fortified unit has no moves left, so no later step of the
   turn walks it home). A muster that still does not form journals where the
   rest of the force stands.
3. *The shipped opening denounces first.* A unit test of the declaration
   failed in CI: on a major with no casus belli, `preferred_war_opening`
   returns a Denounce and offers the Formal War five standard turns later.
   The denouncement now goes out once three edge units are owned, so the
   Formal War is legal by the time the force musters; a Denounce offered at
   the muster is applied and held, not read as a refused war.

The round-0 numbers above measure the gene as first specified (open to turn
55): its cost is the research pull (Science -5% at turn 50, -7% at turn 75)
and it buys no city.

### Round 1: 32 pairs, 150 turns, seeds 52670100-31 (CI run 43470701864)

The gene as changed after round 0 (`696be3fe2a`). Games end at a median
turn 137-142 on a rival's Science Victory (28 of 32 control games; culture
3, score 1), so the turn-150 readings have n = 2 and the table reads the
end of the game instead.

Fidelity of the control (32 worlds): the nearest rival major city defends at
28 / 33 / 40 / 42 at turns 40 / 50 / 60 / 70 (live 25 / 27.5 / 39 / 45), walled
in 84% / 88% / 81% / 81% (live 14% / 18% / 34% / 46%), population 5-7 (live
3-6). The seat knew Iron Working at a median turn 90 and had a first Legion
in 21 of 32 at 105. Iron lay in reach at Bronze Working in 23 of 32 control
worlds and 19 of 32 gene worlds (the gene reaches Bronze Working at turn 48,
the control at 86, with fewer cities).

What the arms did:

- **gene**: opened in 15 of 32 (median turn 29); waited for want of an Iron
  source in 10; bought 2 Iron tiles; 5 upgrades and 10 Legions trained over
  all worlds; a first Legion in 20 of 32 at a median turn 100. **No muster,
  no declaration, no capture.**
- **gene-free-iron**: opened in 16; 31 upgrades and 87 Legions trained; a
  first Legion in 29 of 32 at turn 64, 1.6 Legions a world at turn 60. Three
  Legions stood at the city in 5 of the 16 worlds that opened: in 3 the walls
  stood and no Ram was with them, in 1 (Gelonos) the forecast read 0-62% over
  16 turns and never held at 50% on a turn the Formal War was legal, and in 1
  the seat declared (Kaifeng, turn 67, three Legions and a Ram, forecast 83%
  within 4 turns), took nothing in 20 war turns and asked for peace. In
  another world it denounced Greece at turn 56 and a different path of the
  controller declared the Formal War at turn 73, after the window had shut.
  The 16 windows closed at turn 70 with the force still mustering (9), on a
  named city no longer soft (3), with nothing named at turn 55 (3), and after
  the war (1).
- **stack**: the opening named a city in 6 of 32 and declared in 4 (turns
  36-64); no city taken.

Paired against the control (n = 32; n is lower where a reading is missing):

| | gene | gene-free-iron | stack |
|---|---|---|---|
| declarations by the gene / the opening | 0 | 1 | 5 |
| wars we declared on majors | +0.00 ± 0.00 (of 0.06) | +0.16 ± 0.07* | +0.16 ± 0.07* |
| cities taken (end) | -0.03 ± 0.03 (of 0.09) | -0.03 ± 0.03 | -0.03 ± 0.03 |
| cities lost (end) | +0.28 ± 0.29 (of 0.91) | -0.06 ± 0.14 | -0.16 ± 0.11 |
| wars declared on us by t100 | +0.09 ± 0.07 (of 0.47) | +0.09 ± 0.07 | +0.00 ± 0.00 |
| units lost | -0.50 ± 1.23 (of 10.8) | -1.47 ± 1.55 | -0.09 ± 0.43 |
| cities founded by t75 | +0.00 ± 0.13 (of 6.66) | +0.03 ± 0.10 | -0.31 ± 0.14* |
| cities founded by t100 | +0.10 ± 0.12 (of 7.74) | -0.10 ± 0.18 | -0.45 ± 0.20* |
| cities owned t100 | +0.07 ± 0.11 (of 7.52) | -0.16 ± 0.17 | -0.45 ± 0.19* |
| Science t50 | +0.04 ± 0.16 (of 13.4) | -0.08 ± 0.17 | -0.03 ± 0.05 |
| Science t75 | -0.45 ± 0.90 (of 41.8) | -2.17 ± 0.91* | -0.64 ± 0.65 |
| Science t100 | -1.75 ± 2.05 (of 84.7) | -3.24 ± 1.81* | -2.28 ± 1.39* |
| Science t125 (n 27-29) | -6.26 ± 4.98 (of 111.5) | -4.80 ± 4.63 | -0.47 ± 3.06 |
| Science at the end | -7.11 ± 4.39* (of 128.0) | -8.27 ± 5.45* | -0.74 ± 2.15 |
| techs t100 | +0.10 ± 0.17 (of 27.1) | +0.07 ± 0.23 | -0.42 ± 0.27* |
| techs at the end | -0.94 ± 0.51* (of 38.6) | -0.66 ± 0.38* | +0.06 ± 0.18 |
| Legions t60 | +0.09 ± 0.05* | +1.56 ± 0.37* | +0.00 ± 0.00 |
| military t75 | +5.6 ± 8.3 (of 307) | +6.9 ± 7.9 | -3.0 ± 4.8 |
| score | -11.4 ± 9.9 (of 376) | -7.1 ± 7.1 | -4.4 ± 3.7 |
| last turn | -0.7 ± 1.3 (of 137) | -0.1 ± 1.2 | +1.3 ± 0.8 |
| seat won | 0 of 32 in every arm | | |
| winner was a rival we declared on | 1 of 32 in every arm (a late war) | | |

**Why the free-Iron force still did not take a city, from the journal.** The
muster line names where the force stands each turn:

1. *The force trickles.* Legions come one at a time from the capital and the
   nearest city, 9-15 tiles from the named city, so the first stands at the
   city while the last is still ten tiles out (Corinth, 52670131: "0 of 4
   ... at 8, 11, 11, 12 tiles" at turn 56, "2 of 5 ... at 5, 5, 6 tiles" at
   turn 64).
2. *The stand ring closes.* By turns 55-60 the rival's border can cover the
   ring 3-4 tiles from its city; the walk then falls back to stands 5-6
   tiles out, which a muster counted within 4 tiles does not see (in 3
   worlds the nearest Legion not counted stood at 5-6 tiles on each of the
   last five muster lines).
3. *The edge goes first.* The named city's defence passes 32 (a Legion's 40
   less 8) within 15-25 turns of being named: 3 free-Iron windows and 2 gene
   windows closed "past the naming turn the named city is no longer soft".
4. *Walls.* The sim's cities wall up early; 3 of the 5 musters waited on a
   Ram the force did not have.
5. *A muster that formed did not always read as a capture*: Gelonos read 0%
   to 62% (short of a taker); in the walls-20 sensitivity Uruk read 0% and
   Ostia 85% while the Formal War still waited on the denouncement; Kaifeng
   read 83%, was declared on and was not taken in 20 turns.

**Sensitivity, `--rival-walls 20` (16 pairs, same seeds).** The throttle did
not make the sim's cities soft: the nearest rival city still stood walled in
50% / 81% / 81% / 88% of control games at turns 40-70. Neither gene arm
declared (the free-Iron arm stood three Legions at the city in 2 worlds and
read 0% at Uruk and 85% at Ostia), no arm took a city, and the only difference
at 1.5 standard errors was the free-Iron arm's Science at turn 75 (-4.5 ±
2.1, of 42.6).

### Round 2 (cancelled) and the walls calibration

Round 2 (48 pairs, 250 turns, fresh seeds) was cancelled at the
coordinator's request: a confirmation of round 1's null was not worth its
hour.

The walls calibration (CI run 43474124444; the control alone, 16 worlds, 71
turns, seeds 52670300-15) looked for the `--rival-walls` share whose nearest
rival city walls up as the live one does:

| `--rival-walls` | t40 | t50 | t60 | t70 |
|---|---|---|---|---|
| live (96 Rome Score games) | 14% | 18% | 34% | 46% |
| live (coordinator's reading) | 13% | 16% | 38% | 49% |
| unset (round 1, 32 worlds) | 84% | 88% | 81% | 81% |
| 20 (round 1, 16 worlds) | 50% | 81% | 81% | 88% |
| 10 | 62% | 56% | 56% | 62% |
| 5 | 69% | 69% | 50% | 50% |
| 0 | 62% | 62% | 62% | 62% |

The throttle cannot get there. Even at 0, which turns every Walls build a
chair queues into a soldier, 62% of the nearest rival cities stand walled
from turn 40. `veto_walls` (`src/bin/gene_screen/rival_war_pace.rs`) only
replaces a Walls build at the head of a chair's queue on the chair's own
turn, so walls bought with Gold, or reaching the head between those reads,
are not throttled; which of these the chairs use was not checked. The
differences
between 0, 5 and 10 are within the noise of 16 worlds. Round 3 uses 0, the
strongest throttle, as the "live-like walls" recipe: about four times the
live share at turns 40-50 and within 15 points at turns 60-70.

### Round 3: the opening from the start (32 pairs a recipe, 110 turns, seeds 52670400-31, CI run 43475745803)

Arms: control; gene (the first gene alone); gene-start (both flags);
gene-start-free-iron (both flags, the Legion's Iron waived). Recipes: the
lane, and the lane with `--rival-walls 0`.

The chain, worlds reaching each stage (of 32; in brackets, of the worlds
with Iron in reach at Bronze Working):

| | research goal | resource | 3 Legions | muster | denounced | declared | assault fought | captured |
|---|---|---|---|---|---|---|---|---|
| lane, gene | 19 (13/21) | 15 (13) | 11 (10) | 2 (2) | 3 (3) | 0 | 0 | 0 |
| lane, gene-start | 32 (12/12) | 24 (12) | 14 (10) | 1 (1) | 5 (4) | 0 | 0 | 0 |
| lane, gene-start-free-iron | 32 (12/12) | 20 (11) | 31 (12) | 8 (3) | 17 (7) | 4 (2) | 1 (0) | 0 |
| walls-0, gene | 20 (11/20) | 14 (12) | 10 (9) | 0 | 3 (3) | 0 | 0 | 0 |
| walls-0, gene-start | 32 (12/12) | 21 (11) | 10 (8) | 0 | 3 (3) | 0 | 0 | 0 |
| walls-0, gene-start-free-iron | 32 (12/12) | 22 (11) | 31 (11) | 8 (2) | 15 (6) | 4 (1) | 0 | 0 |

("Resource" is a source bought or improved, or Iron held by turn 80. In
the free-Iron arms Iron is not needed and the column is incidental.)

What failed, from the journal of the 8 declared worlds: in 3 the
declaration never reached the board (logged on the seat's decision view,
the frame's batch cut short; no war on record) and the window closed the
next turn on "the war has ended"; in 4 of the other 5 the force stood
within four tiles for 10-20 war turns with no unit beside the city, against
walled and unwalled cities alike (Toronto, walls 0, 200 health for 12
turns): the shipped war machinery did not press the attack on this Science
seat. In the fifth (Osaka) three Legions and a Ram stood beside the city and
took its walls from 100 to 34, but its health never fell below 189. Both
were fixed for round 4 (rules 6 and the redeclaration above).

### Round 4: after the assault fix (32 pairs a recipe, 110 turns, seeds 52670500-31, CI run 43478040718)

Same arms and recipes, fresh seeds. Games end at turn 110 (the cap).

Fidelity of the control: the nearest rival major city is walled in 75 / 81
/ 81 / 81% of lane games at turns 40 / 50 / 60 / 70 and 50 / 75 / 72 / 66%
on walls-0 (live 13 / 16 / 38 / 49%); it defends at 27 / 32 / 38 / 43 (lane).

Technology turns (median; lane):

| | Mining | Pottery | Writing | Bronze Working | Iron Working | first Legion | third Legion | muster |
|---|---|---|---|---|---|---|---|---|
| control | 12 | 20 | 32 | 84 | 86 | 98 (13/32) | 106 (5) | none |
| gene-start | 7 | 32 | 39 | 20 | 64 | 66 (19) | 56 (10) | 54 (2) |
| gene-start-free-iron | 7 | 45 | 50 | 20 | 39 | 39 (32) | 44 (30) | 51 (9) |

On walls-0 gene-start: Bronze Working 20, Iron Working 55, third Legion 77
(8 worlds). The from-the-start opening stood down at Bronze Working in 24
of 32 worlds on each recipe: at turn 20 the seat holds one or two cities,
and Iron lay in reach of them in 8 of 32 (the control, at turn 84 with
more cities, 25-26 of 32).

The chain (of 32; in brackets, of the 8 worlds with Iron in reach at Bronze
Working in the gene-start arms, 17 in the gene arm):

| | research goal | resource | 3 Legions | muster | denounced | declared | assault fought | captured |
|---|---|---|---|---|---|---|---|---|
| lane, gene | 19 (8) | 16 (10) | 7 (4) | 1 (1) | 1 (1) | 0 | 0 | 0 |
| lane, gene-start | 32 (8) | 22 (8) | 10 (6) | 2 (0) | 2 (0) | 2 (0) | 2 (0) | 0 |
| lane, gene-start-free-iron | 32 (8) | 22 (8) | 30 (7) | 9 (3) | 12 (3) | 6 (2) | 6 (2) | 0 |
| walls-0, gene | 18 (7) | 11 (5) | 10 (5) | 3 (1) | 5 (2) | 2 (0) | 1 (0) | 0 |
| walls-0, gene-start | 32 (8) | 22 (8) | 8 (5) | 1 (0) | 1 (0) | 0 | 0 | 0 |
| walls-0, gene-start-free-iron | 32 (8) | 18 (8) | 31 (8) | 11 (2) | 13 (2) | 5 (2) | 4 (1) | 0 |

The two real-Iron declarations (lane) came after the opening had stood down
at Bronze Working and the first gene's ordinary window found Iron later.

**Why nothing was taken, from the journal of the assaults:** the city walls
up behind the declaration. Of the 7 declared worlds read, walls went from
0 to 100 within two turns of the declaration in 5 (Susa, Osaka, Temuco,
Bandung, Chengdu) and within three in a sixth (Ostia); the seventh
(Tonsberg) was walled already. The Legions took one to three turns to close
the last two or three tiles from their stands. Against standing walls with no Ram, Legion blows took the city from
200 to 199 (Osaka, Bandung, Chengdu) and the Legions died to the city's
strike until fewer than two were left and peace was asked. With a Ram
(Tonsberg) the walls fell from 100 to 7 and the city to 154 health before
the force was spent; at Ostia, unwalled at the declaration, one blow took
it from 120 to 20; it healed to 163 the next turn and walled up the turn
after.

Paired against the control (n = 32; mean ± se of arm minus control):

| lane | gene | gene-start | gene-start-free-iron |
|---|---|---|---|
| Science t50 (of 14.6) | -0.28 ± 0.22 | -2.29 ± 1.10* | -4.48 ± 1.18* |
| Science t75 (of 42.0) | -0.19 ± 0.88 | -2.47 ± 1.33* | -7.93 ± 2.17* |
| Science t100 (of 89.2) | +1.51 ± 3.29 | -5.85 ± 4.03 | -6.82 ± 5.70 |
| cities founded by t50 (of 4.75) | +0.03 ± 0.06 | -0.38 ± 0.18* | -0.44 ± 0.16* |
| cities founded by t75 (of 7.03) | +0.00 ± 0.16 | -0.28 ± 0.25 | -0.31 ± 0.28 |
| cities founded by t100 (of 8.47) | -0.09 ± 0.16 | -0.34 ± 0.27 | -0.38 ± 0.30 |
| cities owned t75 (of 6.84) | -0.03 ± 0.15 | -0.44 ± 0.25* | -0.34 ± 0.28 |
| cities owned t100 (of 8.16) | -0.06 ± 0.19 | -0.41 ± 0.27 | -0.41 ± 0.31 |
| cities taken by t60 / t80 | 0 / 0 | 0 / 0 | 0 / 0 |
| cities lost by t100 (of 0.31) | -0.03 ± 0.08 | +0.06 ± 0.14 | +0.03 ± 0.11 |
| units lost (of 5.6) | -0.16 ± 0.99 | -0.94 ± 1.05 | -0.06 ± 1.07 |
| score at t110 (of 309) | +0.7 ± 5.2 | -13.0 ± 6.7* | -24.7 ± 7.2* |

| walls-0 | gene | gene-start | gene-start-free-iron |
|---|---|---|---|
| Science t50 (of 14.2) | -0.15 ± 0.26 | -1.86 ± 0.89* | -3.86 ± 0.83* |
| Science t75 (of 41.7) | -2.28 ± 1.33* | -0.91 ± 1.60 | -6.12 ± 2.34* |
| Science t100 (of 88.1) | -2.31 ± 2.19 | -2.39 ± 3.97 | -5.67 ± 5.05 |
| cities founded by t50 (of 4.56) | -0.06 ± 0.10 | -0.03 ± 0.17 | -0.22 ± 0.21 |
| cities founded by t100 (of 8.31) | +0.03 ± 0.17 | -0.13 ± 0.30 | -0.31 ± 0.29 |
| cities owned t100 (of 8.00) | -0.13 ± 0.17 | -0.28 ± 0.32 | -0.31 ± 0.30 |
| cities taken by t60 / t80 | 0 / 0 | 0 / 0 | 0 / 0 |
| cities lost by t100 (of 0.31) | +0.16 ± 0.07* | +0.19 ± 0.12* | +0.00 ± 0.11 |
| score at t110 (of 299) | -3.9 ± 3.5 | -5.5 ± 7.1 | -12.8 ± 7.2* |

Round 3's costs on its own seeds agree: gene-start on the lane cost Science
-2.58 ± 0.61 at turn 50* (of 13.5) and nothing at turn 100 (+0.75 ± 3.57);
with the Iron waived -3.45* / -7.25* / -5.31 at turns 50 / 75 / 100.

**The coordinator's sim bar, as a reading:** a city in 1 game in 6 on the
live-like walls recipe: 0 of 32 (gene-start), 0 of 32 with the Iron waived.
Cities founded by turn 100 against cities taken: -0.13 ± 0.30 founded and 0
taken (walls-0, gene-start); -0.34 ± 0.27 and 0 (lane). The bar is not met.

## What to look for in a live carrier's why.log

Every line of the gene has a detail that begins
`edge-unit-takes-the-soft-city: <stage>:`; `grep 'edge-unit-takes-the-soft-city:'
why.log` gives the whole chain in order. Stage by stage, for a Rome seat with
both flags on:

1. **Unit** (turn 1): `unit: legion (40), 3 technologies away`, then
   `start: research goes to legion from turn 1 (3 technologies away)`.
   Absent `start:` means the second flag is off or the unit is not unique.
2. **Research**: `research: bronze working toward legion`, and the research
   pick's own line `edge-unit-takes-the-soft-city: the cheapest (or "most
   valuable") step toward bronze working, which the edge unit outclassing a
   nearby rival city needs` (Mining first). Check the host's research
   matches the turn.
3. **The decision at Bronze Working**: either `start: a iron source is in
   reach; iron working is the goal, ...`, followed by `research: iron
   working toward legion`, or (version 2) `start: recheck: no iron source in
   reach at bronze working; checking every turn until turn T`, then within
   ten turns `start: a iron source is in reach (K turn(s) into the recheck)`
   or `start: iron is supplied K turn(s) into the recheck`, or at turn T
   `start: no iron source in reach at bronze working or in the 10 turns
   after; research returns to normal` (two cheap technologies spent).
4. **Resource**: `resource: builder N walks to the iron at (x, y)`, then
   `... improves the iron at (x, y)`; or `resource: bought a iron source for
   G Gold, H left`. Check the host built the Mine (Iron income above zero)
   and that the stock reaches 10 before the first Legion is wanted.
5. **Bodies and Gold**: headlines `<city> trains Warrior for the edge force`
   with `force: ... before this one`; while they wait, no other Gold buy
   should take the treasury below the Warriors' upgrade bill (the purchase
   floor). Then `force: upgraded a warrior to legion for G Gold of H`, and
   `<city> trains Legion for the edge force`, in up to three cities at once
   (version 2), some `..., ahead of <item> (it keeps its progress)`. A
   Granary or Builder claim should not take a queued Legion back.
6. **The target**: `opened: legion (40) against <city> of <civ> at defence D,
   edge +E, T tiles from our nearest city, walls ...`. While none is soft,
   `waiting: no soft city in reach`; by turn 45 with none,
   `closed: no soft city within 10 tiles is known by turn 45; the opening
   stands down`. Check the host's city banner defence against D.
7. **Denouncement**: `denounced: <civ> so the Formal War is legal in N turns`,
   once the first Legion is owned (version 2). The host should show the
   denouncement.
8. **Muster**: while walking, `hold: B of N legion at <city> with 70+ health;
   the others at ... tiles ...`; then `muster: 3 legion at 70+ health within
   4 tiles of <city>` (6 when the rival's border has closed the 3-4 ring).
   By turn 60 without a declaration: `closed: the force is not at the city
   by turn 60; the opening stands down and the legion stay as defence`.
   Version 2: a Legion under 70 health first `heal: legion N at H health
   heals in place (or in <city>) before it marches`; it should not move
   until healed. Past the naming turn, an unwalled target below +8 gives
   `kept: <city> unwalled at defence D, edge E: the forecast with the N
   legion owned beside it reads X% ...` (or `..., with N of 3 legion owned
   (the floor is -4 ...)`); it ends with `closed: past the naming turn the
   named city is no longer the target: <why>`.
9. **Gates at the muster**: `hold: ... the forecast reads X% within 4 turns
   (raw Y%), short of Z`; `hold: ... the shipped opening offers no
   declaration on <civ> yet (a denouncement waits out its Formal War)`;
   `hold: at war with <civ>`; `hold: N at <city>, walls stand and no breaker
   is with them`.
10. **Declaration**: `declared: 3 legion at 70+ health within R tiles, P in
    the pool; forecast X% within 4 turns (raw Y%), edge +E`. The host must
    show the war that turn. In the simulator a declaration sometimes missed
    the board (`hold: the declaration of turn T on <civ> is not on the board;
    asking again (k of 3)`); on the live bridge that line should not appear,
    and if it does, the host refused the order.
11. **Assault**: `assault: N blow(s) on <city>, health A to B`, and one
    `siege: <city> health H walls W; A legion beside it, N within 4, war
    turn T` a turn. A siege line with no unit beside the city and full
    health, turn after turn, is the round-3 failure (the shipped war
    machinery not pressing the attack).
12. **After**: `captured: <city> from <civ>, N taken, M legion left`, then
    `continue: <next city> ...` or `peace: <reason>` and `closed: <reason>`;
    or `peace: 20 war turns without a capture at <city>` / `peace: 1 legion
    left`.

## Verdict

The gene and its second flag do what they were built to do in the
simulator, end to end: the from-the-start opening takes Bronze Working at
turn 20 and Iron Working at 39-64, raises three Legions by turn 44 when Iron
is not the limit, musters them, denounces, declares on a capture forecast,
and fights the assault. They take no city there, for a reason the journal
names in the assaults: the city walls up within two or three turns of the
declaration, before the Legions are beside it, and Legions without a Ram
barely dent a walled city. With real Iron the opening mostly stands down at
Bronze Working (24 of 32: Iron is rarely within three tiles of one or two
cities at turn 20). The cost is real and measured: about 16% of Science at
turn 50 and 0.4 cities founded by turn 50, recovering to no clear difference
by turn 100.

The simulator cannot price this gene: its chairs wall their cities early
and wall up behind a declaration, which is exactly what the gene's window
depends on not happening, and the live seat sees 13-18% of the nearest
rival cities walled at turns 40-50. Per the coordinator's rule, the chain
executes, so this is the full PR, opt-in and unarmed, for a three-start
live trial by another session. The next change to try, if the trial shows
the same walls-behind-the-declaration pattern on the host, is to bring one
Ram with the force whatever the walls read at the muster.

## What is not verified

- **No live game.** Nothing here has run against Civilization VI. Whether
  the host walls a city up behind a declaration as fast as the sim chairs,
  whether the host executes the turn-start walk and the gene's attacks, and
  whether the purchase floor holds against the live purchase paths are all
  open.
- **The declaration that missed the board** (round 3) is a simulator
  artifact of planning on a decision view; the live bridge sends orders to
  the host. The redeclaration fallback should never fire live.
- **The Gold hold's own effect** was not isolated (it rides the
  from-the-start arm).
- **Rounds 3 and 4 are 110 turns and 32 pairs a recipe**; the 48-pair,
  250-turn confirmation (round 2) was cancelled at the coordinator's request,
  and no confirmation was run of the from-the-start variant, which took no
  city.
- **The walls throttle**: `--rival-walls 0` still leaves 50-81% of the
  nearest rival cities walled at turns 40-70; how the chairs get those walls
  was not traced.
- **The stack arm** (the shipped early-conquest opening) ran in rounds 0-1
  only; on a Science seat its first-capital genes are inert by their lane
  gate.
- **cargo-test** on the final head: see the PR's checks.
