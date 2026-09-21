# Recover counterfaith recruitment

Native run `civvis-20260921T120510Z` (d5563da29 Rust baseline) lost at turn 207 to Arabia's religious victory. Caracas completed its Holy Site and Shrine and bought two Protestant Missionaries on turn 166. It converted to Islam on turn 167. One Missionary was killed by an Apostle that turn. The survivor's attempted spread beside Caracas spent no charge; it moved to Bogotá on turn 168. By turn 171 neither Missionary remained, leaving no Protestant recruiting city despite later Faith reserves.

The Domination non-founder defense now prioritizes reconverting an owned city with an active Holy Site and Shrine when no such city follows the unit's safe counterfaith. The ordinary target score breaks ties among suppliers. Once any supplier follows that faith, ordinary targeting resumes. Founders, other victory lanes, disabled religious victory, and faiths that could complete another founder's victory retain their existing targeting. Pillaged or incomplete infrastructure earns no priority.

This changes target choice, not movement, native spread rules, or Apostle avoidance. Frozen replays cannot prove successful native conversion, survival, or a victory.

## Validation

- `cargo test --profile ci --locked`: 4,043 passed, 53 ignored, no failures. Six focused source-recovery tests passed, including actual model spread/conversion followed by a new counterfaith purchase, last-charge behavior with both exploration genes enabled, release after a supplier recovers, inactive infrastructure, other lanes/founders/disabled victory, and the unsafe alternate-founder case.
- `cargo fmt --all -- --check` and `git diff --check origin/main...`: passed.
- `python3 -m unittest discover -s tools -p test_treatment_append_points.py`: 14 passed.
- `target/ci/civvis soak --games 8 --players 4 --turns 180 --jobs 4`: all eight games (seeds 0–7) completed.

## Frozen native replay

The complete 614-frame terminal history was replayed with the native runtime's 19 flags. Baseline Rust code matches main `fb75a7dfe`; its orders and internal actions are also identical on all 614 frames to the original pinned d5563da29 replay. Baseline completed in 224.79 seconds and candidate in 203.27 seconds; these unpaired local times are not a performance claim.

Five actionable frames changed: 167/0, 168/0, 168/1, 169/0 and 170/0. All changes concern Missionary 6357002; every other actionable order is identical. Five internal-action frames and six exported frames (including historical order receipts) differ.

At 167/0, the candidate removes the move from Caracas (15,20) to (15,21), retaining the spread from the city itself. At 168/0 it removes the move to Bogotá's adjacent tile (17,24), retaining the spread beside Caracas. Later frozen snapshots still place the unit on its historical route near Bogotá, so the candidate requests movement back toward Caracas at 168/1 and 169/0, and starts returning instead of spending its last charge at 170/0. The historical host did not execute these alternatives; subsequent positions, charges, receipts and religion remain those of the original loss.

Missionary 6291479's route to (13,21) is unchanged; the native Apostle kill remains a separate, unresolved tactical failure. No native Domination victory or successful native supplier recovery is claimed.
