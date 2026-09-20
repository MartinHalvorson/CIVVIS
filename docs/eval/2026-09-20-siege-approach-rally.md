# Near-side siege rally experiment: no native action effect

## Decision

Do not merge this experiment as a solution to the ongoing native domination stall. Five focused tests pass and the alternative rally is geographically nearer the approaching army, but all 307 frozen native decision frames produce identical native actions and actionable orders. The readiness improvement is only a reporting change in this deployed configuration.

## Hypothesis

Game `civvis-20260920T065948Z`, pinned28471afa6, keeps task force72 assigned to Thessalonica from turns91–117. At turn108 the city is at axial(16,15), the assembly anchor is(13,18), and most troops are east of the city. The existing `far_side` prioritizes distance from a visible hostile centroid before proximity to the army, potentially choosing a rally beyond the city.

The candidate selects passable land two or three tiles from the city, on the side nearer the army, preferring proximity to the force before distance from enemies. It retains an existing assembly point within three tiles, excludes visible hostile military occupancy and foreign city centers, and falls back to the force medoid when no eligible approach tile exists. Only land Siege rows use it; naval sieges, Defend and Relieve retain their prior rule. This is a geometric preference, not a route-reachability proof.

## Validation

`cargo test --profile ci --locked approach_rally_tests`: five passed. The initial implementation failed three tests; the final far-side test adds a nearby observer and directly asserts that the retained old helper chooses a detour while the new projection does not. Controls cover no visible enemy army, an already assembled force, unavailable land staging, and unchanged naval policy. No full suite or crash soak was run because the experiment is being archived, not shipped.

Baseline code is a374d87d7 (aircraft fix); candidate differs only by the land-rally rule. Each replay uses one persistent `civvis_orders --serve --fresh-board` process, the deployed 19 genes, and the domination target. Input grows only through each native await marker. Both arms exit zero after307 frames. After excluding `order_failed`, `order_verified`, and `turn_verified`, there are zero actionable order changes. `decision.native_actions` also matches on every frame.

On turn108, the first task-force journal entry changes from anchor(13,18),0% ready to anchor(18,16),23% ready. The army does not move differently.

## Why the hypothesis failed

`src/ai/advanced/siege_train.rs::siege_train_step` dispatches Stage directly to `siege_stage_step`. That routine fights opportunistically, backs away from city strike range, otherwise routes toward `city.pos` with `STAGING_FAR` as its stopping radius. It does not consume `force.rally` or `group.anchor`. The journal's readiness is therefore not proof that the active movement controller is waiting for that rally.

Next investigation should examine the actual staging route, local combat constraints, siege composition and target selection. Do not infer that raising reported readiness improves native conquest.

Evidence: `/tmp/civvis-native-siege-rally-117`, `/tmp/civvis-siege-rally-replay/{baseline,patched}-orders.jsonl`, corresponding why logs, `/tmp/civvis-rally-frame-replay.py`, and `/tmp/civvis-3607-*`.
