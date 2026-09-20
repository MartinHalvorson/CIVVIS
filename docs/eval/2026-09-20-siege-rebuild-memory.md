# Preserve siege identity across native rebuilds

The native match `civvis-20260920T053144Z` on `73f12461d` held Thăng Long as its campaign target but recorded no city attacks in its first 60 turns. Its siege journal alternated Stage and Invest. This observation motivated an identity audit; it does not by itself prove why the attack failed.

The fresh-board CLI already remaps campaign and opening city identities. Siege memory was omitted: the map key is a mirror-local city ID, and each record contains a taker and assigned posts keyed by mirror-local unit IDs. Reallocation can associate this memory with another city or soldier, or leave the actual objective with a new Stage record and a restarted patience clock.

The fix remaps cities by location and units through the bridge's host-ID correspondence before deciding on the new board. It preserves stage, entered turn, assessed turn, and surviving posts. Missing cities are dropped; missing or ownership-changed units lose their posts and taker reservation. Actual city ownership changes remain visible to normal siege maintenance. Reservations are reconstructed from surviving siege takers.

Three focused regressions fail with the remap disabled: swapped city/unit IDs lose the intended record, a vanished objective remains attached to a reused ID, and a captured city loses its previous record. They also cover repeated remaps and a missing taker. All three pass with the fix. `cargo test --profile ci --locked` passes 3,632 library tests and 204 binary tests.

A 65-turn frozen replay from the same native match completed successfully in both arms, using baseline `73f12461d` and the same 19 verification genes. Actionable orders differ on 7 turns (35, 40, 48, 49, 50, 52, 55). For example, Archer 589830 on turn 35 changes from FORTIFY to MOVE_TO (36,18). The late siege still stages in both arms; this does not establish that the identity fix solves the current army’s inability to take the city. Artifacts are in `/tmp/civvis-siege-rebuild-replay/`.

No engine rules or combat thresholds change. Frozen-state replay can establish different recommendations, not counterfactual captures or a domination win.

## Native follow-up and combat-count method

Use `tools/civ6_tactics_ledger.py`'s `combat_section`, which counts both `city` and `district` defenders. A separate ad hoc filter that accepted only `city` incorrectly reported zero attacks in the follow-up match. Rechecking with the existing ledger confirms the original `053144Z` sample still has zero city/district strikes through turn 60; the new `063729Z` match on `496d52c67` has 13. Lisbon and Braga show native city damage by turn 48. This is a different random map, so it verifies actual attacks without isolating a causal win-rate effect or establishing a capture.

District combat records in the new match also expose a separate exporter defect: the mod called `CityManager.GetDistrict`, while Firaxis's `Base/Assets/UI/WorldView/CityBannerManager.lua:1033` resolves them through `Players[player]:GetDistricts():FindID(id)`. Failed lookup yielded `gone=true`, hiding coordinates and garrison/wall health and falsely marking surviving district defenders killed. The district-readback correction uses the shipped UI's lookup; an added combat-ledger regression fails eight checks before the correction and verifies surviving and removed district readbacks afterwards. The Python ledger already excludes district removals from unit-kill counts.
