# The midgame power-spike appointment

Status: **implementation checkpoint under semantic integration; no focal seed
has been read**.

## Implementation checkpoint

The opt-in treatment now carries one persistent `WarPlan` through target and
technology selection, prerequisite research, predecessor production, exact
upgrade reservation, formal-war preparation, route-aware staging, local
readiness, and same-turn ranged/siege setup plus melee capture. Its lifecycle
is observable independently from the ordinary five-turn strategy report, and
the evaluator folds each treatment seat's terminal lifecycle exactly once.

The deterministic mechanism suite covers every original obligation below,
including unique replacements, unavailable resources, quoted upgrade
protection across all major Gold sinks, compatible wall breach, every
preregistered invalidation, two-assessment Recovery, constructor inheritance,
public JSON, and a real-route rejection. The evaluator's lifecycle aggregation,
medians, and frozen gate classification tests also pass. Stable episode records
and the clarified raw target-ordering fixture below remain queued behind PR
#574's already-claimed assessment-observer seam. Full CI and the repository
soak remain pending until that semantic merge. The first focal command and all
thresholds below are unchanged and unread.

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
`StrategicPlan`. Every appointment receives a monotonically increasing,
seat-local episode ID so an abort followed by a replan cannot blend two
packages into one apparent declaration:

- episode ID;
- target player and objective city;
- breakthrough technology and assault unit;
- the predecessor body, when the assault unit has a direct upgrade path;
- required assault bodies and breach/support requirement;
- estimated research, production, upgrade, and march costs;
- phase (`research`, `mobilize`, `stage`, `strike`, or `exploit`); and
- the turn on which the appointment was made.

The plan is reported through `PlanReport` and the reasoning journal. Its
lifecycle report retains one record per episode, including formation,
breakthrough, mobilization, declaration, complete-package status, first
treated-seat capture, and abort/finish reason. Aggregate counts are folded from
those records rather than inferred from the final active plan. An experiment
must be able to distinguish a plan that was formed, one whose technology
arrived, one that mobilized, and one that actually declared. A terminal
snapshot saying only `Conquest` is not exposure evidence.

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

For each candidate target, the appointment first uses that target's **least
remaining Science path** that meets the excellent-unit test. Ties prefer the
earlier launch estimate, then higher expected damage, then the stable unit ID.
Target selection then minimizes one raw objective: the existing rival and city
campaign cost plus the unscaled estimated turns to research, produce the bodies
and breach package, and march. Ties use stable target and objective IDs. These
components are reported and pinned by a deterministic ordering fixture; they
are not normalized or reweighted after results. The selector therefore chooses
the best *attack window*, not simply the weakest nameplate. No leader or
civilization name receives a target bonus.

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

## Frozen live-profile screen

The first outcome read is a paired policy screen on fresh maps:

```text
ai_eval advanced_timing_attack advanced --players 8 --width 84 --height 54 \
  --city-states 12 --pairs 60 --turns 250 --speed online \
  --map continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --seed 10100000 --jobs 6
```

The evaluator must add treatment-only lifecycle diagnostics before this run,
folded once by stable episode ID: seat-games with a plan; plans reaching
breakthrough/mobilize/declaration;
median appointment-to-tech, tech-to-declaration, and declaration-to-first
objective capture; declarations with a complete modern package; objectives
captured within 10 turns; abort reasons; and treatment player-turn exposure.
These are mechanism diagnostics and never replace wins.

The capture endpoint is credited only when the treated seat owns the appointed
city after its own elective declaration. Third-party capture, liberation,
loyalty transfer, razing by another seat, or any other objective-owner change
does not count as a treatment capture.

The treatment advances only if every term passes:

- 15% to 75% of treatment seat-games form a power-spike appointment;
- at least 60% of its elective declarations carry the complete modern package;
- at least 35% of declared appointments capture their objective within 10
  turns, with at least 12 declared appointments in the denominator;
- paired win score is at least 52%;
- favorable map directions outnumber adverse directions;
- paired terminal-score share is at least 50%; and
- the repository's unchanged promotion gate does not retain `advanced`.

The exposure band prevents a nearly inert arm or a renamed universal-Conquest
arm from advancing. The two mechanism rates are absolute capability gates,
not comparisons fitted to the control, which has no corresponding appointment
state. If fewer than 12 plans declare, the capability claim is unresolved and
the screen stops.

## Disjoint holdout and strongest-controller transfer

Passing every screen term earns one unchanged 240-map holdout at seed 10,110,000
on the same profile. Coverage must remain in 15%..75%, complete-package
declarations at or above 60%, ten-turn captures at or above 35% with at least
30 declarations, terminal-score share at or above 50%, favorable directions
above adverse, and the unchanged win gate must say `PROMOTE`.

Only that result enables `timed_war` by default in `AdvancedAi`. Because the
implementation sits below the wrappers, defaulting it transfers the behavior
mechanically to evolved, Strategic, production-rollout, and Policy-fallback
agents; it does **not** establish strength transfer. One final 60-map screen
then compares a named `strategic_deep_timing` entrant with the published
`strategic_deep` on seed 10,120,000 and the identical live profile. It must
load the same embedded genome/value artifact provenance in both arms. The
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
