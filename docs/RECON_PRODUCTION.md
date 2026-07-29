# Recon-family production saturation

Status: **preregistered from production saves and code; an evaluator-only,
default-off treatment is implemented on draft PR #606; no focal seed has been
run or read**.

## Implementation checkpoint (2026-07-29)

Draft PR #606 implements the frozen treatment and evaluator without changing
any shipped controller default or any decision threshold in this document.
Recon identity is derived from rules metadata and carried through Basic,
Advanced, and the focal Strategic controller's live and counterfactual agents.
The observer reads completed production at the start-of-turn boundary by stable
unit ID, including Formation production that the engine's `trained:*` counter
does not record, and reads successful Gold/Faith purchases from the action log.
Focused tests cover the family helper, active and first-queued upgraded units,
Basic and Advanced production and purchases, zero-count score identity, the
four-build opening, same-turn multi-city replacement, Strategic branch
propagation, deployment scheduling, inference/gate arithmetic, and observer
accounting. No registered screen or confirmation seed was started or inspected
while the shared Strategic Expansion oracle owned the simulator queue.

## Production observation

The frozen archive is the latest 50 completed production saves at the
2026-07-29 cutoff
`20260729T150427.366946Z-seed-870394659-turn-261-instance-27278`. It contains
400 major-civilization records from eight-player Continents, Poles, Online
games with Science, Culture, and Domination enabled and a policy-visible turn
limit of 250. The open viewer selected that operating stratum. Twenty-five
worlds are Flat (84x54, 4,536 stored tiles) and 25 are Planet (105x44 globe,
4,412 stored tiles). The saves span 26 clean embedded revisions, so this is a
rolling-fleet prevalence census rather than a homogeneous treatment sample.

`Player::counters["trained:scout"]` increments only after a real Scout is
placed by completed production or purchase. Across the frozen archive:

| production observation | count |
|---|---:|
| seats training at least one Scout | 383 / 400 |
| seats training at least ten | 218 / 400 |
| seats training at least fifty | 35 / 400 |
| seats training at least one hundred | 7 / 400 |
| median / 75th / 90th / 95th percentile | 11 / 28 / 42 / 60 |
| 99th percentile / maximum | 162 / 239 |
| Scouts trained | **7,823** |
| final Scouts / final recon-family units | 1,708 / **4,526** |
| Scout kills / all recon-family kills | 219 / 2,273 |

The mean is 19.56 Scouts trained and 11.32 surviving recon-family units per
major seat. A Scout costs 30 base Production, or 15 at Online speed. If every
record represented production, the count would equal 117,345 Online
Production. The counter deliberately combines completed builds with Gold or
Faith purchases, so that number is a nominal Production-equivalent, not a
claim about the currency actually spent. Either path consumes a scarce city
queue or treasury and leaves a unit on the map.

The units do not track remaining exploration need. Mean terminal explored
share was 93.25%. Twenty-seven seats had 31 Scouts still first-queued in their
final save, and 25 of those seats had already explored every stored tile. The
signal survives both geometries: the Planet half trained 3,125 Scouts and
finished with 1,788 recon units; the Flat half trained 4,698 and finished with
2,738. Median training was 10 and 13 respectively.

The tail is not a bookkeeping artifact. In one turn-322 Khmer victory, a
15-city empire had trained 239 Scouts, explored all 4,536 tiles, and still
owned 42 Scouts, 45 Skirmishers, three Rangers, and six Spec Ops. Those 96
survivors had made 38 recorded kills, only one with a Scout. A turn-269 Kongo
winner trained 172 and finished with 138 Scouts plus three upgraded recon
units after full exploration; the entire family made 11 kills. Other
full-exploration seats trained 169, 162, and 150 Scouts and retained 120, 136,
and 104 recon units.

Winners averaged 38.44 Scouts trained versus 16.86 for non-winners. That is
confounded by survival, conquest, city count, treasury, and production
capacity; it is not evidence that Scout saturation helps. Likewise, this
archive cannot say what an alternative queue choice would have produced. It
establishes prevalence and supplies a source-level hypothesis only.

## Mechanism hypothesis

The production policy loses recon-family identity at exactly the point where
an upgraded Scout should satisfy the existing one-unit guard:

1. `EmpireCounts::add_unit` special-cases the literal name `scout` and
   increments `counts.scouts`. Skirmishers, Rangers, and Spec Ops enter only
   the generic military counters even though every one has
   `promotion_class = "recon"`.
2. `production_value` rejects a Scout only while `counts.scouts >= 1`; an
   upgraded recon unit therefore reopens the slot. `counts` already includes
   the first queued item in every city, so the defect is identity, not absent
   queue accounting.
3. Live adaptive controllers carry no explicit `victory_target`. During a
   Recovery plan they run `advanced_production`, but its saturated-domain
   rejection is conditional on an explicit non-Domination target. With no
   force gap, a Scout therefore retains positive raw military value.
4. Scout is exempt from the evaluator's weak-unit rejection and has no
   `obsolete_tech` in the rules. Its 30 base cost then receives the strongest
   completion-time divisor among military candidates.
5. Production happens before `BasicAi::upgrade_units`. An affordable upgrade
   can remove the exact Scout at the end of the turn and reopen the literal
   slot for the next turn while preserving the growing recon army.
6. The adaptive path can subsequently call the Basic city governor, whose
   generic military selection also has no recon-family cap. This supplies a
   second AI-initiated route and is consistent with final saves containing two
   simultaneously queued Scouts.

The missing obsolescence fields and the adaptive saturated-force condition may
amplify the loop, but changing either would test a different policy. The
smallest correction is to make the already expressed one-Scout invariant
recognize the whole promotion family everywhere the AI initiates a unit build
or purchase.

**Frozen hypothesis:** treating active and first-queued recon units as one
promotion family will stop repeated Scout replacement after upgrades, preserve
one exploration/patrol unit, and redirect enough queues and treasury to improve
strategic strength without materially reducing map coverage.

## Frozen treatment

A later implementation may add a default-off `recon_family_cap` controller
entrant. It must not change shipped controller defaults while this experiment
is running. With the flag enabled:

1. recon membership is derived from the rules field
   `promotion_class == "recon"`; no hard-coded successor list is allowed;
2. the empire recon count includes every owned unit and every first-queued
   `Item::Unit` or `Item::Formation` in that family;
3. the existing first-four-build opening book remains byte-for-byte unchanged,
   including a scripted opening Scout;
4. after the opening book, every Advanced and Basic AI-initiated production,
   Gold-purchase, or Faith-purchase candidate in the family is ineligible when
   the count is at least one;
5. applying a new queue or purchase updates the family-aware count before
   another city is considered, preventing same-turn duplicates;
6. when the count is zero, the stock candidate scores, rankings, tie breaks,
   and replacement behavior are unchanged; and
7. upgrading an existing recon unit remains legal and continues to satisfy the
   cap because its promotion family does not change.

This is a production-eligibility treatment, not a unit retirement policy. It
does not delete, disband, gift, combine, or downgrade existing units; alter
unit cost, maintenance, strength, movement, combat, upgrades, or
`obsolete_tech`; change exploration or patrol movement; add a map-coverage
threshold; change the army-size target; or grant Production, Gold, Faith, or
information. Non-recon candidates retain the stock calculation.

Focused tests must establish all of the following before a focal run:

- each of Scout, Skirmisher, Ranger, and Spec Ops satisfies the family helper
  through rules metadata;
- one active or first-queued upgraded recon unit suppresses another recon
  candidate in both Advanced Recovery production and the Basic military
  fallback, including purchases;
- zero active/queued recon units still permit a stock-ranked replacement;
- the first four capital builds and ordinary non-recon production are
  unchanged;
- multiple empty cities cannot queue more than one replacement in a turn; and
- the default-off entrant is replay-identical to stock.

## Frozen evaluator

Add an evaluator-only `recon_family_eval`. Every independent map is replayed
four times: focal seats 0 and the final major seat under the stock production
fleet and under the default-off treatment entrant. The two focal seats are
averaged inside the map, and the map is the only inference unit. Every
major in both arms uses the committed generation-14 champion from
`data/evolved/best.json`, byte fingerprint `fnv1a:40b1fbb2a5b88bc6`, with the
exact `strategic_deep` review cadence 20 and horizon 80. The JSON is compiled
into the evaluator and its generation and fingerprint are asserted before a
game is constructed. The controller is explicitly score-share-only: the live
deployment has no promoted `valuenet.json`, and an optional working-directory
model may not change this experiment. Only the focal treatment arm enables
`recon_family_cap`; every rival stays otherwise identical. City-states and
barbarians retain their stock minor paths.

This controller pin was frozen after the implementation self-audit and before
any null, screen, or confirmation seed was run or read. The earlier factory
construction let a working-directory `evolved/best.json` or `valuenet.json`
silently change both arms while the default-off null still passed. The pin
changes no treatment, outcome, threshold, seed, population, or queue order; it
makes the intended deployed estimand reproducible.

The evaluator uses the deterministic deployment-population schedule already
frozen for the Spaceport and horizon studies. For zero-based map offset `i`:

- players are `[4, 6, 8, 10, 5, 7, 9][i mod 7]`;
- scripts are `[land_only, water_world, continents, true_start_earth, lakes,
  inland_sea, pangaea, small_continents, islands][i mod 9]`; and
- topology is Flat at even `i` and Planet at odd `i`.

`MapSize::for_players` must derive dimensions and city-state counts. The three
periods are pairwise coprime and cover the 126 unattended rollover profiles
without reweighting. Every phase restarts at offset zero. Axis-specific results
are descriptive only and cannot promote, extend, or rescue the pooled gate.

Each game keeps `Game.max_turns = 250` and continues the same stateful agents
externally through turn 320 or an enabled victory. The runner must assert the
policy-visible horizon never changes. It reports, by focal arm and paired map:

- completed production and purchase actions for every recon-family type,
  separately by Production, Gold, and Faith;
- Scout and total recon training per 100 observed focal turns, nominal Online
  Production-equivalent committed, terminal family count, and the maximum
  family count reached;
- recon orders issued after 90% and 100% map exploration;
- turns to 50%, 80%, 90%, and 100% exploration, explored share at turn 200,
  and terminal explored share;
- family combat kills, terminal military power, cities captured and lost;
- total and victory-type wins, Science-race progress, finish turn, terminal
  score, cities, districts, buildings, and treasury; and
- paired map win score, paired terminal-score share, complete
  favorable/neutral/adverse directions, and exact two-sided sign tests.

The new diagnostics are observer-only and must never enter a game decision.
Generated output is not committed.

## Frozen default-off null

Before either treatment seed is opened, one four-map causal null at disjoint
seed `9971999` compares the pinned controller with the same custom Strategic
entrant carrying `recon_family_cap = false`:

```text
recon_family_eval --null --deployment-mix --ai strategic_deep \
  --maps 4 --turns 250 \
  --observe-through 320 --speed online --poles poles --randomize-civs \
  --victories science,culture,domination --seed 9971999 --jobs 6
```

The four maps restart the deployment schedule at offset zero. For both focal
seats, the pinned stock and custom-default-off arms must match in the complete
canonical serialized `Game`, including RNG and action log, as well as every
reported result and observer census field. Any mismatch stops the study before
the screen. The runner may print the frozen PASS label only for this exact
profile; all other null invocations are diagnostic and cannot spend or replace
it. Neither screen seed `9972000` nor confirmation seed `9973000` may be used
for a preflight null.

This null amendment was frozen after implementation audit and before any focal
seed was started or read. It changes no treatment, endpoint, screen,
confirmation, threshold, map population, or resource limit. It closes the
instrument gap in which the original runtime null compared stock with stock,
retained only terminal summaries, and naturally defaulted to the untouched
screen seed.

## Fixed development screen

The one allowed screen is 12 maps / 48 games:

```text
recon_family_eval --deployment-mix --ai strategic_deep \
  --maps 12 --turns 250 \
  --observe-through 320 --speed online --poles poles --randomize-civs \
  --victories science,culture,domination --seed 9972000 --jobs 6
```

The screen passes only if every term holds:

1. stock control completes at least 120 recon-family training actions and at
   least 25% of its 24 focal games complete five, establishing the mechanism
   on the fresh deployment cell;
2. treatment recon-family training actions are at most 35% of control, and
   treatment orders issued after 90% exploration are at most 25% of control;
3. treatment terminal family count is at most 50% of control and the paired
   nominal commitment avoided is at least 75 Online Production-equivalent per
   focal game;
4. treatment turn-200 explored share is no more than two percentage points
   below control, and its median turn to 80% exploration is no more than eight
   turns later among mutually observed cells;
5. paired map win score and paired terminal-score share are each at least
   49.5%; and
6. treatment has no fewer total wins, Science wins, or Culture wins than
   control.

Failure means **STOP**: retain the stock policy, record the result, and do not
tune the cap, add an exploration threshold, change the seed, or inspect the
confirmation.

## Fixed confirmation and decision

A passing screen earns one unchanged 60-map confirmation at seed `9973000`
(240 games). Every other argument and the treatment remain fixed.
Confirmation requires every term below:

- stock again completes at least five recon-family training actions in 25% of
  focal games;
- treatment training remains at most 35% of control, post-90%-exploration
  orders remain at most 25%, terminal family count remains at most 50%, and
  nominal commitment avoided is at least 100 Online Production-equivalent per
  focal game;
- treatment turn-200 explored share is no more than half a percentage point
  below control and median time to 80% exploration is no more than four turns
  later among mutually observed cells;
- paired map win score is at least 52%, favorable win directions outnumber
  adverse directions with an exact two-sided sign-test `p < 0.05`, and
  treatment has no fewer total, Science, or Culture wins; and
- paired terminal-score share is at least 50%, with family combat kills and
  cities captured reported as descriptive guardrails.

A pass permits only a separate gameplay-integration PR with the normal full
promotion test; it does not flip a default here. A failure retains the stock
controller and the negative result. There is no pooled rescue, seed retry,
sample extension, threshold fitting, controller substitution, or post-result
treatment change.

For either phase, a ratio whose control denominator is zero passes only when
the corresponding treatment count is also zero; it cannot pass by treating an
undefined ratio as zero.

The six-job batches join the shared-host simulator queue behind the active
Strategic Expansion oracle, the Spaceport evaluator, and the deployment
horizon evaluator. They must not start while another process owns six or more
simulator cores.
