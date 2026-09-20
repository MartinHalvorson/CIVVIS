# Preserve the appointed air package through queue review

The King / Tiny Pangaea / Online Gran Colombia verification game
`civvis-20260920T043118Z-cont4`, running `a65e42829`, reached Advanced Flight
at turn 206 without a Bomber. Cali completed its Aerodrome at turn 203 and
started a Canal before the aircraft unlocked. The existing faster-base
heuristic did reserve alternatives, but ordinary production review immediately
replaced them:

- Turn 203: Bogotá starts Aerodrome, estimated two turns, then switches to
  Pike and Shot (102 utility versus 55 for the Aerodrome).
- Turn 205: Quito starts Aerodrome, estimated five turns, then switches to
  Pike and Shot (89 versus 45).

These are the controller's `why.log` decisions, not proof that the host started
and canceled both items independently. The final order is what matters to the
host. The game ended in a diplomatic loss at turn 213 with no cities captured.

## Cause and change

The reservation refreshes package status after queuing an item. That status
counts the item as committed. The governor then prices the same queue as if its
requirement had already been satisfied, removing the package bonus before the
build finishes. This also affects the final Bomber and capture body in their
respective quotas.

The live production scorer now counts the board's current queues, and excludes
only the candidate city's own committed item when pricing its remaining quota.
Other cities still see that item as supplied. The first queued airfield retains
its bonus. A second queued airfield, allowed by the existing faster-base
selection, retains it while the two-Bomber launch wing remains uncommitted and
Aluminum is ready. A third field receives no such bonus. There is no bonus
without an active appointment or for an excess completed unit package.

The existing legality, economic recovery, and immediate defense checks remain
upstream of ordinary score comparisons. This change does not alter aircraft
costs, native district legality, research unlocks, or the faster-base selection.

## Evidence

Two regression tests failed against the pre-change scorer: the reserved
Aerodrome scored 4.91 and the final queued Bomber 1.19, both losing their package
priority after commitment. Tests also exercise production review, the second
field's launch boundary, inactive appointments, and the capture-body quota.

`cargo test --profile ci --locked` passed: 3,633 library tests and 204 binary
tests, with 49 library and four documentation tests ignored. All four new
regressions passed. `git diff --check` also passed. No engine mechanics changed,
so an engine soak was not applicable.

A frozen-export comparison against `73f12461d` replayed turns 176–213 with the
same 19 forced verification genes, Gran Colombia, domination target, and
`--serve --fresh-board --explain`. Both binaries returned 38 decisions and
exited successfully. The baseline recorded three Aerodrome displacements; the
patch recorded none. At turn 204, Bogotá's final production order changed from
Pike and Shot to Aerodrome at native coordinates (28,11). At turns 193 and 211,
the patch removed the orders replacing Cali's and Quito's queued Aerodromes.
Actionable orders changed on eight turns; three additional turns changed only
order-verification telemetry (`order_verified`, `order_failed`, or `turn_verified`).

Local replay artifacts: `/tmp/civvis-air-queue-replay/`, including `args.json`,
`events.jsonl`, and baseline/patched orders and reasoning logs. The frozen
export SHA-256 is
`72dd0b8ce3e603b57c64be86a533eeec699141dc798165ac6f705766d24fb24c`.

Frozen exports show different recommended orders; they cannot establish a
counterfactual capture, completed aircraft, or domination win. Native outcome
verification remains required.
