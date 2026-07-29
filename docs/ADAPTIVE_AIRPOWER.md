# Adaptive Conquest airpower production

Status: **preregistered before evaluator implementation; no registered seed has
been run or read**.

## Observation

A read-only census of the 26 most recent completed production games available
on 2026-07-29, through
`20260729T185423.384611Z-seed-979034451-turn-227-instance-69920`, found:

- 208 major-seat games reached a terminal save;
- 168 major seats had researched Flight and 151 had researched Advanced
  Flight;
- 145 of those Advanced-Flight seats still held a positive Aluminum stockpile
  at the terminal boundary;
- every one of the 26 games contained at least one Flight-qualified major;
- the same majors trained 218 late land units from the Tank, Modern Armor,
  Helicopter, Mechanized Infantry, Rocket Artillery, and Giant Death Robot
  families; but
- **zero aircraft were ever trained, zero aircraft survived, and only two
  Aerodromes existed across all 26 worlds**.

The saves preserve `trained:*` counters, so zero training is stronger than a
terminal roster census: it is not explained by aircraft being built and later
destroyed. The positive Aluminum stocks and late land-unit production also
make technology, strategic material, and generic late-game production
insufficient explanations. These are observational facts, not evidence that
building aircraft improves outcomes.

The source supplies a specific decision-path hypothesis. `production_value`
contains plan-aware Aerodrome and aircraft scores, and the advanced unit pass
contains exact air-strike, air-pillage, patrol, interception, and rebasing
logic. Yet an untargeted adaptive agent calls `advanced_production` only for a
Recovery plan. Its ordinary Conquest, Science, Culture, Expansion, and
Diplomacy plans hand empty queues to `BasicAi::cities`; that governor's four
district priorities omit Aerodromes, and its combined-arms selector excludes
the air domain. Explicit victory-target tests can therefore exercise an
airpower policy that the production league's untargeted adaptive agents cannot
normally enter.

## Question and frozen hypothesis

> When an adaptive champion reports a live Conquest plan, has Flight, and can
> pay every ordinary cost, does committing one city to a single two-aircraft
> wing improve matched whole-game performance?

The hypothesis is that an Aerodrome plus a bounded air wing unlocks the
already-shipped operational planner and improves the champion's broad
win/score fitness and territorial progress. The null is that the real district,
Production, maintenance, and strategic-resource opportunity costs are at least
as valuable as that capability.

This is deliberately a bundled end-to-end policy test. It estimates one honest
airbase-and-aircraft commitment, not the isolated value of an Aerodrome, an
aircraft price discount, a resource grant, or perfect air tactics.

## Frozen controller and population

Every major in both arms uses the repository-embedded
`advanced_evolved` champion from `data/evolved/best.json`, generation 14, with
FNV-1a fingerprint `0x40b1fbb2a5b88bc6`. The evaluator must compile that JSON
into the binary, assert both identifiers at startup, and never load a mutable
working-directory `evolved/` artifact. City-states and barbarians retain the
controller's ordinary minor/barbarian behavior.

The fixed deployment population is the unattended supervisor's deterministic
space-filling cycle over its 7 player counts, 9 map scripts, and 2 topologies.
For zero-based map offset `i`:

- players are `[4, 6, 8, 10, 5, 7, 9][i mod 7]`;
- scripts are `[land_only, water_world, continents, true_start_earth, lakes,
  inland_sea, pangaea, small_continents, islands][i mod 9]`; and
- topology is Flat at even `i` and Planet at odd `i`.

The three cycle lengths are pairwise coprime, so all 126 joint profiles are
unique before repetition. Requested dimensions and city-state counts must come
from `MapSize::for_players`; the evaluator may not maintain a second size
table. Each phase restarts at offset zero. Civilizations are randomized by the
map seed. Every map uses Poles, Online speed, Science/Culture/Domination
victories, and resolved Prince difficulty.

`Game.max_turns` remains exactly 250 because production-value deadlines read
that field. Score victory stays disabled and the unchanged world and stateful
controllers are observed externally through turn 320. The evaluator must
assert that `max_turns` stays 250 throughout continuation.

## Frozen intervention

For a focal treatment turn, run the authoritative embedded champion on a clone,
retain the clone's resulting controller state, and replay every successful
logged action on the authoritative game except the final `EndTurn`. Any replay
failure invalidates the harness; it is never skipped. After the stock trace and
before the deferred `EndTurn`, the evaluator may issue **at most one** ordinary
legal `Produce` order.

No intervention is eligible unless the controller's post-trace `PlanReport`
still says `conquest` and the player has researched Flight. Existing and
first-queued Aerodromes are counted empire-wide by district family. Existing
and first-queued aircraft are counted from the rules-derived `domain == air`
family.

### Stage A: one airbase

If the empire owns or first-queues no Aerodrome, an airbase order additionally
requires at least one unlocked air unit whose ordinary one-time strategic
resource cost is covered by the current stockpile. Enumerate only legal
Aerodrome `Item::District` entries returned by the engine across owned cities.
Choose the entry with the fewest current completion turns, then highest city
Production, lowest city id, district name, and tile position as deterministic
ties. Apply exactly that `Produce` order, replacing the city's current first
queue while preserving the engine's normal paused-item progress.

The treatment never creates a district slot or site, ignores placement rules,
discounts the district, accelerates completion, grants a resource, or orders a
second treatment Aerodrome. A stock-built or captured Aerodrome satisfies this
stage without a treatment order.

### Stage B: one two-aircraft wing

Once an active owned Aerodrome exists, the treatment target is two committed
aircraft: living aircraft plus first-queued aircraft. If fewer than two exist,
enumerate ordinary legal `Item::Unit` air candidates from
`Game::producible_items` across owned cities. Rank them by highest
rules-reported ranged attack strength, then melee defense strength, lowest
completion turns, lowest city id, and unit name. Apply the best legal
`Produce` order. A city already first-queuing an aircraft is ineligible for
another order; the policy waits for that committed aircraft to complete rather
than replacing it while trying to commit the second. The engine pays and
commits the real Oil or Aluminum, later pays ordinary maintenance, and may
reject all candidates.

The two-unit cap is the unmodified two-slot capacity of the single treatment
Aerodrome. The evaluator does not order a Hangar, Airport, Airstrip, Carrier,
anti-air unit, formation, purchase, resource improvement, research choice,
policy, Governor, war, target, or tactical action. Once produced, aircraft are
controlled only by the shipped adaptive unit planner. If the live plan leaves
Conquest, existing commitments remain but no new treatment order is issued.

This post-trace splice is an evaluation proxy for a future narrow gameplay
integration. It pays the selected city's ordinary opportunity cost and adds no
same-turn replacement for the displaced stock queue.

## Experimental unit and outcomes

One independent observation is a map seed. Every map is replayed for focal
seat 0 and the final major seat, each once under direct stock control and once
under the replayed treatment. The four games share the exact world profile and
map seed. The two focal seats are averaged within their map and are never
treated as independent samples.

For each focal game define terminal major-score share as the focal Civ VI score
divided by the sum of all major scores in that world. Define the frozen broad
fitness in percentage points as:

```text
80 * terminal major-score share + 20 * won
```

The primary paired map effect is the mean treatment-minus-control fitness over
the two focal seats. Maps above, equal to, or below zero are favorable, neutral,
or adverse. Directional maps receive an exact two-sided sign test.

Also report:

- paired map win score, where equal map wins score 0.5 and each net treated
  focal win moves the score by 0.25;
- paired terminal raw-score share, `treatment / (control + treatment)`, averaged
  first by focal seat and then by map;
- total and victory-type wins, finish turn, score, major-score share, Science
  race progress, military power, city count, kills, and captures; and
- a terminal territorial index equal to foreign-owned cities plus nine extra
  points for each foreign original capital held, so an ordinary foreign city
  is worth one and a foreign original capital is worth ten.

The mechanism census reports Conquest/Flight/resource-ready turns, legal
airbase and aircraft opportunities, successful queue orders, completed and
queued Aerodromes, aircraft trained/surviving/queued by type, treatment games
that fired, real strategic material committed, and successful offensive
`AirStrike`/`AirPillage` actions. Rebase, patrol, and interception activity is
reported separately. Axis-specific summaries are descriptive only; only the
pooled frozen gate decides.

## Exact replay null

Before any treatment result, run exactly:

```sh
target/release/adaptive_airpower_eval \
  --null --maps 4 --deployment-mix --ai advanced_evolved \
  --turns 250 --observe-through 320 --speed online --poles poles \
  --randomize-civs --victories science,culture,domination \
  --seed 10050000 --jobs 6
```

The comparison actor uses the same clone-and-replay seam with the intervention
disabled. All eight matched seat cells must reproduce the direct stock world's
complete serialized `Game` and recorded result exactly. One mismatch stops the
study and leaves both treatment seeds unopened.

## Development screen

Only a passing exact null licenses this command:

```sh
target/release/adaptive_airpower_eval \
  --maps 30 --deployment-mix --ai advanced_evolved \
  --turns 250 --observe-through 320 --speed online --poles poles \
  --randomize-civs --victories science,culture,domination \
  --seed 10051000 --jobs 6
```

The screen advances only if every term holds:

1. treatment fires in at least 10% of the 60 focal seat-games;
2. at least six treatment Aerodrome orders, six completed aircraft, and six
   successful offensive air actions occur;
3. treatment completes more Aerodromes and trains more aircraft than control;
4. mean paired fitness delta is at least +0.25 percentage points per map;
5. favorable maps outnumber adverse maps and the exact two-sided fitness sign
   test has `p <= 0.20`;
6. treatment has at least as many total wins as control and paired map win score
   is at least 50%;
7. paired terminal raw-score share is at least 49.5%; and
8. mean paired territorial-index delta is nonnegative.

Failure means **STOP**. Do not tune the cap, eligibility, ranking, endpoint,
threshold, seed, or sample size, and do not inspect the holdout.

## Disjoint holdout

Only a passing screen earns this unchanged command:

```sh
target/release/adaptive_airpower_eval \
  --maps 120 --deployment-mix --ai advanced_evolved \
  --turns 250 --observe-through 320 --speed online --poles poles \
  --randomize-civs --victories science,culture,domination \
  --seed 10052000 --jobs 6
```

The holdout advances only if every term holds:

1. treatment again fires in at least 10% of its 240 focal seat-games;
2. at least 20 treatment aircraft complete and at least 20 successful offensive
   air actions occur;
3. treatment completes more Aerodromes and trains more aircraft than control;
4. mean paired fitness delta is positive;
5. favorable maps outnumber adverse maps with exact two-sided fitness sign-test
   `p < 0.05`;
6. treatment has at least as many total wins as control, paired map win score is
   at least 50%, and paired terminal raw-score share is at least 50%; and
7. mean paired territorial-index delta is nonnegative.

A pass permits only a separate gameplay-integration PR implementing this exact
one-base/two-aircraft policy. It does not change a shipped default here. A
failure retains stock gameplay and this negative result.

## Invocation integrity, chronology, and resource queue

The runner must fail with exit 2 before constructing a game when any supplied
numeric or text option is malformed or valueless. A formal label is available
only when every common option occurs exactly once with the canonical raw value:
`--deployment-mix`, `--ai advanced_evolved`, `--turns 250`,
`--observe-through 320`, `--speed online`, `--poles poles`,
`--randomize-civs`, `--victories science,culture,domination`, and `--jobs 6`.
The phase must also match its exact map count, seed, and `--null` presence.
`--difficulty` and explicit profile overrides are diagnostic conflicts and
cannot receive a frozen label.

The only permitted implementation smoke uses non-focal seed `10050999`, one
map, and at most one policy-visible turn; it is diagnostic only and cannot
spend a gate. No output from `10050000`, `10051000`, or `10052000` may be read
before this document, evaluator implementation, focused tests, exact commands,
and gates are committed and pushed.

This task owns only this document and
`src/bin/adaptive_airpower_eval.rs`. It changes no shipped AI, engine, rules,
artifact, or production runtime. Compilation and deterministic unit tests may
run locally with at most two build jobs. Every registered command is queued
behind the active Strategic Expansion oracle and every older registered
simulator owner; none may start while another process owns six or more
simulator cores. Latest `origin/main` is integrated once at the declared
pre-measurement boundary, not opportunistically during the blinded queue.
