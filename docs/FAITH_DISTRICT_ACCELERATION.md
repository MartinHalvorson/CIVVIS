# Preregistered Faith district acceleration evaluation

Status: frozen before evaluator implementation and before any registered seed
is run. This task adds an evaluator only; it does not change the engine or a
shipped AI policy.

## Question

Moksha's Divine Architect promotion makes an already legal district available
for immediate Faith purchase. Advanced AI values Gold district purchases with
its full production evaluator, but rejects every non-Gold candidate in that
pass. Its Faith pass considers buildings only. This study asks the narrowest
causal question that does not invent a second district scorer:

> When the stock controller itself chooses to start a district this turn,
> does paying real surplus Faith to complete that exact district and placement
> immediately improve terminal strategic strength without harming wins?

The evaluator uses an untreated one-turn oracle only to name an action stock
would take. The real treatment then makes the legal purchase **before** running
the actual stateful controller, which replans normally from the changed world.
The treatment cannot choose a district stock did not choose.

## Frozen observational evidence

PR #600 froze and ran a read-only latest-50 production-save census at source
commit `5d81637`, through
`20260729T161608.260324Z-seed-416573342-turn-302-instance-39983.save.json`.
All 50 files parsed. Across 376 living major civilizations, mean terminal Faith
was 4,486.5 and median Faith was 3,824.6; 258 seats held at least 2,000 Faith.

After removing only terminal winner/turn-owner guards on private clones, 47 of
those 258 rich seats (18.2%) had a legal Faith `BuyDistrict`; among the 138
seats holding at least 5,000 Faith, 40 (29.0%) did. This is descriptive
availability, not evidence that stock previously skipped the purchase or that
the purchase helps.

That census also replicated a different, already-stopped line: Naturalist or
Rock Band purchases were legal for 249 of 258 rich seats. The registered
cross-plan Culture-asset treatment spent Faith successfully but failed its
development gate. This study may not revive, tune, or reinterpret that policy.
It tests only districts that stock independently selected for production.

Before this registration, source inspection established the exact policy gap:

- `advanced_gold_spending` converts legal unit, building, and district actions
  into `Item`s and calls the shared strategic `production_value`, but then
  rejects `currency != "gold"`;
- `faith_building_spending` accepts only `BuyBuilding`; and
- the engine offers and executes `BuyDistrict { currency: "faith" }` only in a
  city with an established `faith_purchase_districts` effect, at the ordinary
  speed-scaled placement cost times four.

No focal seed or treatment outcome was read.

## Exact source mechanism and treatment

For every focal comparison turn, including the null arm, the wrapper first
clones the current `Game` and `AdvancedAi`, runs one complete untreated stock
turn, and reads only the clone's successful action log and final public
`PlanReport`. The clone and cloned controller are then discarded.

The active treatment can act only when all of the following hold:

1. the focal game is live and still owns the turn;
2. at least 20 external observation turns remain;
3. `turn % 6 == focal_seat % 6`;
4. the untreated oracle did not end the game;
5. the oracle successfully logged `Produce { city, Item::District { district,
   pos } }`; and
6. the unchanged pre-turn game offers the exact corresponding legal
   `BuyDistrict { city, district, pos, currency: "faith" }`.

For every exact match, the evaluator applies the candidate on a private clone
to measure its real Faith price, computes projected production turns saved as

```text
item_remaining_cost_for_city / max(city_production, 1)
```

and requires the focal bank after purchase to retain the stock Faith-building
reserve associated with the oracle's reported strategy:

- Religion: 180 Faith;
- Culture with a current National Park site: the live Naturalist purchase
  cost;
- Culture after Cold War and without such a site: 700 Faith; and
- every other plan: 80 Faith.

The candidate with most projected production turns saved wins; ties are lowest
city id, district id, then map position. The evaluator applies exactly one
ordinary Faith `BuyDistrict` to the real pre-turn game. It then invokes the
real focal `AdvancedAi` from its original state on that changed game. Thus the
controller reassesses its plan, purchases, production, commands, and internal
state normally; no untreated action log is replayed into the divergent world.

The treatment spends real Faith, uses the engine's real cost and placement,
consumes ordinary district capacity, grants no title/promotion/technology,
does not clear a queue or foundation, does not change yields or rules, and
does not suppress any subsequent stock action. It can indirectly displace a
Faith building, unit, or Great Person purchase because the treasury is lower.
That opportunity cost is part of the causal question and is guarded by wins
and strategic endpoints.

## Exact null and focused contract

Before treatment data, a four-map null at seed `10039999` runs the same
untreated-oracle wrapper with purchase disabled. For both focal seats on all
four maps, direct stock and null wrapper must have identical focal results,
census, controller `PlanReport`, and serialized terminal `Game`: eight exact
cells or **STOP**.

Focused tests must prove:

- the deployment schedule and profile-override rejection are exact;
- a synthetic Faith action does not qualify: the engine must offer an exact
  pre-turn Faith `BuyDistrict` matching a successful oracle `Produce` action;
- Gold district purchases, different cities/districts/positions, failed stock
  actions, an oracle win, off-cadence turns, and the final 20 turns cannot act;
- the strategy reserve is exact, including live Naturalist cost precedence;
- choice order is projected turns saved, then stable city/district/position;
- a purchase deducts the exact engine price, completes the exact district,
  leaves the queue untouched, and occurs at most once per focal turn;
- the real controller, not the discarded oracle controller, takes the treated
  turn and its subsequent actions are recorded;
- the null wrapper preserves serialized game and stateful controller output;
  and
- screen and holdout gates reject every individual missing-mechanism or harm
  condition.

## Deployment population and endpoints

The evaluator uses the same deterministic 126-profile unattended-production
cycle as the registered Spaceport, horizon, recon, repair, captive-Spy, and
Pingala studies. For zero-based map offset `i`:

- players are `[4, 6, 8, 10, 5, 7, 9][i mod 7]`;
- scripts are `[land_only, water_world, continents, true_start_earth, lakes,
  inland_sea, pangaea, small_continents, islands][i mod 9]`; and
- topology is Flat for even `i`, Planet for odd `i`.

`MapSize::for_players` supplies dimensions and city-state counts.
Civilizations are randomized; Poles, Online speed, and
Science/Culture/Domination victories are fixed. `Game.max_turns` remains 250
while unchanged stateful agents are observed externally through turn 320; the
runner asserts that policy-visible horizon never changes.

Each map is played four times: focal seat 0 and the final major seat, each as
direct stock and comparison. All other majors are stock `AdvancedAi`; minors
retain their stock paths. The two focal seats are aggregated inside their map,
and the map is the only inference unit.

The evaluator reports:

- cadence turns, stock district intentions, exact legal matches, affordable
  candidates, successful/failed purchases, treatment seat-game coverage,
  Faith spent, projected turns saved, and district-family counts;
- the real controller's same-turn actions after the purchase, including new
  production in the purchased city and Faith actions that still execute;
- terminal Faith, district/building totals, purchased-district survival,
  focal-city Science/Culture/Production, Great People claimed, low-Loyalty
  cities, and lost original capitals;
- wins and victory types, finish turn, score, cities, technologies, civics,
  Science-project progress, lifetime Culture/Tourism, and military power; and
- paired map win score, terminal-score and Science-progress shares, complete
  score favorable/neutral/adverse directions, and an exact two-sided sign
  test.

## Fixed development screen

After the exact null, the one allowed screen is 30 maps / 120 games:

```text
faith_district_acceleration_eval --deployment-mix --maps 30 --turns 250 \
  --observe-through 320 --speed online --poles poles --randomize-civs \
  --victories science,culture,domination --seed 10040000 --jobs 6
```

It advances only if every term holds:

1. treatment completes at least eight Faith district purchases across at
   least six of 60 focal treatment seat-games, with zero failed applications;
2. the real controller completes a turn after all eight purchases, and at
   least six purchased districts survive in focal ownership to the terminal
   observation;
3. treatment has at least as many terminal focal districts, no more sub-70-
   Loyalty cities, and no more lost focal capitals than control;
4. score-favorable map directions outnumber adverse directions, exact
   two-sided sign-test p-value is at most 0.20, paired terminal-score share is
   at least 50%, and mean treatment score is not lower;
5. paired Science-progress share and mean Science progress are each at least
   control, paired map win score is at least 50%, and treatment has no fewer
   total focal wins; and
6. treatment has no fewer Science, Culture, or Domination wins than control.

Any failed term means **STOP**: retain stock, record the negative or
underexposed result, do not broaden the candidate set, change reserve/cadence,
alter the gate/sample/seed, and do not inspect the holdout.

## Disjoint holdout

A complete screen pass earns one unchanged 63-map holdout at seed `10041000`
(252 games). It must retain all mechanism and safety gates, paired terminal-
score and Science-progress shares of at least 50.5%, positive mean score and
Science-progress differences, more favorable than adverse score maps with
exact two-sided sign-test `p < 0.05`, paired map win score at least 50%, and no
loss of total or per-type wins. Only that conjunction permits a separate
gameplay-integration PR; this evaluator cannot ship the policy.

Undefined ratios pass only when both corresponding counts are zero. No pooled
rescue, seed retry, sample extension, subgroup promotion, or post-result
treatment change is allowed.

## Resource and integration order

The exact null, screen, and any earned holdout use at most six jobs and run
alone in the shared simulator slot. They are queued behind every older active
registered job, including #561, #567, #570, #574, #579, #584, #589, #592,
#593, #597, #598, #599, #600, #601, #602, and #603. Studies that stop or land
release their place; this task never jumps a still-live older batch.

The #600 census must first pin its qualifying Faith-district path end to end.
This implementation, a latest-main merge, focused checks, and the full locked
CI suite must then precede the exact null. Exact commands, source commit, wall
time, and results will be recorded before this evaluator leaves draft.
