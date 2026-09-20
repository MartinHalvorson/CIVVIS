# Economic repairs must not override siege defense

Native game `civvis-20260920T063729Z`, pinned to `496d52c67`, ended in a religious loss on turn 127, with no captures and four cities lost. Bogotá fell on turn 92. Before that, the emergency selector repeatedly requested Walls, but the ordinary governor replaced them with Library repairs on turn 88 and Commercial Hub repairs on turn 89. The final native production orders were `BUILDING_LIBRARY` and `DISTRICT_COMMERCIAL_HUB`, respectively. This was not merely intermediate reasoning text.

The final defense handoff deliberately tolerates an already-running defender. Its shared predicate also considered every `Item::Repair` defensive, so economic repairs bypassed the protection and could renew the same false reservation in subsequent frames.

A narrower predicate now governs the confirmed-siege handoff and its final restoration. It preserves Walls, wall-repair projects, local land defenders, and repairs to districts with positive defensive strength (including unique fortified districts). Economic building and district repairs can be interrupted when the existing threat evidence warrants defense. The conservative repair exemption used by pre-war/border and economic policies is unchanged. No new threat threshold, production score, or always-on defense gene is introduced.

## Evidence and validation

Three focused regressions reproduced the prior behavior: reclaiming an economic repair queue, restoring Walls after a later economic repair overrides them, and classifying economic repairs separately from fortifications and local defenders. Controls preserve peaceful economic repairs and the broader pre-war exemption.

`cargo test --profile ci --locked` passed: 3,645 library and 204 binary/integration tests; 49 library and four documentation tests ignored. Three focused tests pass. Latest main (28471afa6) was fetched and merged before the full suite; it was already contained. `git diff --check` is clean. This is an AI queue-policy change, so the engine-change crash-soak requirement does not apply.

Replayed all 345 distinct turn/frame prefixes from native turns 1–127, one persistent `civvis_orders --serve --fresh-board` process per arm, with the same domination target and 19 forced genes. Baseline production code is 28471afa6; candidate adds only this siege-queue predicate. Both arms exit zero. The event file grows only through each frame's await marker, excluding future observations/refusals from that decision.

Exactly two frames change actionable orders after excluding `order_failed`, `order_verified`, and `turn_verified` telemetry:

| Turn/frame | Bogotá baseline | Bogotá candidate |
| --- | --- | --- |
| 88/0 | BUILDING_LIBRARY | BUILDING_WALLS |
| 89/0 | DISTRICT_COMMERCIAL_HUB | BUILDING_WALLS |

All other actionable frames are identical. Historical later states still contain the original choices and city loss; this replay cannot simulate the alternative outcome.

Local evidence is saved under `/tmp/civvis-native-defensive-repairs-127`, `/tmp/civvis-defensive-repair-replay`, and `/tmp/civvis-3605-*`. Prefix replay driver: `/tmp/civvis-defense-frame-replay.py`.

## Limits

This fixes a demonstrated conflict between two production writers. Fixed-observation replays cannot establish that Bogotá would survive, that the game would be won, or that aggregate domination win rate improves. Native games must verify those outcomes.
