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
on its disposable board. A Settler finishing after observation already fills
that single request, even before the host exports its unit. The generic city
target and economic expansion window are unchanged.

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

Thirteen focused tests cover the two failed actual paths, queue retention,
the peacetime force gap, deferred preview and its incoming Settler quota,
an existing ordinary target,
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

## Completed evidence

All four complete pair records, including both legs, their action counts and
conquest observations, are exactly equal across libraries. The on-row outcomes
below are the same for parent and candidate:

| Seed | Ending | Turn | Focal score | Major cities held | Foreign capitals held |
| --- | --- | ---: | ---: | ---: | ---: |
| 37950000 | Science loss | 224 | 688 | 0 | 0 |
| 37950001 | Science loss | 221 | 469 | 0 | 0 |
| 37950002 | Culture loss | 182 | 468 | 0 | 0 |
| 37950003 | Culture loss | 170 | 561 | 0 | 0 |

Neither source wins Domination in either leg. These games provide execution
compatibility and no evidence of stronger play or of the new handoff firing.
The controlled regressions above establish its conditional behavior.

Both native history replays complete 436 frames with all decisions identical.
Their decision-stream SHA-256 is
`d0303e17b243eba07a090760376259fec9b013ed096d3a2e83dc7f193976e974`.
At the first turn-162 frame both produce only Cali's Shipyard. There is no
new native Settler order, colony, Aluminum income, Bomber, capture or win.
The existing blockers at both known deposits retain their authority.

The frozen candidate is `d26bb4b035150fa34b41fac68cfd1fa450f00d0a`.
Its native decider SHA-256 is
`a6566558419d732738e2394265617c0308aeecf64843fac5529474290f65add7`;
the parent decider is
`3a0c0b182d08cdc2a13d6742a193708e29163c1e7878bfd445fd534c0c87fd5f`.
The candidate evaluator is
`3fdcf470cf81bed5a2f709cebad66b80045ef15386684879704cb5a2878d6991`;
the parent evaluator is
`eba30ba6a0f20e136c10d763e83a29c1fb14bf382d9311d5daddc33962536466`.
Library, harness, forced-bundle and protocol hashes, every pair, complete
decision/why streams and the comparison summaries survive under
`~/civvis-tactics-results/2026-09-26/air-resource-colony-demand/`.

Local full validation on both `e1f9a87bc` and `ef63a4623` passed 4,355 tests,
zero failed, with 53 existing ignores. The frozen candidate applied repository
edition-2021 formatting and an equivalent cache-retirement filter; all twelve
then-existing focused tests and changed-line Rust quality passed. CI then
required the new state/default declaration in its a-b append ranges, rather
than beside the Settler maps. That declaration-only move passed all fourteen
append-point tests and renewed changed-line quality.

Final review subsequently proved a deferred-preview quota error: clearing a
finishing Settler's queue before observing its newborn could request another
supply Settler. The new regression failed first. Commit `2b14c78c5` preserves
that incoming-unit information before clearing the disposable queue; all
thirteen focused tests, fourteen append-point tests and changed-line Rust
quality pass. Only the native orders bridge calls this preview, so the
simulator pilot remains the explicitly frozen probe above. The final native
decider is retained separately with SHA-256
`3b81b0a9d12bf03cb8a46ab0e09d27ecc70705ce814990378fbffd38ddc88f57`.
Its renewed 436-frame persistent replay is also exactly equal to both parent
and frozen candidate, with the same decision-stream hash reported above.
Full local validation on `2b14c78c5` passes 4,356 tests, zero failed, with
53 existing ignores.
Final branch CI verifies the actual merge tree; this document distinguishes it
from the local and frozen probes.

No engine rules, gene registration, deployment default, native runtime, UI or
orders database is changed by this task. Native game adoption and an actual
resource-to-Bomber campaign remain unverified.
