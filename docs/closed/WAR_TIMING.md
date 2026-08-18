# The midgame power-spike appointment

Status: **v1, selective v2, and ready-force v3 are implemented as default-off
evaluator arms and were rejected by their frozen 8-player outcome screens;
production `advanced` remains unchanged**.

The implementation is one controller-owned `WarPlan`, not a collection of
bonuses. Its lifecycle is validated before decisions and consumed by research,
production, exact upgrade pricing, discretionary-spend floors, package-only
staging, diplomacy, declaration, tactical finishing, the reasoning journal,
spectator JSON/HUD, and `ai_eval`. Twelve focused deterministic tests cover the
frozen mechanism contract, including the typed evaluator boundary and the
minor-controller exclusion; a separate server contract test carries the plan
through JSON into both browser renderers. The frozen v1 screen below rejected
the broad treatment despite proving that the complete lifecycle executes. The
selective v2 amendment preserves the same controller-owned plan and executor
but sharply limits when an appointment may be made and strengthens its launch
gate.

This is the next military experiment after the ancient-rush line was retired
on the live Continents/Planet cell. It is deliberately not another ancient
selector. The intervention begins after that window and asks whether a major
civilization can make one war a single, persistent appointment shared by its
target selection, research, production, treasury, diplomacy, movement, and
tactical execution.

## What is already known

The current agent does one important thing correctly. `war_census` observed
98% of its wars opening with a force already near an enemy city, with the peak
force arriving only 8.1 turns later. The defect is not simply "declare, then
build". The same wars opened at a mean **11.5x empire-wide military advantage**,
while the readiness gate was satisfied 8.7x over. The agent waits for a
walkover; it does not create a temporary advantage on purpose.

The ancient-rush experiments proved that a deliberately timed campaign can be
made operational: choose one victim, raise a finish-capable floor, stage before
declaring, and keep reinforcements moving after the opening. They also proved
that doing this indiscriminately loses the live Science/Culture/Domination
race. The route-connected selector still exposed 94.2% of seats and won fewer
games despite building larger empires. There is no further ancient threshold
or map selector to fit.

The midgame gap is structural and spans several existing layers:

- `assess()` may change the campaign target every five turns;
- `advanced_research()` values military unlocks, but no chosen target owns one
  breakthrough and no build schedule depends on its arrival;
- an adaptive Conquest plan still sends ordinary unit production through the
  lightweight city governor, while the plan-aware evaluator is reserved for
  Recovery or an explicitly assigned victory;
- discretionary Gold spending happens before upgrades and protects only a
  generic reserve; and
- the staging gate knows the army's present strength, not whether its planned
  upgrade package has arrived.

That makes research, bodies, Gold, staging, and declaration five individually
reasonable decisions with no common deadline. The treatment below supplies the
deadline. It does not loosen combat odds globally and it does not declare from
marching distance.

## Frozen treatment

The treatment is `advanced_timing_attack`. It is identical to `advanced`
except for one opt-in `timed_war` capability. The capability lives inside
`AdvancedAi`, so the same implementation is inherited by weighted/evolved
agents, `StrategicAi`, production rollout agents, and `PolicyAi`'s scripted
fallback. City-states and barbarians remain on `BasicAi`; `RandomAi` remains a
random control. No minor civilization starts an elective power-spike war.

The state is an inspectable `WarPlan`, separate from the five-turn
`StrategicPlan`:

- target player and objective city;
- breakthrough technology and assault unit;
- the predecessor body, when the assault unit has a direct upgrade path;
- required assault bodies and breach/support requirement;
- estimated research, production, upgrade, and march costs;
- phase (`research`, `mobilize`, `stage`, `strike`, or `exploit`); and
- the turn on which the appointment was made.

The plan is reported through `PlanReport` and the reasoning journal. An
experiment must be able to distinguish a plan that was formed, one whose
technology arrived, one that mobilized, and one that actually declared. A
terminal snapshot saying only `Conquest` is not exposure evidence.

### 1. Select a target and the minimum excellent unlock together

Plans may be formed only in peace, after the ancient window closes, with at
least two cities, no materially threatened home city, no friendship/alliance
with the candidate, and at least 40 Standard-speed turns left. Existing wars,
emergencies, explicit victory-denial targets, and the Byzantine Tagma timing
retain their current priority and are not reclassified as this treatment.

The target set is living major civilizations legal under
`campaign_target_legal`. A candidate objective must have a real route for a
current land melee unit to the existing 3-tile staging ring; wrapped or
spherical graph distance alone is not enough. The objective city continues to
use the existing campaign-city valuation, including capitals, defenses,
loyalty, distance, yields, and victory pressure.

For each target, inspect land military units legal for this civilization whose
unlock is an unowned technology. An assault candidate must:

1. be melee-capable, because the package must be able to capture a city;
2. improve by at least 8 Combat Strength over its direct, currently unlocked
   predecessor, or over the strongest currently trainable melee body when no
   predecessor exists;
3. have expected full-health damage of at least 36 against the harder of the
   objective city's current Combat Strength and the strongest target field
   unit within six tiles, using the engine's own mean damage curve; and
4. be materially feasible: a resource-free unit qualifies immediately; a
   resource unit qualifies only with a connected source or enough current
   stock plus deterministic accumulation to supply the package by launch.

Thirty-six damage is the first ordinary roll produced by roughly +5 Combat
Strength. Eight strength is the smallest stock one-generation jump that is
large enough to change the matchup rather than merely rename a body. These are
fixed mechanism thresholds, not values to sweep.

For every qualifying unit, remaining research cost is the sum of its unowned
ancestor nodes and unlock node, crediting current progress only when the
current node lies on that path. Convert it to turns with current Science per
turn. Production turns use the empire's current city Production and count
existing, queued, and directly upgradeable predecessor bodies. March turns use
the slowest required body's movement and the real staging route.

The appointment uses the **least remaining Science path** that meets the
excellent-unit test. Ties prefer the earlier launch estimate, then higher
expected damage, then stable unit/target IDs. Target selection minimizes the
existing campaign cost plus the estimated turns to research, mobilize, and
march; it therefore chooses the best *attack window*, not simply the weakest
nameplate. No leader or civilization name receives a target bonus.

If no target/unlock pair qualifies, the treatment is exactly ordinary
`advanced` for that assessment. It may try again after the ordinary five-turn
planning cadence. Once a pair is chosen, target and unlock are locked until a
declared invalidation below; ordinary plan churn cannot retarget the column.

### 2. Build bodies before the breakthrough, preserve the upgrade

The package asks for four assault bodies. This is not re-fitted from the
ancient result: four is the smallest force that can occupy most of a city ring
while leaving one healthy melee body for the capture. Existing and queued
assault units count. Direct predecessors count while research is unfinished,
and plan-aware production values them ahead of surplus infrastructure. Once
the unlock arrives, only the assault unit itself fills a missing body.

If the objective has standing walls, the package additionally requires one
currently compatible ram/tower or one trainable siege unit whose Bombard
Strength is at least the city's Combat Strength. The existing wall-era rules
decide whether a ram or tower is compatible; this treatment does not waive
them. An unwalled target adds no invented siege requirement.

Adaptive Conquest production is routed through the same plan-aware evaluator
already used by explicit Domination and Recovery. The ordinary governor still
runs afterwards for cities whose queue the strategic pass did not change. The
power-spike floor is four land assault bodies plus the real breach requirement,
not `mil_per_city * city_count`, and disappears when the plan ends.

Before the breakthrough, the treasury reserves the sum of currently quoted
Gold upgrade prices for all planned predecessors plus the ordinary peacetime
reserve. Strategic Gold purchases, plot annexation, deals, and patronage may
spend only above it. The turn the technology arrives, upgrades execute before
discretionary spending and before movement. A predecessor outside friendly
territory routes home during `mobilize`; it does not march toward a war it
cannot modernize for.

### 3. Stage without warning, then strike as one package

The plan uses the existing peacetime 3-to-5-tile staging ring and never enters
the target's territory before war. Formal-War denouncement may run while the
army mobilizes. The declaration remains withheld until all of these are true:

- the breakthrough technology is owned;
- four assault bodies are the planned unit or a stronger successor;
- the breach requirement, if any, is present;
- at least three assault bodies are on the staging ring and the fourth can
  reach it in one turn;
- the local-strength calculation at the objective is at least 1.0; and
- no home city has triggered Recovery.

The current urgent-victory interrupt may still open immediately; it is not a
power-spike appointment and is counted separately. Otherwise the preferred
legal Formal/Protectorate/Casus Belli opening remains in force. If the
denouncement timer is the only blocker, the staged force holds rather than
declaring a Surprise War.

After declaration the plan enters `exploit` and pins the campaign *city* until
it falls, while the tactical focus-fire solver may still clear defenders in
the way. It does not repeat the rejected ancient intervention that forced every
unit to ignore nearby defenders. Ranged and siege units act before melee as
today; at least one healthy melee unit is preserved from a non-forcing attack
when the objective is projected to become capturable later in the same turn.
The objective is then reassessed immediately after capture rather than five
turns later. The quick-strike endpoint is time to first target city, not a
promise that every selected war must eliminate a whole civilization.

### 4. Invalidation is explicit

A plan ends without declaration if the target dies, the objective changes
owner, friendship/alliance makes war illegal, the resource package becomes
impossible, the projected launch moves past the 40-turn remaining horizon, or
a home Recovery alarm persists for two assessments. It ends after declaration
when peace closes the war or no legal target city remains. A single temporary
route obstruction does not erase it. Every invalidation is journaled with one
of these reasons and drops the production and Gold reserves immediately.

## Required deterministic tests

Implementation is incomplete until focused tests prove all of the following:

1. the chooser selects the least-research qualifying unlock, not the strongest
   end-tree unit;
2. a civilization's unique replacement participates in both quality and
   upgrade calculations;
3. an unavailable strategic resource rejects an otherwise attractive unit;
4. target and technology persist across ordinary five-turn reassessments;
5. research follows the full prerequisite path and credits in-progress work;
6. production counts existing/queued predecessors, raises the four-body floor,
   and adaptive Conquest actually reaches the plan-aware path;
7. the exact quoted upgrade bill survives discretionary Gold spending, then
   upgrades execute before movement;
8. a walled objective requires compatible breach capability while an unwalled
   one does not;
9. the agent stages but does not declare with an incomplete or obsolete
   package, and declares once every frozen gate is met;
10. a deterministic tactical fixture uses ranged/siege setup and a reserved
    melee finisher to capture the appointed city in the same turn;
11. each invalidation clears the military floor and treasury reserve; and
12. `advanced`, `advanced_timing_attack`, evolved/Strategic construction, and
    the public plan JSON report the intended off/on/inherited behavior without
    changing minor or barbarian controllers.

The focused suite is followed by `cargo test --profile ci --locked` and the
engine soak required by `CONTRIBUTING.md`.

## Frozen v1 live-profile screen

The first outcome read was the following paired policy screen on fresh maps:

```text
ai_eval advanced_timing_attack advanced --players 8 --width 84 --height 54 \
  --city-states 12 --pairs 60 --turns 250 --speed online \
  --map continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --seed 10100000 --jobs 6
```

The evaluator reports treatment-only lifecycle diagnostics:
seat-games with a plan; plans reaching breakthrough/mobilize/declaration;
median appointment-to-tech, tech-to-declaration, and declaration-to-first
objective capture; declarations with a complete modern package; objectives
captured within 10 turns; abort reasons; and treatment player-turn exposure.
These are mechanism diagnostics and never replace wins.

The treatment could advance only if every term passed:

- 15% to 75% of treatment seat-games form a power-spike appointment;
- at least 60% of its elective declarations carry the complete modern package;
- at least 35% of declared appointments capture their objective within 10
  turns, with at least 12 declared appointments in the denominator;
- paired win score is at least 52%;
- favorable map directions outnumber adverse directions;
- paired terminal-score share is at least 50%; and
- the repository's unchanged promotion gate does not retain `advanced`.

### Frozen v1 result: reject universal mobilization

The completed screen ran all 60 pairs (120 games, average 232.8 turns). It
decisively retained `advanced`: the timing arm won 12 games (10.0%) against 82
(68.3%), produced a 20.8% paired score (95% Wilson interval 12.5%..32.7%), and
had 2 favorable, 13 neutral, and 45 adverse map directions. Its terminal-score
share was 39.2%, with all 60 maps adverse on that measure. The promotion gate
crossed to `RETAIN advanced` at map 20.

The mechanism itself was active, not inert. Treatment seats formed plans in
469 of 480 seat-games (97.7%) and spent 53.0% of their player-turns in an
active appointment. There were 1,029 appointments, 919 breakthroughs, 1,043
mobilizations, and 417 declarations; every declaration carried the complete
package. There were 115 objective captures, including 100 within ten turns of
declaration (24.0% of declared appointments), with median appointment-to-tech,
tech-to-declaration, and declaration-to-capture times of 10, 26, and 5 turns.

That is a useful negative result. The unified machinery reliably aligned
research, production, treasury, staging, diplomacy, and combat, but making it
available to nearly every seat and repeatedly remobilizing damaged economies
was strategically destructive. Treatment empires finished with lower mean
cities (7.63 vs 9.81), population (67.5 vs 103.5), technology (57.0 vs 63.8),
civics (37.8 vs 46.6), and military strength (721.5 vs 1,370.2). V1 remains a
reproducible evaluator arm and stays off by default.

### Reduced implementation preflight (not a promotion read)

After the focused tests, a two-map, 4-player, 44x28 Online smoke run used the
same Continents/Planet/Poles rules but not the frozen 8-player 84x54 cell. It
exists only to prove that the lifecycle reaches real games and that observer
telemetry agrees with it. The final pass, rerun after merging current `main`,
reported 11 appointments, 9 breakthroughs, 10 mobilizations, and 7
declarations; all 7 declarations carried the complete package. Two objectives
fell, both within 10 turns (median 4.0),
and no appointed campaign accepted generic peace before its objective fell.
Paired score was 50.0% and terminal-score share 48.0%, both unresolved at two
maps. All eight treatment seats formed a plan, outside the preregistered
15%..75% promotion band; this reduced profile therefore supplies mechanism
evidence, not permission to promote or retune. Production `advanced` remains
unchanged and the full command above remains the first strength gate.

The exposure band prevents a nearly inert arm or a renamed universal-Conquest
arm from advancing. The two mechanism rates are absolute capability gates,
not comparisons fitted to the control, which has no corresponding appointment
state. If fewer than 12 plans declare, the capability claim is unresolved and
the screen stops.

## Selective v2 amendment (preregistered before its focal seeds)

V2 is a new default-off evaluator arm named
`advanced_timing_attack_selective`. It reuses the v1 `WarPlan`, lifecycle,
research path, exact production and upgrade accounting, invalidations,
diplomacy, staging movement, tactical executor, and observer telemetry. It is
not a second war system. Before reading any v2 focal seed, the only mechanism
changes are frozen as follows:

1. A civilization may make at most one v2 appointment in a game.
2. Its ordinary, unmodified strategic assessment must already be `Conquest`
   with a living target player. The appointment chooser may consider only that
   same target; the treatment cannot manufacture a conquest strategy or swap
   in an easier victim.
3. At appointment time, at least three of the four assault bodies must already
   exist or be queued as the chosen assault unit, a stronger successor, or its
   direct upgradeable predecessor. V2 may finish and modernize a real army; it
   may not redirect an empire into building a strike force from scratch.
4. Declaration requires all four modern assault bodies on the legal staging
   ring, rather than three staged with the fourth one turn away.
5. Local friendly-to-hostile strength at the objective must be at least 1.25,
   rather than v1's 1.0. The breakthrough, compatible breach capability,
   legal-war, and home-safety gates remain mandatory.

These are fixed causal changes aimed at the two v1 failures: excessive
exposure/economic displacement and an underpowered opening. There is no
leader-name rule, new combat bonus, changed tactical odds, threshold sweep, or
seed-specific exception. The v1 arm remains frozen for reproduction.

### Frozen v2 screen

The first v2 outcome read uses disjoint maps and otherwise the same live
profile:

```text
ai_eval advanced_timing_attack_selective advanced --players 8 \
  --width 84 --height 54 --city-states 12 --pairs 60 --turns 250 \
  --speed online --map continents --shape planet --poles poles \
  --randomize-civs --victories science,culture,domination \
  --seed 10130000 --jobs 6
```

V2 advances only if every original screen term passes: 15%..75% treatment
seat-game exposure, at least 60% complete-package declarations, at least 35%
ten-turn captures with 12 or more declarations, paired score at least 52%,
more favorable than adverse map directions, terminal-score share at least
50%, and no `RETAIN advanced` promotion decision. Fewer than 12 declarations
is unresolved, not a pass. This is one frozen read; failure is reported and
keeps the treatment default-off.

### Frozen v2 result: materially safer, still reject

The completed screen ran all 60 pairs (120 games, average 226.9 turns). V2 was
far less destructive than v1 but did not earn promotion: it won 51 games
(42.5%) against 58 (48.3%), produced a 47.1% paired score (95% Wilson interval
35.0%..59.5%, Elo-equivalent -20 with interval -107..+67), and had 9 favorable,
37 neutral, and 14 adverse map directions. The promotion gate was
`INCONCLUSIVE`. Terminal-score share was 48.0%, with 12 favorable and 48
adverse directions.

The selectivity mechanism worked as specified. Plans formed in 258 of 480
treatment seat-games (53.8%) and were active for 15.5% of observed treatment
player-turns. All 82 declarations carried the complete modern package. Thirty
objectives fell, 25 within ten turns: 30.5% of declarations, below the frozen
35% threshold. Median appointment-to-tech, tech-to-declaration, and
declaration-to-capture times were 10, 25, and 5 turns.

V2 therefore passed exposure, declaration count, and package completeness but
failed quick capture, paired score, favorable-over-adverse direction, and
terminal-score gates. The residual cost was much smaller than v1 but remained
visible: treatment empires averaged 8.65 cities, 90.0 population, and 1,079.5
military strength against 8.87, 95.6, and 1,226.9 for `advanced`. The largest
pre-declaration aborts were launch horizon (56), persistent home Recovery (53),
and objective ownership changing (50). V2 remains reproducible and default-off.

## Ready-force v3 amendment (preregistered before its focal seeds)

V3 is the default-off `advanced_timing_attack_rapid` arm. It is v2 with one
additional appointment filter, still inside the same `WarPlan` and the same
research, treasury, staging, diplomacy, tactical, invalidation, and observer
paths:

1. All four assault bodies must already exist or be queued as the selected
   assault unit, a stronger successor, or its direct upgradeable predecessor.
2. If the objective is walled, the compatible breach unit must also already
   exist or be queued; merely being trainable is insufficient.
3. The chooser's existing research + production + real-route march estimate
   must be no more than 30 Standard-speed turns, which is 15 turns on the live
   Online profile.

Every v2 constraint remains: one appointment, ordinary Conquest intent and
the same victim, all four modern bodies staged, 1.25 local strength, legal war,
and a safe homeland. V3 changes no combat odds and grants no production,
research, movement, or Gold bonus. It tests one causal response to v2's
remaining loss: appoint only a force that can plausibly strike before the
target changes owner or a long mobilization taxes the empire. There is no
threshold sweep or reuse of v1/v2 focal maps.

### Frozen v3 screen

```text
ai_eval advanced_timing_attack_rapid advanced --players 8 \
  --width 84 --height 54 --city-states 12 --pairs 60 --turns 250 \
  --speed online --map continents --shape planet --poles poles \
  --randomize-civs --victories science,culture,domination \
  --seed 10160000 --jobs 6
```

V3 uses the same all-terms screen: 15%..75% seat-game exposure, at least 60%
complete-package declarations, at least 35% ten-turn captures with 12 or more
declarations, paired score at least 52%, more favorable than adverse maps,
terminal-score share at least 50%, and no `RETAIN advanced` decision. Failure
is reported and remains default-off.

### Frozen v3 result: closer on wins, still reject

The completed screen ran all 60 pairs (120 games, average 224.3 turns). V3 won
57 games (47.5%) against 54 (45.0%), produced a 51.2% paired score (95% Wilson
interval 38.9%..63.4%, Elo-equivalent +9 with interval -78..+96), and had 11
favorable, 40 neutral, and 9 adverse map directions. The unchanged promotion
gate was `INCONCLUSIVE`. Terminal-score share was 49.1%, with 16 favorable, 5
neutral, and 39 adverse directions.

The ready-force filter produced meaningful but selective exposure: 153 of 480
treatment seat-games formed a plan (31.9%), and appointments were active for
7.6% of observed treatment player-turns. There were 153 appointments, 134
breakthroughs, 175 mobilizations, and 52 declarations; every declaration
carried the complete package. Seventeen objectives fell, including 13 within
ten turns of declaration (25.0%), with median appointment-to-tech,
tech-to-declaration, and declaration-to-capture times of 9.5, 25, and 6 turns.

V3 passed the exposure, declaration-count, complete-package, and favorable-map
direction terms. It failed the preregistered 52% paired-score floor, 35%
ten-turn capture floor, and 50% terminal-score floor. Treatment also finished
behind `advanced` in mean cities (8.70 vs 8.79), population (92.0 vs 94.8),
military strength (1,161.9 vs 1,260.1), and terminal score (669.8 vs 695.0).
The disjoint holdout and strongest-controller transfer were therefore not run.
V3 remains reproducible and default-off.

## Disjoint v3 holdout and strongest-controller transfer (not run)

Passing every v3 screen term earns one unchanged 240-map holdout at seed
10,170,000
on the same profile. Coverage must remain in 15%..75%, complete-package
declarations at or above 60%, ten-turn captures at or above 35% with at least
30 declarations, terminal-score share at or above 50%, favorable directions
above adverse, and the unchanged win gate must say `PROMOTE`.

Only that result enables selective timing appointments by default in
`AdvancedAi`. Because the
implementation sits below the wrappers, defaulting it transfers the behavior
mechanically to evolved, Strategic, production-rollout, and Policy-fallback
agents; it does **not** establish strength transfer. One final 60-map screen
then compares a named `strategic_deep_timing_selective` entrant with the
published `strategic_deep` on seed 10,180,000 and the identical live profile.
It must load the same embedded genome/value artifact provenance in both arms. The
strongest controller keeps the default only if the treatment forms at least
12 appointments, favorable directions are not fewer than adverse, terminal
score is at least 50%, and the unchanged promotion gate does not retain the
control. A failure leaves the capability reachable as an entrant but off in
that constructor.

There is no threshold sweep, seed retry, pooled rescue, target-name rule, or
ancient-rush fallback. A screen failure may be diagnosed from the frozen phase
counts and abort reasons, but any materially changed mechanism requires a new
preregistration and new seeds.

## README completion rule

The core `README.md` is updated in the implementation/decision PR, not in this
preregistration. It will explain the appointment model across target selection,
minimum excellent technology, prebuild/upgrade budget, staging, and strike;
name which AI ladder rungs inherit it; and report the actual screen/holdout
decision, including a negative result. The README must not describe an eval-only
entrant as shipped default behavior or call a mechanism rate a strength gain.
