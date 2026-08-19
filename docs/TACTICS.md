# Unit battling: the state of the art, what CIVVIS does, and what changed

2026-07-31. Historical record of the tactical-search experiment; companion to
`src/skirmish.rs` and `src/bin/battle_bench.rs`. §§1–8 are the original
record, including the removal; §§9–10 record the operator-directed
restoration, subsequent search improvements, and where they now run. §11
records the arena deadline contract, §12 the capture-the-flag
objective, §13 the scenarios, and §15 the stock arena's move to two
standing armies with no reinforcements (§14 is the lobby's setup order).

## 1. What the published state of the art is for this problem

A Civ turn is a **multi-action turn**: one player moves every unit before the
opponent moves any. If a side has `n` units with `b` choices each, the branching
factor for a single turn is `b^n`, not `b`. At six units and six options that is
already 46,656 turns to consider, and it grows exponentially in army size. This
is why ordinary game-tree search does badly here: alpha-beta and vanilla MCTS
spend their whole budget failing to get past the root.

The literature that owns this exact shape is small and consistent:

- **Portfolio Greedy Search** — Churchill & Buro, *Portfolio Greedy Search and
  Simulation for Large-Scale Combat in StarCraft*, CIG 2013. Do not search over
  actions; search over **assignments of scripts to units**, and hill-climb the
  joint assignment one unit at a time, re-evaluating with a playout. Scales to
  50-vs-50 fights where alpha-beta cannot start.
- **Online Evolutionary Planning** — Justesen, Mahlmann & Togelius, *Online
  Evolution for Multi-Action Adversarial Games*, EvoApplications 2016. Treat the
  whole turn as a **genome** — one action per unit — and evolve it against a
  state evaluation function. Evaluated on *Hero Academy*, a turn-based
  multi-action tactics game structurally identical to a Civ turn, this beat both
  MCTS and greedy search **by a large margin**. This is the closest published
  analogue to our problem that exists.
- **Portfolio Online Evolution** — Wang et al., AIIDE 2016. The combination:
  evolve over the space of script assignments rather than raw actions.
- Adjacent: **NaiveMCTS / combinatorial multi-armed bandits** (Ontañón) for
  multi-action move generation, and **Puppet Search** (Barriga, Stanescu & Buro)
  for searching over a handful of scripted choices instead of raw actions.

For comparison, the strongest *shipped* commercial approach in this genre —
Civilization V's `CvTacticalAI` — is not a search at all. It partitions the map
into **dominance zones**, assigns each zone a **posture**, and then runs an
ordered list of scripted **tactical moves** with prioritised target lists. It is
a coordination layer over heuristics, and its strength comes from deciding
things *per zone* rather than *per unit*.

The through-line of all of it: **the win is in deciding the units together, not
in evaluating any one of them better.**

## 2. What CIVVIS already had

Worth stating plainly, because it is better than a reading of `src/ai.rs` would
suggest, and because the real agent is in `src/ai/advanced.rs`:

For each candidate attack, `AdvancedAi` clones the game, applies the action
through the **real engine**, and reads the exact result
(`tactical_attack_value_owned`). It then extends the line with
`forcing_reply_penalty_owned`, a bounded **quiescence-style reply search** that
refreshes the enemy's forcing actions and prices a two-action focus-fire answer,
including one-step approach moves. Candidate evaluation is parallelised across a
persistent `WorkPool`, with only owned clones crossing the thread boundary.

As a **single-unit** evaluator that is genuinely close to the state of the art.
It is not a weak heuristic and it should not be described as one.

The weakness is one level up. The turn is assembled by
`advanced_units` walking the units in a **fixed class order** — ranged, then
siege, then melee — and letting each one commit **greedily and irreversibly**
before the next is considered. That commitment rule costs four things:

1. **The reply term is biased by position in the order.** Each unit prices the
   enemy's answer against a board where its own teammates have not moved yet.
   Ranged units act first and over-price their exposure — the screen that will
   stand in front of them does not exist yet — and melee acts last and
   under-prices it. The bias runs in exactly the wrong direction.
2. **No broad joint target assignment.** With four attackers and three defenders,
   every attacker picking its individually-best target is not the best *set* of
   attacks. The `focus_fire` gene is a flat bonus toward one shared tile, which
   trades spread-fire for overkill rather than solving either. Production now
   repairs the narrow, forcing case: a bounded friendly-volley extension sees a
   direct two-unit kill and prices the enemy reply after the second friendly
   action. It is intentionally not a general assignment search.
3. **The order is never questioned.** Soften-then-capture is usually right,
   which is why a fixed order works as well as it does, but it is wrong whenever
   a melee kill has to clear a tile or a firing lane first.
4. **Movement is blind to the attacks it enables.** A tile is scored on
   depth-to-target, adjacent support and incoming threat; nothing scores it for
   the shot it opens. This is the largest of the four by a wide margin — see §4.

## 3. What was built, then removed

The removed experiment was a bounded **Portfolio Online Evolution** over the
engagement. It was evaluated as an optional entrant and never became part of
the production controller.

- **Portfolio.** Each engaged unit gets a short list of candidate *lines* — an
  attack from where it stands, a step onto an adjacent tile followed by the
  attack that step opens, or the empty line that declines. Generated
  geometrically, so candidate generation costs no clones, and pruned to the best
  few by a closed-form damage prior.
- **Genome.** A line per unit plus a permutation giving the play order.
- **Fitness.** Clone once, play the whole turn, score the resulting position:
  material swing across the whole board, minus the **marginal** change in the
  enemy's best answer, minus the same `attack_threshold` toll the shipped agent
  charges itself, minus `FORTIFICATION_FORFEIT` for each unit the plan moved.
  Pricing the reply once against the finished turn is the direct repair for
  defect 1; the forfeit term is what makes defect 4 reachable.
- **Seeding.** Sequential greedy — the shipped construction — restarted from
  several orders, which is Portfolio Greedy Search's own remedy for
  order-dependence.

Three design rules earned by measurement, all recorded because getting them
wrong cost real iterations:

**Charge the shipped attack toll.** A first version dropped
`attack_threshold`, which made the treatment "plan jointly *and* be far more
aggressive". That conflates two changes, and the aggression alone is
composition-dependent. The search should change *which* attacks are made, not
how cheaply the agent is willing to be hit.

**Seed the real incumbent.** A first version seeded a *static* assignment —
every unit choosing against the untouched board — and called it greedy. It is
not: the shipped rule is sequential and adaptive, each unit choosing knowing
what its teammates already did. Over 400 melee scenarios the static seed spread
damage where the sequential rule concentrates it, and **lost 700 kills on
identical total damage** (2306 vs 3009 kills at 456,200 vs 456,220 damage). A
search that starts behind the incumbent cannot climb back on a small budget.

## 4. Approach moves, and the one term that made them work

Letting a unit **step onto a tile and then take the attack that step opens** is
where most of the value in this whole layer turned out to be. It is also
*actively harmful* until the evaluation can price what the step gave up.
Measured on `battle_bench`, paired material swing per scenario against the stock
agent:

| portfolio | melee-only | combined arms |
|---|---|---|
| attacks only, no stepping | +5.6 | +28.2 |
| stepping, no forfeit term | **−228.9** | +178.8 |
| stepping, only if the step does not thin the line | −44.7 | +119.0 |
| **stepping + fortification forfeit (shipped)** | **+16.5** | **+276.4** |

**The obvious diagnosis for that −228.9 is wrong, and it cost two attempts to
find out.** The natural story is that the unit broke formation, so the whole
enemy front answered it. An explicit adjacent-friendly-support term was built
and swept over weights 0/10/20/25/50 against three compositions on disjoint
seeds — and **could not be distinguished from zero at any weight**, with
combined arms in fact *highest* with it switched off entirely. The formation
hypothesis is refuted, not merely unhelpful; it is not worth re-attempting.

What the evaluation was actually missing is much simpler: **a unit that holds
its ground can dig in next turn, and one that stepped forward cannot.**
Fortification is a *future* action, so it is invisible to every term that prices
the position as it stands — material, damage, reply. Charging a flat
`FORTIFICATION_FORFEIT = 40` per unit that moved is one subtraction, and it
moves combined arms from +28.2 to +241.3 while taking melee-only from −228.9 to
neutral.

The weight is a plateau, not a knife edge. Swept out-of-sample at 700 seeds a
cell:

| forfeit weight | melee-only | combined arms |
|---|---|---|
| 20 | −19.5 | +228.1 |
| 30 | +0.5 | +219.4 |
| **40 (shipped)** | **+14.2** | **+223.3** |
| 50 | +15.4 | +195.7 |

Combined arms is flat across the whole range; melee crosses zero near 30. 40
sits comfortably inside both.

## 5. The instrument

Whole-game win rate is the right acceptance test and the wrong measuring device
here. `docs/ORACLE.md` puts a 100-cell paired run's resolving power at roughly a
70/30 split, and a six-player game produces on the order of fourteen unit kills.
A change that fights meaningfully better can pass through it without a trace.

`src/skirmish.rs` + `src/bin/battle_bench.rs` measure combat directly: two
identical armies, the same map, **the seats swapped**, and a count of what each
agent destroyed and lost. The swap makes terrain and seat advantage cancel
exactly within the pair.

**The control is the load-bearing part.** `--a advanced --b advanced` reports a
paired mean of **exactly** 0.00 with 0 of 60 seeds diverging. Nothing this
harness reports is its own noise. The report also carries a **fires-check** —
seeds on which the two agents' play diverged at all — because a null from a
treatment that never fired says nothing about the game.

The instrument also reports **unit lifetime return** by kind. Every military
unit carries the enemy-unit HP it actually removed, including melee
counter-damage and the final attack of a unit that dies in that exchange.
Killing blows are capped at the victim's remaining HP, so overkill is not
output. When a unit leaves play its damage and represented Production are
folded into its civilization's saved lifetime ledger; units still standing at
the scenario boundary are composed into the same table. `battle_bench` prints:

- observed lives;
- mean Production represented per unit;
- mean damage over one observed life; and
- damage per Production invested.

This answers a different question from material swing. Material swing says
which controller traded better; lifetime return says *which pieces paid for
themselves while it did so*. Only damage to enemy units is attributed to a
unit. City and Encampment strikes remain in the side-level damage ledger but
not in a unit row, because no unit produced them. The Production figure is the
investment that entered play: directly trained Corps/Armies carry their
1.5x/2x cost, combined formations add their constituents, and an upgraded unit
keeps the investment that created it.

## 6. Results

### Combat, where the change acts

`battle_bench`, 24 turns, 28×20, armies of six, each seed played twice with
seats swapped. The removed experiment was compared against `advanced`, **2000
seeds a cell, on seed blocks fresh to the final configuration** — disjoint from
every block used to choose the forfeit weight or the search budget.

| composition | exchange ratio (ours vs theirs) | paired material swing | sign p |
|---|---|---|---|
| combined arms | **1.760** vs 0.568 | **+276.4 ± 7.2** | <0.0001 |
| ranged heavy | **2.090** vs 0.479 | **+271.4 ± 7.4** | <0.0001 |
| with siege | **1.261** vs 0.793 | **+116.3 ± 7.6** | <0.0001 |
| melee only | 1.016 vs 0.984 | +16.5 ± 4.3 | 0.1247 (see below) |
| control (`advanced` vs `advanced`) | 1.000 vs 1.000 | 0.00 ± 0.00 | 1.0000 |

Taken **on this branch's merged tip**, not on the commit the design was tuned
against — engine drift from the four PRs merged underneath it (#665 perf, #666,
#667, #668) moved combined arms from +299.2 to +276.4 and melee from +21.4 to
+16.5. That is the repository's standing rule about re-taking a baseline on your
own commit, and it is why the tuning-era figures are not the ones quoted here.

The melee cell is the one to read carefully: the paired t is decisive
(t = 3.840, p = 0.0001) while the **sign test is not** (p = 0.1247, on an
847/784 split). Those disagree because the melee effect is carried by size on a
minority of scenarios rather than by a consistent direction — 369 of 2000 seeds
are exact ties. The honest reading is **positive on average, not reliably
positive scenario by scenario**, and the claim in this document is only that
melee is no longer *harmed*, which every version before the forfeit term was.

**Decisive on the three compositions that contain ranged units** — which is
every army the production code actually builds — and no longer harmful on the
melee-only stress case that every intermediate version lost outright. The size
tracks how much the composition rewards positioning: an all-ranged line more
than doubles its exchange ratio, a melee scrum barely moves.

For scale against the first version of this work, which planned only attacks
already available and did not step: combined arms **+28.2 → +276.4**.

⚠ **The sign test in this harness had an overflow bug** that reported
`p = 1.0000` on a 1122-to-317 split. `2^n` is `inf` past n≈1023, the binomial
coefficients overflow with it, `inf / inf` is NaN, and Rust's `NaN.min(1.0)`
returns **1.0** — a perfectly confident null on overwhelming evidence. Large n
now uses a normal approximation. Every number above was recomputed after the fix.

### Search budget is binding

Unlike the macro search, where doubling the budget is the one reproducible win
and quadrupling does nothing, combat quality here keeps climbing across the
whole range measured (700 scenarios a cell):

| pop / gen / lines | combined arms | melee | seated cost |
|---|---|---|---|
| 12 / 6 / 6 | +236.9 | +6.2 | 1.13× |
| **20 / 10 / 10 (shipped)** | **+279.1** | **+17.6** | **1.29×** |
| 32 / 20 / 16 | +300.0 | +17.6 | 2.19× |

20/10/10 is the knee — most of the available gain at a cost still far under a
searching macro seat. Raising it further is a live option if the compute is
ever worth spending, which on the whole-game evidence below it currently is not.

### Cost

`battle_bench --cost`, 6 players, 74×46, 9 city-states, 250 turns, interleaved
so both fleets meet the same contention:

| fleet | ms a game-turn | ratio |
|---|---|---|
| all `advanced` | 29.75 | 1× |
| **one joint-tactics seat among five** | **43.65** | **1.47×** |

Measured in the configuration that would ship — one seat among five — for the
reason `docs/EVAL.md` records: measuring only an all-treated fleet once read 29×
for something that costs 6.4× seated. For scale, a `StrategicAi` seat is 6.4×;
this is 1.47×.

### Fires-check at deployment scale

Over 949 treated seat-turns in full six-player games, the search planned on
**243 turns (25.6%)**, reaching **885 unit decisions**. (The attack-only variant
managed 9.4% and 277; admitting approach moves nearly tripled the layer's
footprint, because a unit no longer has to already be in contact for the search
to have anything to decide.) It is emphatically not a no-op in deployment.

⚠ Getting this number required fixing a real instrumentation bug first: the
blanket `impl<T: Ai + ?Sized> Ai for Box<T>` in `src/ai.rs` forwards each trait
method explicitly, and a newly added one silently fell back to the default
`None`. The first reading was a confident **0 fires**, contradicted by the run
being 1.99× slower than its control. **A census that reports zero while the
clock says otherwise is a broken census, not a null.**

### Whole game — INCONCLUSIVE

`ai_eval`, deployment profile: 6 players, 74×46, 9 city-states, 250 turns,
mirrored maps with seats swapped.

| version | maps | paired score | Elo-equivalent | sign p | verdict |
|---|---|---|---|---|---|
| attacks only | 120 | 47.5% (38.8–56.4) | −17 (−79..+45) | 0.3915 | INCONCLUSIVE |
| attacks only | 300 | 51.3% (45.7–56.9) | +9 (−30..+49) | 0.4657 | INCONCLUSIVE |
| **+ approach moves** | **300** | **51.3% (45.7–56.9)** | **+9 (−30..+49)** | 0.4841 | **INCONCLUSIVE** |

**An 8.5× larger combat effect produced no change in win rate at all** — the
same 51.3% and the same +9 Elo. That is the single most informative number in
this document, and it is a much stronger version of the oracle's finding than a
single null: the tactical dial was turned an order of magnitude and the win rate
did not notice.

Only 100 of 300 maps broke at all. The confidence interval admits everything
from a small loss to a small gain; this does not show the change helps, and it
does not show it hurts.

The terminal-score diagnostic — explicitly *not* a promotion input — favours the
treatment in every run: 67/53, then 173/127 (p=0.0093), then 165/135 (p=0.0939).
Consistent direction, but wins are what the gate reads.

## 7. What this means, stated plainly

- **The change does what it claims, decisively.** It fights better on all four
  army compositions, significantly, on seed blocks fresh to the final
  configuration, at 1.47× the cost of a stock seat among five. Combined arms
  nearly doubles its exchange ratio, 1.760 against 0.568.
- **It does not convert into measured wins, and the failure is emphatic.** Two
  300-map runs at 6p/74×46 both land on 51.3% and Elo +9. The second had **8.5×
  the combat effect of the first** and moved the win rate not at all.
- **That is the expected outcome.** `docs/ORACLE.md` already bounded military
  capability as off this simulator's critical path: `taker`, `modernity` and
  `attrition` — free positioning, free unit quality, free healing — all null,
  while `treasury` went 62–0. What this adds is a *dose–response* check on that
  bound, which a single null cannot give: turning the tactical dial an order of
  magnitude produced no response at all.

The experiment was removed after the whole-game gate remained inconclusive.
Its null result remains recorded here so the same unproven search is not
reintroduced by accident.

**If someone picks this up**, the ranked list is:

1. **Do not spend more on tactical quality expecting wins.** This is now the
   strongest evidence on record for that: an 8.5× swing in measured combat
   quality, twice, at 300 maps each, with no movement in win rate. The battle
   benchmark can keep improving while the win rate does not, and the search
   budget table shows it *would* keep improving if compute were spent on it.
2. **The interesting question is why fighting better does not win.** Wars are
   rare (~1.4 a game) and take almost nothing (`civvis-wars-take-nothing`), so a
   layer that fights 80% better on the turns it fires may simply not fire on
   enough of the game to matter. Whether the constraint is *how well the army
   fights* or *how often and to what end it is used* is now the well-posed
   question, and the second half of it is untouched by this work.
3. **`battle_bench` is reusable for any tactical change**, at roughly three
   orders of magnitude more combat events per unit of compute than whole games.
   Run the control first, every time, and sweep at least a ranged-heavy and a
   melee-only composition — this work had a treatment that measured +178.8 on
   one and −228.9 on the other.
4. **The formation hypothesis is refuted, not open.** Do not re-attempt an
   adjacent-support term for approach moves; it was swept and is
   indistinguishable from zero. The term that mattered was forfeited
   fortification, and the general shape of that lesson — *the missing term was a
   future option the unit gave up, not a property of the position it reached* —
   is the one worth carrying to the next evaluator.

## 8. Production tactical role basics (2026-08-03)

The production `AdvancedAi` now assigns the ordinary unit classes to explicit
battlefield jobs. This is separate from the optional joint search described
above and applies to both its exact per-unit evaluator and its bounded portfolio
candidate pruning:

- melee prefers anti-cavalry, anti-cavalry prefers light or heavy cavalry, and
  both cavalry classes prefer melee when exchanges are otherwise close;
- ranged units value shots outside the defender's direct return-fire range, and
  movement prices the enemy's next move-and-attack envelope instead of only its
  current attack radius;
- siege prefers districts with standing walls;
- melee and anti-cavalry prefer a walled assault when an era-compatible
  battering ram or siege tower is adjacent, while those support units follow
  only a class that can actually use their aura and hold once they reach the
  wall;
- light cavalry pillages before routine combat; heavy cavalry attacks first and
  uses pillaging as its fallback.

When an engaged force has a direct two-unit kill, the first attack gets a
bounded friendly-volley extension: it confirms the second legal attack against
the cloned engine state, then replaces the reply price that incorrectly placed
the enemy between those friendly actions. The search considers at most three
immediate targets and eight deterministic finishers, excludes movement and
cities, and leaves the finisher's own exact exchange score in control. This
improves actual focus-fire sequencing without reviving the removed portfolio
search or adding quiet-move clone fan-out.

The bonuses assign close choices; exact damage, kills, captures, and enemy reply
damage still dominate decisive exchanges. The feature is enabled by the
production constructor and remains off in the frozen Basic and `advanced_v1`
controls so historical evaluator identities do not change silently.

## 9. Restoration and the v2 search (2026-08-07)

Operator directive: make tactical unit combat as strong as the instrument can
prove. The removed experiment, its census wiring, and the deleted instrument
(`src/skirmish.rs`, `src/bin/battle_bench.rs`, pruned in #1194/#1278) were
restored from history, re-verified (control at exactly 0.00 over 60 and again
over 200 fresh seeds), and then strengthened by measurement. Every change below
was screened on at least combined-arms and melee-only at 300 paired seeds, kept
only where the screen improved or held, and the final configuration was
confirmed on 1000 paired fresh seeds a cell (seed block disjoint from every
screen).

What changed in the search itself:

- **Siege portfolio honesty.** `do_ranged` refuses a siege piece that moved
  (absent the attack-after-move promotion), so every siege approach line was a
  turn-wasting suicide walk and a pruning-slot thief; a siege piece that
  already moved has no legal shot at all. Both are now filtered at generation.
  Siege cell +90.0 → +108.7.
- **Budget to the new knee.** 20/10/10 → 32/20/16 (pop/gen/lines): +246.0
  combined. A 48/28/20 probe measured flat (+273 vs +279), so the curve that
  "kept climbing" in §6 now plateaus at 32/20 — the evaluator improvements
  moved the knee, and the budget stops there.
- **Two-step approach lines** for units with *strictly more than two* movement
  — the blow itself needs a movement point, so for a two-move unit every
  two-step line arrives refused and stands in contact unfortified (measured:
  melee −32.6 before the gate, back to noise after it). Intermediate tiles
  adjacent to known hostile military are filtered (ZOC forfeits the second
  step). Combined +275.0; ranged and melee unchanged, exactly as the mobility
  gate predicts.
- **Mobility-true enemy reach in the closed-form reply.** An m-move melee unit
  strikes from m tiles, mounted ranged from range+m−1, and siege from its
  range alone (it cannot move and shoot). The old constants (2, range+1) were
  exact for two-move infantry and priced a four-move horseman at half its real
  envelope. Siege cell +108.7 → +205.8; combined +279.2.
- **Two nulls, recorded so they are not re-attempted:** stacking
  `tile_defense_bonus` into the closed-form reply and priors (the exact term
  `do_attack` uses) measured −10 combined / −10.6 melee with the melee sign
  test at p=0.09 in the wrong direction — the exact forward model already
  prices the terrain of every attack taken, and pre-discounting our tiles in
  the enemy's answer mostly licenses braver stands. And the deeper budget
  above the new knee buys nothing.

### v2 results — 1000 paired fresh seeds a cell, seats swapped, control 0.00

| composition | exchange ratio (v2 vs stock advanced) | paired swing | sign |
|---|---|---|---|
| combined arms | **1.592** vs 0.628 | **+264.3 ± 12.2** | 730/243, p<0.0001 |
| ranged heavy | **1.993** vs 0.502 | **+395.4 ± 13.3** | 804/164, p<0.0001 |
| with siege | **1.349** vs 0.742 | **+199.7 ± 15.0** | 645/315, p<0.0001 |
| melee only | 0.944 vs 1.060 | −3.4 ± 4.8 | 375/398, p=0.43 |

Against the greedy production controller the army now trades at 2.5× (combined
arms), 4.0× (ranged heavy) and 1.8× (siege) kills-per-loss ratio-of-ratios;
the melee scrum stays at noise, which §4 explains and §7.4's forfeit term
already priced. Relative to the restored v1 on the same screens: combined
+233 → +279, siege +90 → +206, ranged flat at +363, melee unchanged.

### Cost and fires at deployment scale

`battle_bench --cost`, one treated seat among five, 6p/74×46, interleaved:
**1.57×** a stock seat (v1 was 1.47× at the smaller budget; a `StrategicAi`
seat is 6.4×). Fires-check: the search planned on 47 of 303 treated
seat-turns (15.5%), reaching 145 unit decisions.

### Where it runs

- **The live bridge enables it** (`enable_joint_tactics`, tagged
  `joint-tactics`, ablatable as `live_without_joint_tactics`). The deployed
  agent is the one the operator asked to fight better, and the whole-game
  evidence in §6 says the rating this cannot move is not the thing being
  optimized there.
- **The Battlefield arena now routes promoted `AdvancedAi` controllers through
  the same search automatically.** `Game::is_arena()` is the seam: a bounded
  20×20 fight is the surface this search was measured for, while a native Civ
  world keeps its existing greedy commitment rule. The frozen `advanced_v1`
  anchor and an explicit `disable_joint_tactics` withholding remain greedy, so
  evaluator identities and live ablations stay honest.
- **The tournament `advanced` entrant keeps the greedy rule on world games**
  and the frozen Basic/`advanced_v1` identities are untouched, so recorded
  ladders stay comparable. The `advanced_joint_tactics` arm remains the
  explicit measured treatment for non-arena evaluation.
- §7.1's warning stands for win-rate work: do not spend more here expecting
  wins. This section exists because the operator asked for combat strength as
  its own objective, and that is what the instrument certifies.

Separately in the same change: the Advanced military step's enemy list
excluded the barbarian seat, so with no major war running every soldier took
the peacetime path while raiders pillaged home districts (live run
`civvis-20260807T172510Z`). The list now admits the barbarian seat when it has
a presence within `HOME_THREAT_RADIUS` of our cities, the step consults
`garrison_step`/`home_defense_objective` (barbarian-scoped) ahead of the
campaign march, and a claimed responder closes decisively instead of hovering
at the raider's reach. Scoping to the barbarian seat is measured, not
stylistic: the unscoped version moved the melee bench −4.8 → −18.1 by
rerouting wartime defense, and the scoped version returns bit-identical
major-war benches.

## 10. The v3 portfolio: leaving a fight, and rotating the front (2026-08-08)

v2 made the search stronger inside the space it could express; v3 grew the
space. The gap was structural, and §9's own fitness function pointed straight
at it: [`reply_estimate`]'s gang-kill term prices a unit the enemy can pool
damage onto and kill at its full loss value — but the portfolio offered that
unit only three kinds of line, *attack*, *step-and-attack*, and *stand still*.
The fitness could see the pool closing over a wounded unit and no candidate
action could take it out. The same asymmetry ran the other way: the per-unit
mover retreats threatened units after the plan, but it decides alone, against
its own objective, and the plan was scored assuming the unit stayed put.

Three changes, each screened at 300 paired seeds on all four compositions and
kept only where the screen improved or held (block 5,000,000; every number is
against stock `advanced` on the same harness whose control pairs to exact
0.00):

- **Withdraw lines.** Movement-only lines for any unit standing inside at
  least one enemy battery's mobility-true reach: one step, and two steps for
  units with two or more movement points — the attack lines' strictly-more-
  than-two gate deliberately does not apply, because there is no blow at the
  end of a withdrawal, and against two-move melee (reach 2) the second step
  is usually the one that actually exits the envelope. Candidates are ranked
  by the same battery/pooling arithmetic the fitness will apply —
  `trade_caution` times the drop in this unit's pooled price, minus
  [`FORTIFICATION_FORFEIT`] — and only tiles clearing that bar are offered:
  a withdrawal that dodges scratches is portfolio dilution, and one that
  breaks a lethal pool pays for itself several times over. At most
  [`MAX_WITHDRAW_LINES`] per unit, **appended after the attack truncation**
  so a retreat can never crowd a shot out of the portfolio. Screen:
  combined +213.9 → +262.3, ranged +354.4 → +434.8, melee +13.1 → +27.8
  (p = 0.0024), siege noise-flat.
- **Handoff steps.** A step may now land on a tile currently occupied by
  another *engaged* friendly — one that has its own portfolio and might
  vacate first. The order permutation is what arranges vacate-then-occupy,
  and the engine is what enforces it: an unvacated handoff step is refused
  at evaluation and the line dies there, exactly like any other illegal
  member of the geometric superset. This is the rotation move — the healthy
  unit taking over the tile its wounded teammate is withdrawing from, so
  the front holds its shape instead of thinning. A flat
  [`HANDOFF_DISCOUNT`] at pruning time prices the chance it never becomes
  legal. Screen, cumulative: combined +317.5, ranged +485.0, siege +258.3,
  melee +35.5 — and the melee sign test reaches p = 0.0013, the first time
  in this work's history that the melee cell's *direction* is reliable
  rather than merely its mean.
- **Withdrawn-unit authority.** Units whose winning line moved them without
  landing a blow (`TacticalPlan::withdrawn`) now hold for the rest of the
  turn: the wartime mover would otherwise re-decide the retreat and march
  the unit straight back toward the contact the plan just paid the forfeit
  to break. This is §7.4's lesson applied once more — the plan's value
  includes what the unit will *not* do next, and only the planner knows it.

The budget knee did not move this time: 40/24/16 measured +324.5 combined
against 32/20/16's +317.5 (inside one standard error) with melee identical,
so the shipped budget stays at 32/20/16.

`reply_estimate` was refactored onto shared `enemy_batteries` /
`victim_price` helpers so withdrawal priors and reply pricing cannot drift
apart; the arithmetic is unchanged and the full suite (including the
`advanced_v1` source-contract pin, re-pinned for the anchor-inert change) is
green.

### The instrument's army cells, recorded

Earlier sections named the compositions without recording them; these are the
exact `--army` strings v3 was screened and confirmed on, for future
comparability (the combined-arms cell is `SkirmishSetup::default`):

| cell | army |
|---|---|
| combined arms | `warrior,warrior,spearman,archer,archer,horseman` |
| ranged heavy | `archer,archer,archer,archer,warrior,warrior` |
| with siege | `catapult,catapult,archer,archer,warrior,spearman` |
| melee only | `warrior,warrior,warrior,spearman,spearman,warrior` |

### v3 results — 1000 paired fresh seeds a cell, seats swapped, control 0.00

| composition | exchange ratio (v3 vs stock) | paired swing | v2, same block | sign |
|---|---|---|---|---|
| combined arms | **1.821** vs 0.549 | **+319.9 ± 11.6** | +243.3 ± 11.7 | 798/183, p<0.0001 |
| ranged heavy | **2.634** vs 0.380 | **+512.6 ± 13.1** | +358.3 ± 13.3 | 886/92, p<0.0001 |
| with siege | **1.730** vs 0.578 | **+304.7 ± 14.4** | +180.6 ± 14.7 | 737/243, p<0.0001 |
| melee only | **1.132** vs 0.883 | **+25.4 ± 5.1** | −7.1 ± 4.9 (p=0.15) | 466/342, p<0.0001 |

Kills-per-loss ratio-of-ratios against the greedy production controller:
**3.3×** combined arms, **6.9×** ranged heavy, **3.0×** siege, **1.3×**
melee. The melee row is the qualitative change: v2 sat at noise on this
block exactly as §9 recorded, and v3 is decisively positive on both the
t and the sign test — the scrum finally rewards the one positional idea
it contains, pulling the unit the pool is closing over and backfilling
its tile.

Both arms of the comparison — v2 (the merged §9 configuration, rebuilt at the
same base commit) and v3 — were taken on the identical fresh seed block
(6,000,000+), disjoint from every screen above, so the v2 → v3 delta is a
paired reading on the same maps, not an artifact of block choice.

### Cost and fires at deployment scale

`battle_bench --cost`, one treated seat among five, 6p/74×46, interleaved:
**1.58×** a stock seat — v2 was 1.57×, so three new line families cost one
point of ratio; the lines are geometric at generation and the budget did not
move. Fires-check: the search planned on **71 of 303 treated seat-turns
(23.4%), reaching 261 unit decisions** — up from v2's 47 and 145, because a
withdrawal is a decision the search can now own on turns that offer no good
attack at all.

## 11. Arena deadlines are draws (2026-08-08)

Every Tactics setup surface offers a **50, 100, 150, or 200 turn** battle
clock; 100 was the default when this was written (§15 moved it to 250 and
added that rung on 2026-08-15). The command-line form is
`--tactics-turn-limit <turns>`. A general explicit `--turns` value still
overrides the Tactics choice for launchers that need a one-off cap.

Domination is the arena's only victory *lane*: the last army standing wins.
(§12 adds a second way for a battle to end, the capture-the-flag objective,
which answers to its own setup option rather than to a victory checkbox —
the same shape the Mercy Rule uses.) If both sides still have units after the
selected final turn, the battle is a true draw. Material, health, score, and
seat order may describe the position, but none breaks the deadline tie.

A draw is terminal even though it has no winner. Raw saves record
`victory_type: "draw"` with `winner: null`; observation and status documents
add `finished: true` and `draw: true`. Match series, league records, the
browser finale, and the production spectator supervisor all use that terminal
contract, so a drawn battle advances cleanly to the next scheduled game.

## 12. Capture the flag (2026-08-08)

An arena can be set up so that each side is given a **flag** instead of a
city. A side's flag stands where its city would have stood, and the battle is
won by moving a unit onto the **other** side's flag. The setting is `Capture
the flag` in the Tactics card, `tactics_flag` on `/new`, and `--tactics-flag`
on the command line.

Flags **replace** the city objective rather than joining it:
`TacticsRules::sanitized` forces `cities` to 0 whenever flags are asked for,
on every surface at once, so a flag battle is always city-less however the
cities control was left. Nothing places a flag anywhere but a seat, so a flag
is on whatever ground the seat is on and inherits the seating's symmetry: the
two sides open the same distance from each other's objective.

Standing on your **own** flag captures nothing. That asymmetry is the whole
mode: a flag is something to defend as well as something to take, both armies
deploy around their own, and the opening position is therefore not already
won. `Game::same_side` is what decides it, so teammates cannot take each
other's flags either.

The win fires in `Game::relocate`, which is the single point every march,
melee advance, airlift and retreat passes through, so there is no way to reach
a flag that does not check it. The result is recorded under a victory type of
its own, `FLAG_VICTORY` (`"flag"`). Like the Mercy Rule it is **not** one of
`VictoryConditions::NAMES`: `set_winner` admits it exactly when the battle has
flags at all. Saves carry `arena_flags` (seat → position), and the verdict
reads "Captured the Flag" in the browser and "captured the flag" in the
supervisor's record.

Both controllers aim at the enemy flag through `Game::arena_enemy_flag`, which
returns the nearest flag that is not the asking side's —
`AdvancedAi::domain_objective` for land columns, and `BasicAi::military_step`
ranked above every other march. It returns `None` on every world and on every
arena without flags, so the `advanced_v1` anchor's decision stream is
unchanged by construction and its source-contract pin is a compatibility
re-pin rather than an Elo-protocol change.

**Flags are never hidden.** `/state` publishes `arena_flags` to every viewer
regardless of fog, and both renderers paint them *after* their fog pass —
`drawFlatArenaFlags` on the bounded field, `drawPlanetArenaFlags` on the small
globe, with a contract test pinning that ordering. This is a deliberate rule
rather than an oversight: both commanders marched in knowing where the other
side's flag stood, and a capture-the-flag battle in which you must first go
and find the thing you are capturing is a different game. What the fog still
hides is everything *around* a flag — whose army is standing on it, and in
what strength.

The marker is a **column of light** in the owner's jersey, with a pennant at
its head, rather than a map symbol. A pennant at ground level is a few pixels
of terrain detail that disappears into a wooded hex; a beam is legible from
the far wall, at any zoom, and over the fog wash and the blank vellum of
unexplored ground alike. The globe has no screen-space vertical to raise a
beam along, so there the same light is a pulsing beacon on the tile itself.

**Measured.** Giving each side its own flag changed what the mode measures.
The first version of this feature put one neutral flag between the armies, and
same-controller battles ended at turn 3 to 5 with no fighting at all — first
touch on an even field is close to a coin flip, so it measured little beyond
who owned the fastest unit. With a flag each, the objective sits at the far
end of the field behind the enemy army, so reaching it means going through
them.

Eight same-controller battles, 20x20, stock economy — the spread is the
finding, so it is given rather than averaged away:

| battle length | surviving army of 48 | force posture |
| --- | --- | --- |
| t8, t8, t11 | 46–47 | 44–61% engage |
| t18, t20 | 40–41 | 57–69% engage |
| t31, t43, t50 | 22–28 | 74–80% engage |

The mode spans two outcomes rather than producing one. A quarter of battles
are still quick captures where a fast unit gets through before the lines meet
and almost nobody dies; the long ones are grinding fights that cost half of
both armies. Length, engagement and casualties move together, which is what
says the race and the fight are the same problem rather than two things
happening in the same battle. Beware quoting a single figure from this: an
earlier draft of this section cited one game's 74% engage as though it
described the mode, and the honest range is 44–80%.

## 13. Scenarios, and Trafalgar (2026-08-09)

Everything above this section measures. A **scenario** does not, and the
distinction is the whole reason it is a separate kind of Tactics map rather
than another arena preset.

An arena's claim is that the two sides are even: the same roster on both ends
of the same field, played twice with the seats swapped, so what is left in the
ledger is the play. That is what makes `battle_bench` and
[`src/skirmish.rs`](../src/skirmish.rs) instruments. A scenario gives that up
on purpose. It re-fights one particular engagement, with the forces that were
actually there, which at Trafalgar means twenty-seven against thirty-three.
**No number out of a scenario compares two agents**, and nothing in this
section should be quoted as though it did.

What a scenario is for instead: a position with a known right answer. Nelson's
plan is famous precisely because the obvious move — form line and engage van
to van — is the wrong one, and a controller that finds the same answer he did
has demonstrated something an even fight cannot ask about.

`MapScript::is_scenario` is the marker, and it changes four things:

- **The chart is fixed.** [`src/trafalgar.rs`](../src/trafalgar.rs) holds a
  30×24 chart of the Gulf of Cádiz — open sea, the Andalusian shore along the
  east, Cape Trafalgar and its shoals — and the seed moves nothing on it. Two
  launches are the same battle or it is not a scenario.
- **The seats are history's, and are not dealt out.** Every other layout in
  `mapgen` shuffles seat order so it cannot correlate with the order the ends
  are listed. Here it must not: Britain is seat 0 because **Britain moves
  first**, and seat 0 is also the seat a person plays.
- **Each side gets its own order of battle**, from a table naming every ship,
  rather than one roster mirrored onto both ends.
- **The economy is the battle's.** `TacticsRules::for_script` strips the
  cities, production, gold, research, uniques and flag whatever the Tactics
  card asked for. What survives is the clock and the series length, because
  neither is a claim about 1805. The browser greys the rest out rather than
  offering controls the server is about to overrule.

### The position

Noon on 21 October 1805, as Collingwood came within range and twenty minutes
before Nelson did. Two British columns in line ahead heading east — the
weather column on row 10, the lee column five rows south and one hex further
on, which is the head start the freshly-coppered `Royal Sovereign` had — with
`Africa` alone in the north where she had been separated in the night. Facing
them, thirty-three ships on the starboard tack heading north for Cádiz, in a
crescent bowed to leeward, one deep in the van and two deep from the centre
aft with Gravina's squadron of observation outboard of the rear. Behind them,
close enough to see, the cape they could not fall back past.

### What is abstracted, and what it costs

Stated plainly, because a scenario that quietly flatters itself is worse than
no scenario:

- **Every ship of the line is a Frigate.** The ruleset has one sailing warship
  of the age and the *Santísima Trinidad* and the little *Africa* are both it.
  Rate is carried on top of that as a promotion — see below. What is still not
  modelled is the Royal Navy's rate of fire, the thing that actually decided
  the exchange once the lines were locked, so what is left on the board is the
  part Nelson chose: where sixty ships were.
- **The wind is not modelled**, and it decided a great deal. What survives of
  it is geometry: the van starts far from the fighting and cannot easily get
  back, which is what happened to Dumanoir for most of the afternoon.
- **Distances are compressed** — one hex between ships in a column, and a
  shore drawn a few hexes off a rear that was nine miles from it.
- A Frigate is a *ranged* unit with two tiles of reach, which favours the
  formed line over the fleet crossing open water toward it far more than
  broadside gunnery did. The approach costs the attacker more here than it
  cost the Royal Navy.

That last point shows up immediately in play. Two `advanced` controllers on
the stock clock spend the opening trading the approach badly — through the
first forty turns Britain was down ten ships and the Combined Fleet none —
and then dissolve into a general melee rather than either holding the line or
cutting it. Neither side plays Trafalgar. That is a finding about the
controllers and about the ranged abstraction above, not a result, and it is
the reason this section makes no claim beyond it.

### Rate, as promotions (2026-08-10)

A scenario that draws every ship as the same unit says the *Santísima
Trinidad* and the *Africa* are the same ship, which they are not. Promotions
are the natural place to say the difference: they are the engine's own
per-unit modifiers, combat already reads them, and granting one at setup is
the same act as a unit having earned it.

`trafalgar::rate_promotions` maps a gun figure to a promotion set, by **one
rule applied to both fleets** — it never asks whose flag is up:

| rate | promotion | ships |
| --- | --- | --- |
| 64 and under | none | 3 British, 1 Spanish |
| 74 and over | `line_of_battle` (+7 Ranged Strength against naval units) | 24 British, 32 Combined |

**Why two bands and not three.** The obvious third band is the seven
three-deckers, and it was built and then measured out again. The Frigate's own
promotion tree has exactly one promotion that adds broadside against ships —
the one above. The rest of it is anti-land, anti-district, anti-air, or
healing, which `Game::unit_heal_rate` switches off outright on every Tactics
map. This battle has no land units, no districts, no aircraft and no healing,
so the only remaining lever for a first rate was `coincidence_rangefinding`,
+1 attack range.

Three seeds a configuration, stock controllers, 100-turn clock:

| ladder | 1805 | 7 | 42 |
| --- | --- | --- | --- |
| no promotions | draw | draw | draw |
| `line_of_battle` at 74+ | draw, 25 against 12 | draw | draw |
| plus +1 range at 100+ | **France, turn 29** | **France, turn 28** | **France, turn 27** |

A ship that outranges everything fires without reply, and the Combined Fleet
had four such against Britain's three. That turned a hundred-turn action into
a rout inside thirty — not because the Combined Fleet was heavier, which it
was, but because the stand-in was far stronger than the thing it stood in for.
A three-decker's guns did not shoot appreciably further; what she had was
weight, and the board has no way to say "heavier still". So a first rate is a
ship of the line and no more, and a test asserts it stays that way rather than
leaving the next reader to rediscover the measurement.

The broadside band itself is close to inert on the whole-battle result — three
draws before and after — which is the expected shape: it separates four ships
out of sixty. It is in because the scenario should describe the fleets
correctly, not because it was expected to move a number.

### Admirals, rated in stars (2026-08-11)

Nine flag officers were present at Trafalgar and the scenario now carries all
of them, on the ship each actually flew in, rated 2 to 5 stars.

| admiral | ship | stars | on the board |
| --- | --- | --- | --- |
| Nelson, commander-in-chief | *Victory* | 5 | +1 movement, Fleet (+10 Strength) |
| Collingwood, second in command | *Royal Sovereign* | 4 | +1 movement, Fleet |
| Gravina, squadron of observation | *Príncipe de Asturias* | 4 | +1 movement, Fleet |
| Northesk | *Britannia* | 3 | +1 movement |
| Cisneros | *Santísima Trinidad* | 3 | +1 movement |
| Álava | *Santa Ana* | 3 | +1 movement |
| Magon | *Algésiras* | 3 | +1 movement |
| Villeneuve, commander-in-chief | *Bucentaure* | 2 | +1 movement |
| Dumanoir le Pelley, the van | *Formidable* | 2 | +1 movement |

**Why a threshold rather than a bonus per star.** There were three British
flags and six in the Combined Fleet, so anything paid out per flagship hands
more of it to the larger, more admiral-heavy side — the opposite of what the
feature exists to say. Paying only for admirals rated 4 or better gives
Britain two and the Combined Fleet one, and the asymmetry then falls out of
rating the men rather than out of a thumb on the scale.

**The mechanism.** A Fleet (`formation` 1) is +10 Strength through
`unit_formation_bonus`, which `unit_ranged_strength` includes — so it reaches a
ship of the line's broadside. It costs a reinterpretation: a Fleet in the
shipped rules is two ships merged, and `unit_production_cost` prices one at
1.5x, so the three flagships weigh half again as much in a material ledger.

**What was tried first.** The rating was originally spent on **flanking** —
`flanking_bonus` pays +2 for every friendly ship adjacent to the target beyond
the attacker, multiplied by the owner's naval flanking bonus, and the ruleset
already ships Horatio Nelson as a Great Admiral at +50%. Cutting a line and
doubling on what it isolates is Nelson's whole plan, so it looked like the
right home. It cannot work here, for two independent reasons, both measured:

1. `flanking_bonus` is only ever called from `do_attack`, the melee path.
   Every ship here is a Frigate, which is `naval_ranged` and attacks through
   `do_ranged` — which never consults it.
2. The ships never close anyway. Over 120 turns played by the stock
   controllers, **no ship was ever adjacent to two enemies**; the most any ship
   ever had alongside was one. A unit that shoots from two tiles away has no
   reason to come to contact, and does not.

Worth keeping because it generalises: a mechanic that reads adjacency is
unavailable to an all-ranged force, whatever the history says it should model.

**Measured**, ten seeds a configuration, stock controllers, 100-turn clock:

| | draws | Combined Fleet wins |
| --- | --- | --- |
| no admirals | 8 | 2 (turns 66, 98) |
| admirals | **9** | **1** (turn 79) |

Material on seed 1805: 25 against 12 without admirals, **20 against 12** with.
Britain trades better and survives two seeds it previously lost, without the
result swinging the other way — which is the size of effect wanted here after
the rate experiment showed how easily a per-ship bonus overshoots.

## 14. The setup order (2026-08-15)

Operator-directed. The lobby's Tactics questions are asked in the order they
depend on one another, and every answer narrows the next:

1. **Game mode** — Civ or Tactics.
2. **Human players** — who is at the keyboard, unchanged from Civ.
3. **Scenario** — **Custom** first and by default, then every catalogued
   battle (`historical_scenarios::SCENARIOS`, Trafalgar included) grouped by
   era, earliest first. A named battle brings its own map, opening forces, era
   and clock, and fixes the arena economy it was fought under (§13); its
   briefing — ground, objective, both orders of battle — sits directly under
   the control.
4. **World type** — *Custom only.* **Flat**, a bounded field walled on all
   four sides, or **Planet**, a small globe with the two sides on opposite
   faces of the world.
5. **Map** — *Custom only,* and cut to the world type: a flat world offers
   **Land** (`MapScript::Battlefield`); a planet offers **Land**
   (`TacticsPlanet`) or **Ocean** (`TacticsOcean`). Only one Land is ever on
   the menu at a time, which is why the two can share the name — the ids they
   travel under have not changed, so saves, sweeps and `?map=battlefield`
   deep links mean what they always did.
6. **World size** — whatever the combination above is drawn at, from
   `setup::battlefield_sizes()` filtered on the map. Flat Land is a ladder of
   three: **Square 10×10, March 10×20, Field 20×20** (ids `10x10`, `10x20`,
   `20x20`; the last is the field the site's Tactics link opens — #1626
   briefly renamed them Field/March/Battlefield and the operator put the
   original names back). Either globe is offered at diameters 8, 10, 15 and 20. A named
   battle lists only the sizes it is charted at — one each today, shown
   fixed — and a battle charted at a second size needs nothing but a second
   `BattlefieldSize` row.

Then the Tactics settings card, as before. The Civ mode's order (size, shape,
map) is untouched: the same controls are recomposed by `placeSetupControls`
whenever the mode moves, so each game asks its world questions in its own
order and a Civ choice survives a visit to Tactics and back. Reading a world
back into the panel — a page reloaded over a running battle, or staged
next-game settings — goes through the same steps (`adoptTacticsWorld`): the
battle if the map is a named one, else Custom with the map's world type and
the map, then the size whose dimensions these are.

What went: the scenario library browser (#1458's era/commander/terrain lens
tabs and card grid). It chose the same thing the Scenario select now chooses,
one control up, and a setup pass reads better as one column of questions than
as a column with a catalog folded into it. The catalog data it drew on is
unchanged and still feeds the select and the briefing.

## 15. The stock arena: two standing armies, no reinforcements (2026-08-15)

Operator direction. The stock Tactics battle now opens with the two armies
each side is dealt and **nothing behind them**: `TacticsRules::default()` is
one city a side, **0 Production**, **0 Gold**, a technology every five turns,
and a **250-turn** clock (a new top rung on the turn-limit ladder). No unit
that was not on the field at turn one ever joins it, and nothing is upgraded
mid-battle; the city stays as the objective the other side is coming for.
Until this change the stock arena granted 30 Production and 30 Gold a turn on
a 100-turn clock — every "stock economy" figure above was measured under that.

Production per turn and Gold per turn are **still settings** on every setup
surface — the lobby's Tactics card, `--tactics-production` / `--tactics-gold`
on the command line, `tactics_production` / `tactics_gold` on `/setup` — so a
reinforcement battle is one menu away; the default is what moved, not the
menu.

Two things kept honest by the profile:

- The rating profile carries the arena's grants
  (`arena=cities:1,production:0,gold:0,...`), so a Tactics ledger written under
  the old default stays matched to its own arena rather than being read as
  the new one.
- `tools/tactics_bench.py` now pins the economy its baseline was recorded
  under (`ECONOMY`: 30/30, five turns a tech, 100-turn clock) instead of
  inheriting the stock arena, so `docs/TACTICS_BASELINE.md` still means what
  it says. Re-baselining on the new stock arena is a deliberate change to that
  pin, made in the same pull request as the new figures.

Two rules had quietly assumed reinforcements, and both are corrected here so
the stock battle can actually be won:

- **Last army standing, with a city.** `check_elimination` kept a side with
  a city alive after its last unit fell, on the stated ground that "it
  collects Production every turn and the next unit off the queue puts it
  back on the field". With no grant that is false, and measured on the new
  stock arena the result was a battle nobody could win: seed 7 on the 20x20
  field had Egypt annihilate Rome's whole company by turn 38 and then draw
  at the clock, because four archers cannot walk into an empty city, and a
  first `advanced`-vs-`basic` tournament went 4/4 draws at t500. A side with
  no units is now finished unless it holds a city **and** the arena grants
  Production or Gold to field another unit from; a defeated seat's empty
  city satisfies the Domination lane on an arena the way a defeated seat with
  no city already did. Same seed, same field: Egypt wins by domination on
  turn 38. Eight `advanced`-vs-`basic` battles then decided in 24–36 turns
  (3–4, one t500 draw with survivors on both sides).
- **A drawn arena battle ends its tournament game.** `run_tournament` looped
  `while game.winner.is_none()`, and a Tactics draw is terminal with
  `winner: None` — so a drawn battle was played on forever by both
  controllers at full CPU. Under the reinforced default a tournament arena
  battle almost never drew inside 500 turns, so this had never bitten; the
  first stock tournament after the change hung four workers. The loop now
  runs `while !game.is_finished()`, drawn games are rated as games nobody
  won (the path already existed), and a twelve-turn arena tournament is the
  regression test.

Old saves are unaffected: a save that predates the turn-limit field still
loads its 100-turn clock (`legacy_turn_limit`), the same way a save that
predates fog loads unfogged.

## 9. The capture regression that was not (2026-08-17)

A stale `TACTICS_BASELINE.md` produced a false alarm and hid a real result. Both
halves are worth recording, because the mistake is cheap to repeat.

**The false alarm.** The committed table said `advanced` took 97.5% of the
`1 city per side` regime against `basic`. Re-measured on 2026-08-17 it read
75.8%, and that was reported as a 21.7-point regression in a shipped product,
qualified as "holds at 120 games, so it is not sample noise".

It was sample noise, and the qualification was the error: **the two numbers came
from different sample sizes.** Only the new figure was 120 games. Rebuilding the
2026-08-15 commit and measuring it properly:

| capture, `advanced` v `basic` | n | pct | 95% CI |
|---|---:|---:|---|
| recorded at `7cd011bb` | 40 | 97.5% | 87.1–99.6 |
| re-measured at `7cd011bb` | 120 | 81.7% | 73.8–87.6 |
| re-measured at `7cd011bb` | 480 | **81.2%** | 77.5–84.5 |
| measured at `04d9c805` | 480 | **77.3%** | 73.3–80.8 |

The same binary measures 81.2% at 480 games and 97.5% at 40, and the recorded
figure's interval does not overlap its own binary's 480-game interval. Sixteen
of the twenty-two points were never there. The remainder is **p = 0.136, no
established difference**, and the same pair against the frozen anchor moved the
*other* way (58.8% → 64.4%, p = 0.074). Two columns in opposite directions with
neither significant is noise, not a regression.

**What the stale table was actually hiding.** #1858 routed the bounded
joint-tactics search through the arena movement seam. Measured across its own
parent, 240 seat-mirrored games per matchup:

| no cities | before | after | | |
|---|---:|---:|---:|---|
| `advanced` v `basic` | 60.4% | **87.9%** | +27.5 | p = 6×10⁻¹² |
| `advanced` v `advanced_v1` | 92.9% | **99.6%** | +6.7 | p = 1×10⁻⁴ |

That is ROADMAP objective 4 delivering, and it went uncredited for two days for
the same reason the phantom regression went unchallenged: nothing said how old
the table was.

**What changed as a result.** `--games` defaults to 120, `--write-baseline`
refuses fewer, and the refusal fires before any games are played. A baseline is
what every later run is compared against; at 40 games it is a ±15-point
instrument, and this is what that costs.

## 16. Coordinated finishing: the volley as its own part, plus a three-blow chain (2026-08-18)

Operator goal: 2–3 units standing near each other should work together to
eliminate enemy units — an enemy two blows from death is a good group target.

**Where that behaviour lived, and why it was dormant.** §8's bounded
friendly-volley extension is exactly this mechanism for the two-blow case, and
it shipped (#1360) inside `BasicAi::tactical_strategy`. The war-half removal
(#1589) then took `tactical_strategy` out of production as part of a
four-flag bundle on a +38 whole-game gate — a **composite** gate, which never
priced the volley on its own. Since then the production greedy path has had no
multi-unit kill reasoning at all: `prioritize_immediate_kills` spends
single-blow finishes, and the `focus_fire` gene is a flat nudge. (The joint
search of §§9–10 covers the arena seam and the live bridge, not the world-game
greedy controller that `battle_bench` measures.)

**Two changes, one flag each.**

- **`AdvancedAi::coordinated_finish`** admits the friendly-volley extension
  without the rest of the closed bundle. The volley's role bonuses read 0 in
  this configuration (`tactical_action_bonus` is still gated on
  `tactical_strategy`), so what is enabled is the kill-credit and the
  reply repricing — the coordination, not the war-half doctrine. Evaluator
  arm: `advanced_coordinated_finish`.
- **`AdvancedAi::volley_chain`** extends the volley to a **pair of
  finishers** when no lone teammate can complete the kill, so a three-blow
  group kill is visible to the setup shot. A single finisher is always
  preferred; the chain runs only when the single-finisher search comes up
  empty, behind a closed-form damage prefilter (the two heaviest remaining
  mean blows, with 1.5× headroom for the roll spread, must cover the
  survivor's health) so a healthy defender never pays the clone enumeration.
  Both blows must clear their own exact-exchange bar and the reply is priced
  once, after the whole friendly sequence, against all three exposed bodies.
  Withhold arm: `advanced_single_finisher_volley` (the treatment minus only
  the chain).

**Measured.** Screens at 300 paired seeds a cell (block 21,000,000+), kept on
a fresh confirmation block (22,000,000+) at 1000 paired seeds a cell, seats
swapped, control at exact 0.00 both for `advanced` and for the treatment
against itself. The confirmation was then **re-taken on this branch's merged
tip** — five PRs (#2059 evacuation among them, the same tactical subsystem)
landed underneath the first reading and roughly halved it, which is the
repository's standing rule about tuning-era figures in action. The merged-tip
column is the one this document claims. `advanced_coordinated_finish` less
`advanced`, 1000 paired seeds a cell:

| composition | pre-merge base | **merged tip** | merged-tip sign |
|---|---:|---:|---|
| combined arms | +32.9 ± 5.9 | **+18.1 ± 4.6** | 187/118, p = 0.0001 |
| ranged heavy | +85.6 ± 9.4 | **+49.2 ± 6.6** | 304/157, p < 0.0001 |
| with siege | +21.8 ± 7.9 | +9.6 ± 5.2 | 116/102, p = 0.38 — a null |
| melee only | +7.5 ± 1.7 | **+5.4 ± 1.4** | 75/30, p < 0.0001 |

Decisive on mean **and** sign on combined arms, ranged and the melee-only
stress case that harmed every early version of the joint search; siege
dropped to a directional null on the tip; no composition harmed. Fires-check:
play diverged on 448 of 1000 combined-arms confirmation seeds. The chain's
own share on the merged tip (`advanced_coordinated_finish` less
`advanced_single_finisher_volley`, 300 seeds a cell): ranged **+31.1 ± 8.3**
(sign 53/21, p = 0.0003), combined +5.9 ± 3.4 (directional), siege +4.1 ± 5.7
(noise), melee −0.9 ± 1.7 (2 of 300 seeds diverged — the chain is nearly
unreachable there, as expected: three-blow ganging is a ranged shape). The
single-finisher volley returning to a live path carries the rest.

**Cost.** `battle_bench --cost`, one treated seat among five, 6p/74×46,
interleaved: **1.03×** a stock seat (the joint search is 1.58×). The printed
joint-tactics census correctly reads 0 for this arm — the volley has no
census hook; its fires evidence is the divergence count above.

**Whole game.** 150 pairs at 6p/74×46/9cs/250t, seed block 23,000,000,
`advanced_coordinated_finish` v `advanced`: RESULT-PENDING.

The frozen anchors are untouched by construction (`legacy()` carries neither
flag; the `ANCHOR_BEHAVIOUR_FNV` fingerprint passes unchanged), and stock
`advanced` keeps today's behaviour unless the flag is promoted on the
whole-game evidence above.

## 17. Approach lines from the exact reach (2026-08-19)

The one-step and two-step approach blocks of §9 walked rings by geometry:
every step cost 1, a two-step line needed "strictly more than two" movement,
and an intermediate tile beside a hostile was skipped by hand. Three things
were wrong at once. A two-move unit on a road, or a three-move one over flat
ground, reaches a firing tile two hexes out with movement to spare and got no
line; a one-step line onto hills-and-woods was offered, refused when played,
and stood the unit in contact unfortified; and a four-move horseman's third
and fourth hexes did not exist at all — the mobility the closed-form reply
already priced for the *enemy* (`enemy_batteries`, §9) was invisible for our
own lines.

**What changed.** `Game::approach_reach` is the engine's own movement flood
with one difference from `reachable`: entering enemy zone of control still
stops the walk, but the movement the unit keeps on arrival is reported rather
than zeroed, because a unit that stops in a zone of control keeps its unused
movement for the blow (`do_attack`, `do_ranged`, `can_pay_melee_entry`). The
two-step block is gone; from two hexes out, a line is offered from every tile
the unit reaches with movement left for the strike — for melee, enough to pay
the defender's tile; for ranged, any — along the flood's own path, priced by
the same prior as before less `APPROACH_STEP_TOLL` (4) per step. The one-step
block stays (it also owns the handoff onto a teammate's tile, which no flood
can express), siege stays grounded, and every line is still played through
the engine at fitness time. Only tiles within the unit's range of a hostile
unit, an at-war city or an unpillaged Encampment are scanned for strikes.

**Screen** — 300 paired seeds a cell, block 7,200,000, `advanced_joint_tactics`
v stock `advanced`, base (§16 tip) against this change:

| composition | base | **this change** |
|---|---:|---:|
| combined arms | +365.9 ± 20.5 | +374.1 ± 20.9 |
| ranged heavy | +498.8 ± 23.5 | +498.8 ± 23.5 |
| with siege | +175.0 ± 25.6 | +173.9 ± 26.6 |
| melee only | +57.0 ± 10.5 | +56.4 ± 10.4 |
| **cavalry** `horseman,horseman,horseman,heavy_chariot,archer,archer` | +398.0 ± 21.4 (1.720 v 0.582) | **+476.2 ± 22.0** (1.879 v 0.532) |
| **mounted medieval** `knight,knight,courser,crossbowman,crossbowman,pikeman` | +495.7 ± 49.6 (1.314 v 0.761) | **+590.2 ± 48.9** (1.384 v 0.722) |

**Confirmation** — fresh block 7,300,000: cavalry +352.3 ± 20.0 → **+409.7 ±
21.6** (exchange 1.651 → 1.744); mounted medieval +601.1 ± 50.6 → +622.1 ±
48.5 (1.367 → 1.393); combined arms +372.8 ± 21.2 → +370.7 ± 21.5. The four
foot compositions hold within one standard error on both blocks; the two
mounted compositions — where the old geometry threw the third and fourth
hexes away — gain on both. Sign tests are decisive on every cell either way
(the search was already far ahead of the greedy rule); the delta is read from
the paired means on identical seeds.

The frozen `advanced_v1` anchor never runs the joint search and is untouched;
the arena `advanced` and the live bridge pick this up automatically. See
`docs/LIVE_TACTICS.md` §7 for the program this belongs to.

