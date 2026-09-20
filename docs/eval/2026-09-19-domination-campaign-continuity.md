# Domination verification: preserve the opening before tuning it

Observed run: `civvis-20260920T033503Z`, Gran Colombia, King, Tiny Pangaea,
Online speed, explicit domination target. Baseline runtime revision:
`dd39e8f58`; orders binary SHA-256:
`c9ecff248318032e3885535a6d5e6623a354f0aaa53d0855848c95ee2a9f8b46`.
Evidence: the run's `events.jsonl`, `why.log`, and `decisions.jsonl` under
`~/civvis-civ6-runs/control/` on `MacBook Pro`.

## Strategic diagnosis

The early conquest opening selected Xanadu on turn 10, but repeatedly
released it because “the objective city changed hands to a third party.”
There were ten such releases, on turns 11, 15, 20, 22 (twice), 28, 29, 33,
36, and 39. Every sampled native frame-zero export on those turns still
reports Xanadu at host (38, 19), belonging to Mongolia (player 2).
At turn 40 the opening expired without assembling its force.

The opening stores mirror-local city and unit IDs. The live bridge rebuilds
its board on every frame, reallocating those IDs. It carries ordinary unit
memory through the host-ID mapping, but did not carry the opening's city or
force. A new mirror could therefore make the target appear captured, or
make survivors appear dead (or replace them with unrelated units).

This is a prerequisite failure for the strategy: raising the opening's
production bids or aggression cannot make its state refer to the right army.
The repair maps the target through its city location, maps force members
through stable host unit IDs, retains the original timing and loss history,
and counts missing declared-force members once. Real ownership changes
remain decisions for ordinary conquest maintenance. A missing target drops
its opening and the associated pinned campaign rather than using a recycled
city ID.

By turn 73 the campaign was also fighting defensively: Quito was threatened,
four land units were assigned to its defense, two to the Xanadu siege, and
three naval units also had siege assignments. The land siege had zero units
staged and 34 strength against an estimated bill of 199. The turn-85
portfolio trace reported five cities, one capital, and military strength 185.
The turn-65 trace had four cities, one capital, and military strength 304.
These are snapshots, not proof that a particular production choice caused
the military decline.

The capital also repeatedly used `capital-settler-after-completion` on turns
11, 15, 20, 22, 28, 35, and 36; nearby barbarian pressure interrupted its
opening for defenders on turns 12, 16, and 32. This makes early capital
safety and the interaction between first-settler completion and conquest
reservation concrete follow-up questions, rather than assuming a larger
military bid alone will supply an assembled force.

## Recorded-turn replay

Both executables read the same frozen export file for turns 1–40 using
`--serve --fresh-board --explain --victory domination
--civ CIVILIZATION_GRAN_COLOMBIA`, with the run's nineteen explicit genes.
The baseline executable above was compared with this change's CI-profile
`civvis_orders` executable. Local outputs are retained under
`/tmp/civvis-conquest-rebuild-replay/`.

| Decision event | Baseline replay | Patched replay |
| --- | ---: | ---: |
| False third-party target release | 8 | 0 |
| Opening selected | 9 | 1 |
| Deadline expiration without assembly | 1 | 1 |

The replay reads one recorded state per turn; the live run also made
within-turn decisions, so its count of ten releases differs from the replay's
eight. The test verifies persistent controller behavior through the actual
CLI integration, not only the remapping helper.

**The replay is not a counterfactual game.** It cannot make new orders alter
later recorded positions, unit production, or ownership. The unchanged
assembly timeout is therefore neither proof of a win nor proof that the
repair cannot improve assembly in a new live game.

## Remaining verification

The objective of dominant domination wins is not achieved by this repair.
The next live run needs to retain one opening, assemble it, and demonstrate
capital captures. Track the target, force identities, assembly share,
declaration turn, military losses, and captured original capitals. If
assembly still expires, inspect the reserved capital queue and rally routes
before increasing army size or opening additional wars. The turn-73 force
allocation also warrants checking whether naval assignments can actually
contribute to the chosen siege.

A separate defensive-queue regression in PR #3592 addresses a confirmed
claim being lost when a later frame starts with the defender already queued
and diplomacy clears the active war before production. Its test reproduced
the defect; Quito's visible defender/settler rescoring alone did not prove a
bad final host order.
