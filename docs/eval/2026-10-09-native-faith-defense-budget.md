# Native faith defense budget experiment

Native run `civvis-20261008T233904Z` lost to a science victory at turn 191. At turn 150 it held 2,179 Faith and the native menu offered Artillery for 430 Faith, Tanks for 480, and Field Cannons for 330. No military Faith purchase was issued during the game; the final bank was 5,206 Faith. These are performance witnesses, not win evidence for a proposed fix.

The replayed observation exposed 56 legal standard military Faith purchases; all 56 applied successfully to independent mirror copies. With the exact deployed force-on list and a fresh Domination controller, the existing rule kept 2,179 Faith and issued no military Faith purchase. Disabling only `counterweight-finishes-one-shrine` in this diagnostic produced a Tank purchase. This diagnostic did not actuate native orders or change the deployed policy. Fresh-controller replay does not preserve the original controller's cross-turn memory.

The rule's `counterweight_bank_held` reserved the entire bank whenever a rival religion crossed the warning threshold and held one of our cities. The reservation was added to the separate priced Missionary reserve, so a growing bank remained unavailable even when there was no unfinished safe counterweight source.

## Candidate behavior

The candidate holds the entire bank while an actionable safe source awaits its Shrine. When that Shrine is complete, the existing priced Missionary shortfall remains protected and surplus can fund the army. Without an unfinished safe source, a bank hold cannot buy counterweight Missionaries; the candidate releases that indefinite hold. The existing military upkeep guard remains active.

This is an isolated native controller experiment. The source module belongs to the private native integration and is absent from public `origin/main`. Apply the zero-context patch to its recorded parent with `git apply --unidiff-zero`. The [reviewable source patch](2026-10-09-native-faith-defense-budget.patch) records only that module and its regression tests; this report does not add an unused module to the public crate.

- Parent candidate: `cce830c6cb1c1fe0ced6ffb386296534355837eb` (lake identity plus merged Campus fix).
- Candidate revision: `41bae96dea386e3cdfcdafa5e18b0a0a7a43e32e`.
- Candidate snapshot: `/Users/martbot-mbp-m5-max-128/civvis-pins/native-lakes-campus-faith-cce830c6c`.
- Patch SHA-256: `1fc4792b4eeaebda667a0b73e901aca8c476f89d06cc27c4232d0cfc4e012650`.
- Game policy and force-on list: unchanged.

## Validation and acceptance

The existing unfinished-source and religious-defense tests must remain green. New tests cover a completed source with a nonzero Missionary reserve, an unavailable safe source, and an actual solvent military Faith purchase after completion. All 39 focused regressions passed (17 counterweight, 1 faith mobilization, 2 upkeep guards, 13 Campus, 4 naval, 2 lake identity). The final release build of both native executables passed in 2m41s. With the rule still enabled and the force-on list unchanged, the built candidate now issues the Tank purchase in the same recorded turn-150 replay and retains 996.5 Faith. Both enabled and diagnostic-withheld replays choose that purchase after the correction.

The native runtime in `civvis-20261009T001708Z` reported the exact candidate revision and `civvis_orders` SHA-256 `75c3623e5909203c1b51cd1f9b1463d3e3ea4cdfc375a23956ae366af5400cec`. A Pikeman purchase for Faith issued at turn 132 in Maracaibo was verified by the native bridge at turn 133. A separate fresh-controller replay of that position produces no army purchase under either parent or candidate: earlier spending leaves 433 Faith, below the existing 600-Faith army threshold. The native purchase proves actuation but does not isolate the hold correction as its cause. The turn-150 replay above does isolate the corrected hold on its recorded input.

The combined candidate finished three native attempts with no wins: production-rank retirements at turn 150 in `civvis-20261009T001708Z` (rank 3) and `civvis-20261009T002536Z` (rank 4), then a religious defeat at turn 144 in `civvis-20261009T003632Z`. It captured no cities in these attempts. Its results are 0/3, including both retirements. The candidate includes the lake and Campus fixes; these outcomes belong to the bundle, and no comparison establishes a win-rate gain.

Acceptance requires the enabled rule in the built candidate to issue a legal military Faith purchase on the recorded observation, then actual native purchase application and game outcomes. Native wins determine success. Natural defeats and production-rank retirements remain non-wins; incomplete attempts remain separate. No win-rate improvement is claimed.
