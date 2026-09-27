# One guarded Aluminum colony for a committed Bomber wing

The target is Prince, Simón / Gran Colombia, four-major Tiny Pangaea,
Online speed and Domination, with all other victory conditions enabled.
This closes a supply handoff; it does not establish stronger play.

## Observed gap

Native run `civvis-20260926T222220Z`, source
`3d8d41e43c982b92bd703c1d503c6e86e1a081e5`, ended at turn 218 with a
Culture loss, score 1025. Across its 585 exported states there was no positive
Aluminum stock or income and no observed Bomber. Advanced Flight arrived at
157, following Radio at 151 and a completed Aerodrome at 150.

The immutable prefix through the first turn-162 state is
`~/civvis-tactics-results/2026-09-26/air-readiness/222220-turn162-frame0/events.jsonl`:
16,936 lines, SHA-256
`f411b837a4bb39b0aa423ff0cacb65709899ff879fcabba6ce48d46ba40cbf28`.
It has ten cities, no Settler, seven Builders and no recon unit. Known
unclaimed Aluminum `(6,23)` has 83 uncharted plots in its remote nine-tile
Loyalty neighborhood. Its nearest observed healthy non-ranged land bodies
are seven tiles away. The other deposit `(9,22)` has a negative hypothetical
Loyalty rate. Neither is ready for this colony request. #3793 independently
investigates the scouting step; this change does not clear fog or appoint a
new military expedition.

The original native binary's persistent 436-frame replay, documented in
`2026-09-26-air-resource-expedition.md`, has no turn-162 Settler in either its
ordinary or forced-recon arm. A fresh one-shot controller's Settler suggestion
is not evidence about the native controller's persistent choice.

Two controlled regressions failed before implementation. With the city target
already met and a known guarded Aluminum site, a complete Domination planning
turn queued zero Settlers. An otherwise untargeted newborn chose ordinary
`(10,12)` instead of Aluminum `(12,10)`. The first reservation fix also failed:
the ordinary production review immediately cancelled its at-cap Settler.

## Handoff and bounds

Only enabled `air-surge` / `air-surge-2` with explicit Domination can request
the colony. The existing shortfall contract must identify a revealed resource
for an already committed airfield. A usable owned or peaceful Suzerain
connection backlog stays a Builder job. An appointed timed war keeps its
queues. Existing Settlers and queued Settlers close new acquisition.

One idle legal factory is selected by training time plus route length, with
deterministic city/site ties. One hex per turn is budgeted for that route,
followed by the existing air-surge endgame reserve; it is not a prediction of
native arrival time. Current queues, recent attacks, source-city Loyalty,
host production legality, population, and threatened-city checks are retained.
The handoff follows existing defense and stability reservations but precedes
the peacetime force floor, which otherwise fills every idle queue.

Exactly one valid queued supply Settler remains authoritative through the
ordinary production review. Earlier siege and insolvency responses retain
their authority. The deferred production preview uses the same reservation
on its disposable board. The generic city target and economic expansion
window are unchanged.

An untargeted non-opening Settler may choose the guarded resource ahead of
ordinary economic ranking. Valid ordinary targets keep their walkers. The
existing candidate checks require charted land, legal spacing and ownership,
friendly nearby city-states, a healthy non-ranged land defender within four,
zero visible supported risk and a nonnegative full Loyalty forecast. The
remote fog and ambiguous-border guards are mandatory for this request, even
when an optional generic forecast is withheld. Active route deferrals, capture
scars, other walkers' reservations and unit-specific refusals still apply.

One tracked appointment follows native unit-ID remapping. If its supporting
facts disappear, the cached target is retired before ordinary ranking can
reuse it. The stalled-founding path checks that appointment as well. The
existing emergency shelter, escort, safe-step, retreat and exhaustion paths
retain their own authority; no new movement or founding command is invented.

## Validation and interpretation

Twelve focused tests cover the two failed actual paths, queue retention,
the peacetime force gap, deferred preview, an existing ordinary target,
knowledge/security/legality/clock and connection guards, native ID remapping,
lost permission at arrival and stalled founding, and expired route deferrals.
A controlled normal-production and legal-movement scenario founds the center
and obtains +2 model Aluminum income while its other units remain stationary.
This does not prove a whole-controller expedition or native income.

The new fixed Prince block was registered before implementation at
`~/civvis-tactics-results/2026-09-26/air-resource-colony-demand/protocol.json`.
Seeds `37950000`–`37950003` are retained without replacement. Parent is
`98a1749f6c0cbe6bcb6d3d72e11c9a98b1cd1bbb`; the external evaluator is frozen
from `9dda6319d413cb11bc5fdda55dfa487a6c1f4c11`. Both libraries use the same
current 21-row forced bundle, including `builders-work-through-raiders`, and
the same `strike-reach` paired rows. Prince is set for players and barbarians.
Both legs and all four seeds are retained; the on rows are compared across
sources. This small compatibility probe cannot establish efficacy or promotion.

The native observed-history replay uses the original twenty-row forced bundle,
the immutable prefix and fresh persistent controllers for parent and candidate.
Future observed native states remain fixed. Decisions can show whether a guard
or handoff fires; they cannot demonstrate different survival, scouting,
founding, Aluminum, Bombers, captures or victory.

Full validation, candidate provenance and the paired/replay result tables are
pending. Keep this PR draft until those checks are completed. No engine rules,
gene registration, deployment default, native runtime, UI or orders database
is changed by this task.
