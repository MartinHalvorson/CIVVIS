# City campaign identity across live-board rebuilds

Fresh-board CLI decisions previously carried strategic and campaign city IDs into a newly allocated board. An unchanged home city could inherit an enemy objective ID, falsely incrementing the capture count; enemy targets could silently move between cities.

The bridge now remaps the strategic attack/defense targets and ordered campaign objectives by city location before normal campaign maintenance. Actual ownership changes remain visible to maintenance. Missing objectives are removed without counting captures. Already-completed campaigns retain their capture history until normal peace cleanup.

## Validation

Four focused tests cover recycled IDs, strategic attack/defense identity, a genuine capture alongside a missing objective, and completed-campaign history. Explicit unremapped controls falsely count an unchanged home as one capture and a capture plus vanished objective as two; mapped cases count zero and one respectively. Initial fixture failures were invalid coordinates and are not regression evidence.

The full local CI-profile suite passed: 3622 library and 203 binary tests, with 49 library and four doctests ignored.

A frozen native replay used 26 exported turns (80–105) from `civvis-20260920T035745Z`, the 19 verification genes, and exact baseline `a65e42829`. Both CLI runs emitted 26 decisions. Baseline journaled objectives switch Cairo/Aleppo four times; the candidate switches once. The remaining switch follows an explicit air-surge appointment against Aleppo on turn 96, rather than a board-ID change. This patch does not change that strategy or remap every other subsystem's city memory.

Replay artifacts are local at `/tmp/civvis-city-campaign-rebuild-replay/`. Frozen exports demonstrate decision continuity, not counterfactual captures or a domination win. No engine rules changed, so engine-equivalence soak testing is not applicable.
