# Defence tactics: how the live seat dies at Deity, and what answers it

Owner: the `defense-tactics` task (2026-11-27). Scope: early defence and our
elimination. Planned conquest is `docs/OFFENSIVE_CAMPAIGN.md`; the rival
Science race is another task's.

## The operator's heuristics (verbatim)

```
Defense Tactics Strategy Heuristics
Barbs
- Scout can heal in place and usually tank a barb warrior.
- Ideally hide behind a river, hills, woods/jungle, or some stacked combination of defenses.
- Warrior can take down spearman with time. Helps having +5 combat.
- Be careful about units spawning and being trapped in and killed.
- Really gotta play the long game and keep units alive with barbs.
- Good to know: Spearman almost always chase slingers
Barb Horde
- Capital cannot be captured.
- 2nd, 3rd cities will get razed though.
Invaded by Neighbors
- AI should really monitor neighbor mil size and watch for power spikes.
- Watch for enemy unique unit peak.
- Walls are best defense.
- Archer in city is solid.
- Encampments are solid at elite levels.
- Chokepoints, like in mountains.
Understand attrition odds
- Civ VI - play to eliminate units
- Advance Wars - damage dealt / taken
```

## 1. How we die (live evidence)

**Sample.** The 28 Deity rows of the ledger's `results.jsonl` with
`outcome == "defeat"` since 2026-11-22 (1 CIVVIS + 5 Firaxis AI, 6P Small
Continents, Online; 245 finished Deity games in the window, so 11%), and
their `runs/<tag>/events.jsonl.gz` and `summary.json` from the `ledger`
branch. Every one of the 28 has a full per-turn log: a `state` frame per
frame (our cities, buildings, units with moves and attacks left, every met
rival's military power, war state, opinion and the units we see), `combat`,
`order_verified` (production, purchases) and the bridge's refusals. As a
control, 19 Deity games of the same days that were invaded (a city lost or a
recorded invasion) and **survived**. The scripts are in the probe archive
(`docs/eval/2026-11-27-stacked-shooter-steps-out.native-queries.tar.xz`,
`census/`); they read the logs directly, because `tools/civ6_events.py`
reads `~/civvis-civ6-runs/control`, which only the live host has.

**Who kills us.** In the 21 logs that record the original capital's fall
(seven end first), a neighbouring major took it every time. Of the 65 cities
we lost in all 28, majors took 55, city-states and the Free Cities 9
(loyalty flips after the capital went), barbarians 1 (Tawdenni, t105, to
Line Infantry and Field Cannons). The eliminations come from one lane:
religious-target games lost 21 of 119 Deity games to elimination (17.6%),
science-target games 7 of 131 (5.3%). Mali alone lost 11 of 30.

**The war that takes the capital.** Declared on us at a median turn 68, by a
major fielding a median **3.0x** our military power (3.4x ten turns
earlier: the gap is chronic, not a spike). The capital falls a median **7
turns** after the declaration. In 7 of the 21 the same major had already
fought us once and made peace. Across all 59 wars declared on us the median
ratio was 4.0x (24 at 4-8x, 21 at 2-4x, 7 at 1-2x, 6 at 8x or more, 1 below
parity). Our own war orders: 34, in 24 of the 244 Deity summaries of the
window.

**Warning.** Two or more of the eventual taker's soldiers stood within eight
tiles of our capital a median **20 turns** before the declaration (20 of
21). Through those 20 turns the capital's queue head was a military unit on
25% of turns, a Settler 23%, a religious item (Holy Site, Prayers, Shrine,
Temple) 21%, Walls 9%. Once at war it was a military unit 58% of the time.

**What we had.** Walls stood at the capital on the declaration in 17 of 21
and at the fall in 18 of 21. A shooter stood in or beside it at the
declaration in 14 of 21; at the fall one stood on the centre in 18. None of
the 65 lost cities had an Encampment. Our military power was 0.19-0.34 of
the strongest rival's at every mark from t20 to t80. Gold on turns at war with a major at 2x or worse: a median 60 banked; an
Archer was not for sale at that price (Mali 050225Z t58: 68 Gold, the menu
sold a Scout), so Gold bought 28 soldiers on those turns across the 28 games,
while Faith bought 65 Missionaries or Apostles and 2 soldiers. Rival unique units were in sight before 24 of the 59
declarations; the early wars came with Warriors, Chariots and Catapults.
The rivals' opinion of us on the turn before they declared was "Unknown
Reason" (hidden agendas) in 109 of 145 negative modifiers; religion appears
in 3.

**What the eliminated did with what they had.** This is the defect this
change answers. A land shooter of ours with an enemy soldier within its
range at the start of our turn fired on 49% of those turns (800 of 1,641).
Split by where it stood:

| where the shooter stood | shooter-turns with a target | fired |
|---|---:|---:|
| alone on a City Center of ours | 384 | 322 (84%) |
| on a City Center **with another soldier of ours** | 408 | 68 (17%) |
| in the field | 849 | 410 (48%) |

Civilization VI puts a unit trained or bought in an occupied City Center on
the centre, and offers a shooter that shares a City Center with another of
its owner's combat units no target (`Game::shooter_stacked_in_city`; 657 of
746 such shots refused across 70 logs on 2026-11-10). 531 of our turns in
the 28 games began with two or more of our soldiers on one centre, 468 of
them with an enemy within four tiles, and all 28 games had them. In the 19
survivors the stacked share of shooter-turns with a target was 9% (160 of
1,742), against 25% in the eliminated games; the stacked shooters fired 15%
there too. The mechanism is the same in both groups; the eliminated games
had nearly three times as much of it, at the walls that fell.

Lisbon, `civvis-t7-20261123T091247Z`: walls, five Archers at full health in
and beside the City Center from t54 to t59, three on the centre itself.
Byzantium (1.4x our power) brought a Catapult beside the city at t57; the
walls went 100, 26, 0 on t58-59 and the city fell at t60 to a Spearman. Our
Archers fired three times in six turns, every shot from the one Archer alone
on its tile. The three on the centre received no order but the bridge's
default hold, and the host refused every Fortify, Alert and Skip it sent
them. `shooters-man-the-capital` (armed) calls a shooter on the capital's
centre, finds no legal shot for it, and holds it there for the turn.

**Attrition (summary.combat).** Kills per loss: eliminated 1.25, invaded
survivors that lost a city 1.62, games that lost no city 2.9. Damage dealt
per damage taken: 0.80, 0.93, 1.15.

**Economy (summary marks).** At t60 the eliminated held a median 2 cities;
survivors 4-5. At t50 they made 14 Production to the survivors' 27-31 (the summary's `rival_production`: ~105) and had 9 population to 14-16. The second city came at
t18 in both groups: the eliminated stall after it.

## 2. Each heuristic against the code

"Live" is read from what the seat played: `genome_treatments.treatments` in
the summary of `civvis-t8-20261127T042704Z` (decider 760a9ab1), not from
`deploy/live-force-on.txt` (see section 4: the host does not play that list).

| heuristic | what the AI does today | live | gap | evidence |
|---|---|---|---|---|
| Scout heals in place, tanks a barb warrior | Recon follows the explore and contact sweeps (`ai/unit_turn.rs`, `advanced/scout_distance.rs`); healing is the generic wounded rotation (`advanced/battle_planner.rs`, `docs/LIVE_WOUNDED_MOVEMENT.md`); `opening-scout-comes-back` brings the opening Scout home | `opening-scout-comes-back`, `scout-first-opening` live; `barbarian-scouts-are-scouts` (Repair) withheld by the ledger | No rule says a wounded Scout fortifies to heal beside a barbarian rather than walking on | Not measured here: barbarians took 1 of 65 cities; Scouts are not in the death path at Deity |
| Hide behind river, hills, woods/jungle | `safest-stand` (stand on the tile the danger field prices lowest), `strike-reach`, `risk-is-priced` (`advanced/priced_risk.rs`), `defend-where-you-stand`; the engine prices `tile_defense_bonus` (hills, features, forts) | all live | Terrain enters only through the danger field; a river between a defender and its attacker is not scored anywhere I found. **This change**: the stepped-out soldier takes the tile with the best cover (+3 hills, +3 woods/rainforest) and the fewest enemy melee beside it | Lisbon's Archers never left the centre to use any tile |
| Warrior grinds a Spearman, +5 vs barbarians | Camp clearing: `camp-party` (Repair), `emptied-camp-is-taken`, `camp-tile-buyout`, `war-kills-the-bands-2` | live | ClearCamp forces hover beside fortified camps (memory: 2026-09-26 proxy census) | Barbarians are not the Deity killer in these 28 |
| Units spawning and trapped | `builder-barbarian-safety`, `builders-work-through-raiders`, `civilian-out-of-reach` (`docs/CIVILIAN_SAFETY.md`) cover civilians | the first two live; `civilian-out-of-reach` off | Military units are not checked against camp spawn rings | Not seen in the 28 |
| Keep units alive with barbs | `ranged-hp-reserve`, `doomed-blow-veto`, `falling-city-outranks-the-heal`; `wounded-out-of-reach` and `swap-rotation` off | partly | No barbarian-phase preservation rule of its own | — |
| Spearman chases Slinger | `docs/HOST_ENEMY_MODEL.md` fits the host's strike choice (it takes what is in front of it, above all a unit it can hit without moving) | model only (`--rival-host-tactics` bench) | No kiting or bait logic | — |
| Barb horde: capital safe, 2nd/3rd razed | No barbarian-horde reading | — | — | 1 of 65 cities lost to barbarians; the 9 city-state/Free City losses are loyalty flips after our capital fell |
| Monitor neighbour military, power spikes | The capital watch (`capital-walls-before-the-surprise`: a met major's soldier within 8 tiles of the original capital in the last 10 turns, at 1.0x at Deity with `capital-walls-at-any-ratio`); `near-rival-deterrence` (live t30-80 only, trains toward parity, at most 2 queued, spares Holy Sites/Shrines/Temples and every project); `shooters-man-the-capital` | watch and deterrence live; `near-rival-deterrence-holds-late`, `-opens-early`, `surprise-watch-holds-its-ratio` off | The watch reads 20 turns ahead but after the walls stand it moves only production claims that a Settler, a Holy Site, Prayers or a Shrine outrank: 25% military in those 20 turns. The ratio is 3x for 20+ turns before the war: there is no spike to wait for | §1 *Warning* |
| Enemy unique-unit peak | `unique-unit-preference` (ours), `decisive-window` (ours) | live | Nothing reads a rival's unique unit as a threat | 24 of 59 declarations had one in sight; early wars came without |
| Walls are best defence | `capital-walls-before-the-surprise`, `capital-walls-at-any-ratio`, `frontier-walls-before-the-surprise`, `defenders-hold-the-walls`, `opening-yields-to-walls` | live | The walls arrive: 17 of 21 capitals were walled at the declaration and fell anyway, 7 turns later | §1 *What we had* |
| Archer in city is solid | `shooters-man-the-capital` (shooters home, a gun first), `every-shooter-at-the-walls`, `siege-at-the-walls-dies-first`, `city-strike-takes-the-gun`, `threatened-city-buys-its-defender-2`; `capital-archers-before-the-surprise(-2)`, `threatened-capital-trains-its-shooters` off | mostly live | **The archers are there and do not shoot**: a shooter sharing a centre is refused every target, and 408 of the 792 centre shooter-turns with a target were shared. Nothing moved the second one out; `shooters-man-the-capital` holds it there. **This change** | §1 table; Lisbon |
| Encampments at elite levels | `encampment-seals-the-pass`, `wall-sortie-skips-the-encampment` | live | No Encampment stood at any of the 65 lost cities | §1 |
| Chokepoints (mountains) | `advanced/chokepoints.rs`: `chokepoint-claim`, `pass-picket` live; `chokepoint-siting`, `chokepoint-garrison`, `pass-picket-2` off | partly | Not measured here | — |
| Play to eliminate units | `battle-planner-2`, `fire-plan` (off), the immediate-kill pass, `doomed-blow-veto` | `battle-planner-2` live | Half the shots on offer are not taken (49%); the stacked ones almost never (17%) | §1 table |
| Damage dealt / taken | `risk-is-priced`, `strike-reach`, `safest-stand` | live | Ratio 0.80 in the eliminated, 1.15 where no city fell | §1 *Attrition* |

## 3. The change: `stacked-shooter-steps-out`

`src/ai/advanced/stacked_shooter_steps_out.rs`, opt-in. At the start of our
unit turn, before `shooters-man-the-capital` and the battle plan, each City
Center of ours where a shooter shares the tile with another of our soldiers,
while an enemy land soldier (barbarians included, recon not) stands in sight
within four tiles, keeps one soldier on the centre (a civilian's guard if one
is there, else the best shooter) and steps every other one onto the best
free tile beside it: a tile with a shot this turn first, then the fewest
enemy melee beside it, then cover, then our own ground. The battle plan and
`shooters-man-the-capital` then see shooters that can fire. Off, nothing
runs.

**What it moves on the ledger.** The direct reading is the share of
shooter-turns with a target that fire (`events.jsonl`: 49% in the eliminated
games; a stacked shooter is 17% of the time a shooter at all): it should
rise toward the 84% a lone garrison reads, within ten games. Then
`summary.combat` kills per loss (1.25 in the eliminated) and damage
dealt/taken (0.80), which converge in tens of games. The outcome is the Deity
elimination rate, 11.4% (28 of 245) since 11-22, median elimination turn
100, and the turn the original capital falls (median 80 in the 21 logs). A
cut from 11% to 8% needs about 1,500 games an arm to read at 80% power and
5% two-sided; from 11% to 7%, about 800. At the ladder's ~50 Deity games a
day that is two to four weeks of a live screen; the before/after on
`results.jsonl` will be readable sooner but confounded by everything else
that ships.

**How it gets armed.** The tag is added to `deploy/live-force-on.txt` in this
PR, which is how the repository arms a gene. The live host does not play that
file today (section 4), so the operator also has to put the tag in the host's
override (`~/.civvis-live-force-on`, one comma-separated line) or make the
override match the repository list. A live screen
(`~/.civvis-live-screen-gene`, `docs/LIVE_SCREEN.md`) is the clean A/B; the
slot was held by `religion-seat-takes-the-faith-beliefs` on 11-27.

**What was verified.** Unit tests (`stacked_shooter_steps_out/tests.rs`)
including a whole turn of the live-like seat (battle planner and
`shooters-man-the-capital` on) in which two stacked Archers fire none of
their shots off and both on; and paired replays of captured live boards
from the eliminated games through `civvis_orders --resumed`
(`docs/gene_screens/fires/stacked-shooter-steps-out.action-pairs.json`).
Not verified: host execution, and any outcome.

## 4. Where live arming happens

`codemap --gene` reads `deploy/live-force-on.txt` (283 tags). The summary of
the last game on 11-27 records the `--with` list the supervisor passed
(`forced`, 621 tags) and what the decider seated (`genome_treatments.forced`,
475; `treatments`, 770). The 621 hold 281 of the repository's 283 and 340
the file does not; the two the file holds and the host did not pass are
`bankruptcy-exits-the-drain` and `siege-tier-yields-to-the-bombers`. The
supervisor reads `CIVVIS_WITH_FILE`, else a non-empty
`~/.civvis-live-force-on`, else the repository file
(`tools/ops/civvis-game-supervisor.sh`), so the host is on a local override.
`codemap --gene`'s "armed live" therefore says "no" for genes the seat plays
(`threatened-city-buys-its-defender-2`, `watched-capital-keeps-its-defender-gold`,
`shooters-man-the-capital`, `capital-walls-at-any-ratio`, `befriend-the-strongest`).

## 5. Next, ranked by the evidence (not in this PR)

1. **The watch drives the army after the walls stand.** 20 turns of warning,
   25% of them spent on a military unit. A version of the capital watch that,
   once the walls stand, keeps two range-two shooters within one tile of the
   capital (not on it) and lets Prayers, a Shrine, a Temple and a Settler wait
   while a watched major is at 2x or more. `capital-archers-before-the-surprise`
   does this before the walls and is off; the 2026-11-15 Immortal screens
   measured its cost (Science t100 -6.5, cities -0.6) at a lower bar.
2. **Religious-lane opening.** The eliminated stall at two cities; religious
   games are eliminated at 3.3 times the science rate. #5885 (religion seat
   third city first) works this.
3. **Peace with a 3x neighbour is a ten-turn rebuild window.** 7 of 21 capital
   wars were a second war by the same major; the ten turns between were spent
   on Settlers, Shrines and Builders (Mali 050225Z t45-57).
4. **Shots in the field.** 48% of field shooters with a target fired; the
   reasons (line of sight, a retreat to heal, the doomed-blow veto) are not
   separated here.
