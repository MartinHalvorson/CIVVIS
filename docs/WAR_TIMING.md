# The midgame power-spike appointment

Status: **implementation checkpoint under semantic integration; no focal seed
has been read**.

## Implementation checkpoint

The opt-in treatment checkpoint carries one persistent `WarPlan` through target
and technology selection, prerequisite research, predecessor production, exact
upgrade reservation, formal-war preparation, route-aware staging, local
readiness, and same-turn ranged/siege setup plus melee capture. The focused
mechanism suite for that checkpoint was green, but a later pre-data semantic
audit found that the implementation still lacks the stable episode records,
two-level target ordering, capture provenance, and fog-honest appointment seam
required below. The code is therefore incomplete and the focal command remains
prohibited.

PR #574 owns the overlapping assessment-observer seam and lands first. This PR
then merges it, implements the prospective validity amendment below, reruns the
focused and full suites, and completes the repository soak. No focal seed has
been read.

## Prospective pre-data validity amendment (2026-07-29)

This amendment was written after static review of the implementation and before
seed `10100000` or any other focal outcome. It closes four ways the checkpoint
could otherwise answer a different question from the intended timing-policy
experiment:

1. An appointment may consider only a living, legal major with at least one
   City Center in the acting seat's **current** visibility set. A stale or
   never-seen city cannot create an appointment. Current visibility includes
   only the engine's normal team/alliance sharing.
2. Objective and route selection are fog-honest. The objective score uses only
   the visible city, the acting seat's own cities and units, currently visible
   target units, and last-seen tile memory. Unknown tiles are impassable to the
   planning search. Mutating an unmet rival, a hidden city or unit, or an
   unexplored route while holding this public state fixed must not change the
   selected `WarPlan`.
3. A quick objective capture requires immediate engine provenance that the
   treated seat conquered the appointed city from the appointed target after
   its elective declaration. A later owner match is insufficient.
4. A timed declaration is already illegal unless its modern package is
   complete. The complete-package fraction is consequently an implementation
   invariant, not independent capability evidence. It must equal 100% for a
   result to be valid, but it is removed from the advancement score rather than
   being presented as a fitted success rate.

These are prospective validity corrections, not a response to outcomes. The
command, seeds, exposure band, declaration denominators, quick-capture rate,
paired outcome gates, and no-retry rule remain unchanged.

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
`campaign_target_legal` with a City Center currently visible to the acting
seat. A candidate objective must have a remembered-terrain route for a current
land melee unit to the existing 3-to-5-tile staging ring; wrapped or spherical
graph distance alone is not enough. The route search traverses only the seat's
last-seen tile snapshots, applies its current embark/ocean unlocks to those
snapshots, treats an unknown tile as impassable, and uses remembered ownership
for border legality. Live hidden terrain, ownership, cities, and units are not
consulted. Ordinary execution still revalidates each immediate move, and one
temporary obstruction still does not invalidate a plan.

For a currently visible candidate city define the frozen public objective cost

```text
7 * nearest-own-city distance
+ 5 * nearest-routed-land-melee distance
+ 1.8 * visible City Combat Strength
+ 0.12 * visible city HP
+ 0.16 * visible wall HP
+ 0.45 * clamp(visible hostile strength - own local strength, -250, 250)
+ 11 * (6 - remembered passable land approaches)
- 7 * visible population
- 180 if it is an Original Capital
- 135 if its original owner is the acting seat
```

Local strength uses radius seven. A hostile unit contributes only when its tile
is currently visible and `unit_visible_to` is true. Approaches are the six
adjacent remembered tiles and are capped at six. Terms that require private or
hidden state—current off-screen defenders, buildings, districts, yields,
loyalty, unseen population pressure, rival military totals, and inferred
victory pressure—contribute nothing. Lower cost is better.

For each target, inspect land military units legal for this civilization whose
unlock is an unowned technology. An assault candidate must:

1. be melee-capable, because the package must be able to capture a city;
2. improve by at least 8 Combat Strength over its direct, currently unlocked
   predecessor, or over the strongest currently trainable melee body when no
   predecessor exists;
3. have expected full-health damage of at least 36 against the harder of the
   visible objective city's current Combat Strength and the strongest currently
   visible target field unit within six tiles, using the engine's own mean
   damage curve; and
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
Target selection then minimizes one raw objective: the public objective cost
above plus the unscaled estimated turns to research, produce the bodies and
breach package, and march. Ties use stable target and objective IDs. The unit
winner is finalized within each target before any target is compared; remaining
Science may never dominate across targets. These components are reported and
pinned by a deterministic two-level ordering fixture; they are not normalized
or reweighted after results. The selector therefore chooses the best *known
attack window*, not simply the weakest nameplate. No leader or civilization
name receives a target bonus.

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
turns later. At that post-action boundary, a capture is credited only while the
city's conquest provenance names the appointed target as `captured_from` (or an
equivalent engine war event names the treated actor, target, city, and turn).
The quick-strike endpoint is time to first target city, not a promise that every
selected war must eliminate a whole civilization.

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

1. the chooser selects the least-research qualifying unlock *within each
   target*, then compares target objective-plus-launch costs without allowing a
   cheaper technology to dominate across targets;
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
    changing minor or barbarian controllers;
13. hidden-state noninterference holds for unmet rivals, unseen cities and
    units, and unexplored or changed-under-fog route tiles;
14. stable seat-local episode IDs and retained records prevent abort/replan
    events from blending in the evaluator fold;
15. ownership changes without treated-seat conquest provenance never count as
    an objective capture; and
16. every recorded timed declaration satisfies the complete-package invariant,
    while the screen classifier does not treat that tautology as advancement
    evidence.

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

The capture endpoint is credited only from the immediate conquest provenance
specified above after the treated seat's own elective declaration. Third-party
capture, liberation, loyalty transfer, trade, razing by another seat, or a later
objective-owner match does not count as a treatment capture.

Before any outcome gate is interpreted, every recorded elective timed
declaration must satisfy the complete-package invariant. A value below 100%
invalidates the run as an implementation failure and stops the line without a
seed retry; it is neither evidence for nor against the policy.

The treatment advances only if every term passes:

- 15% to 75% of treatment seat-games form a power-spike appointment;
- at least 35% of declared appointments capture their objective within 10
  turns, with at least 12 declared appointments in the denominator;
- paired win score is at least 52%;
- favorable map directions outnumber adverse directions;
- paired terminal-score share is at least 50%; and
- the repository's unchanged promotion gate does not retain `advanced`.

The exposure band prevents a nearly inert arm or a renamed universal-Conquest
arm from advancing. The quick-capture rate is an absolute capability gate, not
a comparison fitted to the control, which has no corresponding appointment
state. If fewer than 12 plans declare, the capability claim is unresolved and
the screen stops.

## Disjoint holdout and strongest-controller transfer

Passing every screen term earns one unchanged 240-map holdout at seed 10,110,000
on the same profile. Coverage must remain in 15%..75%, ten-turn captures at or
above 35% with at least 30 declarations, terminal-score share at or above 50%,
favorable directions above adverse, and the unchanged win gate must say
`PROMOTE`. The 100% complete-package validity invariant is checked first here
as well.

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
