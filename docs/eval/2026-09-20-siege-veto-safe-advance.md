# Safe approach after a siege attack veto

The native King domination run `civvis-20260920T065948Z` had healthy siege
reinforcements held by the battle planner because every proposed attack was
suicidal. The attack veto also marked them ordered, preventing the later siege
staging logic from approaching the objective.

The planner now permits a healthy (at least 80 HP), unembarked land siege unit
outside the five-tile staging radius to take one legal adjacent step toward its
at-war city objective. The existing danger field must predict no reply damage,
and no enemy unit may occupy the destination. The unit then fortifies or stops
and remains marked ordered: the vetoed attack cannot run later in the turn.
Wounded recovery, garrisons, escorts, reserved takers and non-siege units retain
their existing handling. Safety is conditional on the observed model, including
its visibility and combat estimates.

## Controlled validation

The full battle planner regression first failed against the original code: a
Horseman with only suicidal attacks stayed put despite a safe step toward its
siege. The fixture confirms the planner actually vetoes those attacks before
asserting movement. The fixed planner advances without damaging enemies or
spending its attack, and keeps the unit ordered. Controls cover non-siege units,
blocked approaches, wounded recovery, partial health below the return threshold,
and an enemy civilian occupying the only otherwise safe step.

## Frozen native replay

Both executables process the same growing native event prefix in persistent
`civvis_orders --serve --fresh-board --explain` sessions, using the verification
batch's 19 forced genes and domination target. The source is frozen through
turn 120 frame 2: 307 frames, both processes exit 0. The local source directory
is named `/tmp/civvis-native-siege-rally-117`; its name does not describe the
actual final frame. Baseline production code is `8d4e986af`; the candidate adds
only this battle planner change.

After excluding `order_failed`, `order_verified` and `turn_verified`, five frames
have different actionable orders. `decision.native_actions` also differs in five
frames. The archer with host ID 851976 receives new approach moves at turns
101/0 (29,16), 106/0 (30,15), and 107/0 (30,15). Other differences are adjacent
unit routing and subsequent-frame order suppression. Journal model coordinates
differ from the exported host coordinates. Repeated frames use recorded host
states; repeated proposed moves are not multiple completed advances.

This establishes an actual change to native orders, unlike the rejected rally
anchor experiment in #3607. It does not establish host acceptance, a captured
city, increased victory rate, or a domination victory. The running native batch
remains pinned to `28471afa6` and cannot exercise this change until a fresh batch.

Artifacts on the verification host:
- `/tmp/civvis-siege-veto-replay/{baseline,patched}-orders.jsonl`
- `/tmp/civvis-siege-veto-replay/{baseline,patched}-why.log`
- `/tmp/civvis-veto-frame-replay.py`
- `/tmp/civvis-3608-{red,green,baseline-replay,patched-replay,full}.log`

No engine or game rules changed, so an engine crash soak is not applicable.
