# Builder job progress and high difficulty conquest foundations

Date: 2026-10-17. Reference source: `262d7e37dfcc3cb6c1d78dbafc9ac7519b2e1bd7`.

A successful movement action does not necessarily advance an improvement job. The risk-aware mover can correctly take a safer step away from the selected tile, while the Builder controller incorrectly treats that step as reason to retain the same job. The correction keeps the safe movement and temporarily sets aside its non-progressing assignment; it does not force the Builder into danger or weaken a capture threshold.

## Current source diagnosis

The four exploratory maps use the frozen public focal force bundle, Gran Colombia Domination in seat 0, three adaptive CIVVIS rivals, four majors, 60 by 38 Pangaea, Online speed, six city-states, barbarians and all six victory types. Seat 0 remains handicap-exempt; opponents receive Immortal or Deity. Every game runs to its natural terminal under the unchanged 250-turn clock. These previously observed maps are diagnosis, not a fresh promotion sample or Firaxis verification.

| Difficulty | Seed | Natural terminal | Focal win |
| --- | --- | --- | --- |
| Immortal | 43250000 | Rival 1 Science, turn 159 | No |
| Immortal | 43250001 | Rival 1 Science, turn 154 | No |
| Deity | 43250100 | Rival 3 Culture, turn 124 | No |
| Deity | 43250101 | Rival 1 Science, turn 138 | No |

On Immortal 43250000, Builder 235 first appears in the turn-35 snapshot with three charges and still has all three in every snapshot through turn 47. Its first recorded charge expenditure is a turn-76 woods chop, followed by plantations on turns 77 and 85. The entire intervening span is not attributed to one stall: the isolated pacing evidence concerns turns 35–47.

During that window the Builder retains the horse-pasture job at `(0,26)`, repeatedly approaching and leaving the neighboring tiles. At turn 42, for example, both the complete route and the legal next step lead directly from `(-1,26)` to the job, but risk-aware movement instead chooses `(-2,26)`. That safe action returns success, so the normal job sweep keeps the pin. The direct capture scorer reads zero on 27 of the 32 logged pinned-target queries; five earlier queries are nonzero. Filtering only high direct-capture goals would therefore miss most of this loop.

Two tempting explanations are rejected. The goal is not globally terrain-unreachable, and relaxing traversal would not repair this case. Nor are early Builder improvements universally absent: through turn 60 these four reference games record 5, 9, 9 and 9 successful Improve actions respectively, including pastures, plantations and charge-spending woods chops. Counting only final farms and mines understates work already performed.

## First candidate: unexposed negative evidence

The first candidate, Git tree `c9c1407bbd838f3f8cf5b69fecf2e1ad8e25ab0b`, guards progress bookkeeping on `builder-tries-the-next-tile` and calls its helper only from the next-job iterator and marginal-usefulness path. All eight registered fresh pairs lose identically, with zero focal or global action contrasts and zero postponement events. All four known-map candidate replays also have identical actions and no postponements. These are unexposed negative results, not evidence that a live correction works or that conquest improves.

Static wiring explains the miss: `gene_ledger::deployment_default_on` reads `genes.rs::DEPLOYMENT_GENOME`, not the reporting row's historical `on` state. The next-job tag is absent from that deployment list and the frozen force bundle. The normal sweep consequently bypasses the first candidate's helper. This distinction matters: the earlier isolated next-job tests pass but do not cover the deployed normal sweep.

## Normal-sweep correction

The revised intervention covers both normal and next-job sweeps, retained and newly selected jobs, and marginal-usefulness assignments. Progress tracking follows risk-aware movement rather than the unrelated next-job opt-in. After the existing safety-aware mover succeeds, an adjacent step that neither follows the pre-step A* first tile nor reduces distance to the job clears that assignment and places it in the existing Builder avoidance map for the existing scaled six-standard-turn hysteresis window.

The actual move remains unchanged. A true A* first-step detour retains its assignment even when straight-line distance does not fall. Multi-hex movement is not reclassified by this adjacent-step rule. Expired avoidance permits the original job again; legacy movement preserves historical assignment handling. No new treatment, deployment flag, genome vote, force-bundle entry, difficulty, clock, victory gate, global movement score or capture-risk limit is introduced. The marginal-usefulness journal describes a retained assignment only when one remains pinned; an off-route retreat gets its own postponement event.

## Exact controls and regression tests

All four reference wrapped versus unwrapped controls match the complete applied-action stream and every recorded pre-turn focal-state snapshot byte for byte. Two further logging-only replays of Immortal 43250000 each match all five original trace outputs exactly: actions, snapshots, plans, journal and terminal result. Source-tree verification checks all 2,756 reference blobs.

The initial unchanged-runtime regression run fails the two intended pin-retention assertions and passes its two controls. The first correction passes those pin assertions but exposes a fixture-coordinate mistake: the unchanged safe endpoint is `(5,6)`, not the initially assumed `(4,5)`. The fixture moves its alternate mine to the actual safe endpoint rather than changing movement or safety. That run and the original RED failures remain in the provenance.

All six first-candidate focused tests pass, as do 98 Builder-filtered tests (including the new tests) and nine marginal-usefulness tests. That candidate passes the full Rust suite: 6,175 passed and 56 ignored, plus scoped formatting and clippy. These results validate the first candidate only, not the subsequently revised normal sweep. Two new normal-sweep regressions explicitly disable next-job mode; their RED/GREEN results and final-source validation are recorded separately.

Both new normal-sweep tests fail on the first implementation, then all eight focused tests pass with the revised helper. The first revised full-suite run exposes one pre-existing assertion that a major-war negative control must retain its job pin. That assertion encodes the bookkeeping deliberately changed here, not a different physical move or weakened capture safety. The updated negative control compares position, remaining moves, job assignment and avoidance with an otherwise identical untreated major-war controller. The existing direct-capture and safe-hold assertions remain intact. The local failure (5,839 passed, one failed, 52 ignored) and the independent hosted failure are retained, not presented as successful validation.

## Fresh full game regression scope

Each cohort contains eight fresh random maps, four Immortal and four Deity, registered once before reference launch. The normal-sweep cohort uses new seeds registered after the first unexposed results, never relabeling those old maps as fresh. Reference outcomes remain unread until all candidate executions finish. The comparison is matched whole-build regression: every AdvancedAi controller, including the rivals, receives the correction in the candidate. It is not an isolated focal-treatment effect. All seeds, losses and unexposed pairs remain in the result.

The first candidate's 2,757 blobs are verified, with only two production source files and the new regression-test module changed from the reference. Revised-source identity and verification are retained under `normal-sweep/`. Unrelated whole-file formatter hunks were restored before freezing either candidate; interrupted pre-cleanup builds remain recorded. Matched probe lockfiles and the frozen focal force bundle remain unchanged. Candidate outputs record their actual tree identity instead of falsely using the reference commit.

The revised games use checkpoint `e2fbee4dfa6e7c659cd44cf089ed9219fd4c5d07`, tree `cb721b4ed4aeae53d8d4dd22d09abcb16b860ec0`, with all 2,757 blobs verified. Final test-control checkpoint `a6550cb6435b3c431e96fb8b6b280950f59fe6cd`, tree `3e32b26ed425b48e6999320a55c5b31ecfc1473d`, differs only in the `cfg(test)` file `src/ai/advanced/tests.rs`. Every production source blob, the eight focused regression tests and the evaluated runtime are unchanged. Game outputs retain the evaluated tree identity. Both external probe locks use the same dependencies, including `syn` 3.0.3; the repository lock uses `syn` 3.0.2. This is matched probe evidence, not a claim that its complete dependency build is the deployed Native binary.

## Revised full-game results

All eight newly registered pairs now have both global and focal action contrasts, with 123 focal postponement events. Both builds still lose all eight games, and neither has an observed foreign-origin city hold. Improved actions through turn 60 rise in aggregate from 33 to 41, but city production at that checkpoint is higher on only three maps and lower on five (aggregate difference minus 5.8, median difference minus 0.5). These mixed economy indicators do not establish a conquest or overall-strength improvement.

| Difficulty | Seed | Reference terminal | Candidate terminal | Improve by 60, old/new | Production at 60, old/new | Postponements |
| --- | --- | --- | --- | --- | --- | --- |
| Immortal | 20603809636 | Rival 1 Science, 163 | Rival 3 Science, 156 | 0 / 2 | 27.4 / 20.9 | 6 |
| Immortal | 20153601825 | Rival 1 Science, 158 | Rival 3 Science, 156 | 5 / 10 | 32.5 / 48.5 | 17 |
| Immortal | 20233954517 | Rival 1 Science, 171 | Rival 1 Science, 165 | 7 / 10 | 40.7 / 41.0 | 34 |
| Immortal | 20987614614 | Rival 2 Science, 144 | Rival 2 Science, 149 | 3 / 4 | 42.2 / 44.2 | 11 |
| Deity | 20196447569 | Rival 1 Science, 152 | Rival 1 Science, 145 | 3 / 3 | 21.5 / 18.0 | 35 |
| Deity | 20757887598 | Rival 3 Science, 138 | Rival 1 Science, 150 | 6 / 6 | 28.5 / 28.0 | 10 |
| Deity | 20340810738 | Rival 2 Science, 135 | Rival 2 Science, 134 | 6 / 3 | 29.5 / 16.4 | 5 |
| Deity | 20562812303 | Rival 2 Science, 129 | Rival 2 Science, 137 | 3 / 3 | 18.5 / 18.0 | 5 |

All four revised candidate wrapped/unwrapped controls match actions and focal snapshots byte for byte. Terminal result fields also match except the expected `traced` metadata bit. The four known-map replays remain exploratory: Improve counts through 60 change 5 to 10, 9 to 8, 9 to 6, and 9 to 12; none becomes a focal victory. Divergent worlds can assign different unit IDs, so these are not same-ID causal comparisons of Builder 235.

The revised analyzer initially stopped on a missing descriptive `comparison_scope` key after all games completed. Its correction supplies the already-registered whole-build scope as a fallback; no seed, source, game, metric or registration changed. The failed script and log are retained. The first cohort's earlier wrapper verifier correction similarly preserves the raw result-bit mismatch instead of relabeling it byte-identical.

Primary evidence is terminal wins and victory type. Secondary evidence is successful Improve actions, fixed pre-turn city production and military-power snapshots, observed foreign-origin city holds, full action contrast, and descriptive postponement events. Improve counts include feature removals and are not by themselves yield gains; snapshot holds do not enumerate every capture between rounds. These CIVVIS games do not establish a Native hardest-difficulty victory.

This ships as a narrowly tested assignment-progress repair, not a successful strength promotion. The next conquest blocker remains the large military/economic deficit and absence of successful city-taking before rival Science victories. The full local tools suite also retains one independently reproduced baseline failure: `test_conflict_hotspots.TheCheckIsWiredAndOneDirectional.test_the_live_roadmap_passes_its_own_check` (3,415 run, one failed, one skipped). It is not silently waived or repaired by this change.

## Final validation and reproducibility

On final test-control checkpoint `a6550cb6435b3c431e96fb8b6b280950f59fe6cd`, `CARGO_TARGET_DIR=<study>/target-validation-final cargo test --profile ci --locked` passes 6,177 tests with 56 ignored, including all eight focused tests. Scoped `tools/rust_quality.py --base 262d7e37dfcc3cb6c1d78dbafc9ac7519b2e1bd7 --head a6550cb6435b3c431e96fb8b6b280950f59fe6cd` passes changed-line formatting and all-feature clippy. The pre-ready fetch/merge reports the same unchanged main revision; no in-flight checks were canceled or superseded to update the head.

The adjacent registration collection preserves both original prospective registrations and explicitly labels its own assembly as retrospective. The result contains every pair, exact validation counts and limitations. The content-addressed provenance archive preserves 58 complete executions, including controls and known-map repetitions; these are not 58 independent strength trials. It contains 494 files in 319 unique SHA-256-addressed base64 blobs, compressed to 39,018,302 bytes. Its SHA-256 is `c1d93ec1f8fe21ec8184a2650f713419148bfc1d243b36b4528e669d12f49fd8`.

To inspect provenance, decompress the gzip as JSON, resolve each relative `files` entry through `blobs[sha256].data`, base64-decode it and verify its recorded byte length and digest before writing into a scratch directory. Both extracted `analyze_fresh_evaluation.py` and `normal-sweep/analyze_fresh_evaluation.py` reproduce their saved summaries byte for byte without an engine rebuild. The packaging round-trip independently verifies all raw candidate wrapper controls as well. Rebuilding a probe additionally requires the full pinned reference checkout and recorded source patches; the archive intentionally does not duplicate every unchanged repository blob or compiled binary.
