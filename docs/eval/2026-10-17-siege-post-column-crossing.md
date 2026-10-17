# Siege firing posts behind a friendly column

## Registered question

Registered on October 17, 2026 against production `712a48e638dcdd2464b321f9fff9acbb4ee4eb23`, before adding the regression cases or changing production. No Native game, process, deployment pin, difficulty, safety floor, treatment bundle or verification gate is changed by this task.

`siege_posts_keeping` accepts a new or retained firing tile only when `siege_route_step` finds an ordinary first step. That router deliberately treats friendly soldiers as obstacles. In contrast, `approach` already executes a legal `MoveTo` through a friendly column when an assigned post exists. The existing direct-approach test manually supplies a post, so it does not test whether assignment admits that move. Hypothesis: a gun with an otherwise legal firing tile reachable through its own screen this turn receives no post and fails to arrive.

This is a source-derived hypothesis, not an established cause of the previous 32 simulator losses or a measured Native failure. The prior cohort's open-city diagnostic includes unposted shooters; it does not identify this routing predicate as their cause.

## Fixed behavioral tests

Add Catapult and Archer cases using the existing `walled_city` fixture, a narrow dry corridor, a friendly Swordsman between the shooter and an empty range-two firing tile, and unchanged legal movement rules. Require natural post assignment, recorded tactical movement through the stationary screen, arrival at the post and subsequent legal wall damage. Do not manually assign a post to make the positive case pass. Require preservation of a valid previous post through the same column.

Negative controls: insufficient remaining movement to cross the screen, an enemy screen, a sealed terrain pocket and a water-only approach remain inadmissible. Existing reachable-pocket, no-route, staging, entry and full Rust tests must pass. Execute the new tests against unchanged production first; retain the original assertions and baseline failures before implementing the repair. Only test-fixture mistakes may be corrected, with the original failure recorded.

The intended repair is limited to firing-post admission: retain the ordinary route as first choice, then accept a complete engine-legal path to the post this turn only if every traversed tile respects the existing dry-land and reserved inner-ring constraints. Execution still uses existing approach, entry-danger checks, stacking and recorded tactical guards. No new movement rule or weaker endurance/entry gate is permitted.

## Fixed full-game probe

After the regression baseline, freeze baseline and candidate `victory_eval` binaries built with `cargo build --profile ci --locked --features developer-tools --bin victory_eval`. Record their source and executable hashes. The sole production source difference must be firing-post admission; all other compiled source, the deployment ledger and `deploy/live-force-on.txt` remain identical.

For each source arm, run `--domination-pair unwalled-city-spares-the-train --games 4 --start-seed <seed> --difficulty <rung> --out <new-file>` once at Immortal seeds 43250000–43250003 and Deity seeds 43250100–43250103. Retain all journals with `CIVVIS_PAIR_EXPLAIN`. This is 32 complete game executions over eight distinct maps: source arm × difficulty × four seeds × the existing policy OFF/ON contrast. The OFF/ON switch is not an ablation of the routing repair. The source change applies to every controller using the repaired code, not just the focal seat.

Retain the harness's four-player, 60×38 Pangaea, six-city-state, Online, barbarian, all-victory, natural 250-turn setup. Gran Colombia focal seat zero targets Domination and is handicap-exempt; the opponents are adaptive CIVVIS controllers, not Firaxis AI. No early stopping, seed replacement, outcome-conditioned rerun, relaxed gate, or conversion of a no-contrast exit into success. Wait for all four source/rung jobs to terminate before inspecting any outcomes.

Report every game, source-arm contrast, actual completion status, focal wins, victory type, sampled foreign-city/capital holds and the existing policy contrast exit codes. Full canonical action records are compared only within each policy pair by this harness and are not persisted for cross-source comparison. Ownership observations occur at full-turn boundaries and final state; zero observed foreign holds cannot exclude transient capture/recapture inside a round. Journal planning rows do not prove executed arrival or shots. These simulator outcomes cannot establish Native adoption or highest-difficulty Native victory.

## Results

### Behavioral reproduction and repair

Registration checkpoint `3ece4209a6d0000fdbb87eb4996c254c64840a6e` precedes regression-only checkpoint `897b8f26e73dcf9ccdcb64c16973ef785247d80f`. The registration was pushed before the baseline test results were available. Unchanged production fails exactly the three predicted post-assignment assertions (Catapult, Archer and previous-post case): six controls pass, three fail, exit 101. The positive cases fail before their recorded-movement and wall-damage assertions, not because of a fixture setup error.

Repair checkpoint `582dd9ce6` changes only `src/ai/advanced/siege_train.rs` in production. It preserves the ordinary route as first choice. Its fallback requires a complete engine-legal `path_to` the post this turn; every step must remain dry for a land unit already on land and outside the city's reserved inner ring except for the post itself. Both new and previous firing-post admission use this predicate. Melee posts, candidate ranking, line of sight, occupancy, movement execution, entry-danger margins and endurance budgets are unchanged.

The initial repaired siege suite passes **245 tests, zero failures, one existing ignore**. All nine firing-post cases pass, including Catapult and Archer assignment followed by recorded `MoveTo` through a stationary friendly Swordsman, arrival and actual wall damage on the next turn. The original regression file is preserved as an unchanged prefix of the final file (prefix SHA-256 `dd00f8ec39a6f9de433b22436eef116fda9071b30b2f4ad8dcdcdd4d13f9232d`).

Review found that the shared positive helper invokes the ranged-shooter substep for both kinds. A separately appended test therefore exercises the actual Catapult `Arm::Siege` handler, including natural assignment, recorded crossing, no same-turn shot after crossing and wall damage after the turn reset. This supplemental case is not part of the original red run. Its first compilation was interrupted before execution to correct a boolean expectation: existing `fortify_or_stop` returns false, meaning no progress, even when it fortifies. The interruption and unchanged original assertions are retained in the provenance. The final siege suite passes **246 tests, zero failures, one existing ignore**; all ten post cases pass. Complete `cargo test --profile ci --locked` passes **6,169 tests, zero failures, 56 existing ignores**. These tests establish the constructed arrival-and-fire defect and repair, not its frequency in games or a victory gain.

### Fixed full-game probe

Baseline production is frozen from regression-only checkpoint `897b8f26e73dcf9ccdcb64c16973ef785247d80f`; its `victory_eval` executable SHA-256 is `f339065672e1215bd8f1750dac1ac9df63e9d03028b2c87d5ed0f4721ec17163`. Candidate production is frozen from `582dd9ce6acecebab4d96669563d0d86f5d0e840`; its executable SHA-256 is `62b89fff9c9af5accfbb59152e470e2a0aae263e948aff120be2cbda65045927`. The complete committed-tree comparison differs only in firing-post admission. Both arms' 768 tracked source/data/deployment/Cargo inputs are hashed, with supplemental build-script, mod, vocabulary and test-asset inputs also confirmed unchanged.

All four source/rung jobs terminated before the first outcome inspection. **All 32 game executions lose**: 28 rival Science victories and four rival Culture victories, ending on turns 124–162. No foreign city is held at the sampled full-turn boundaries or final state. A transient capture and recapture between observations cannot be excluded.

All four variants (baseline/candidate × policy OFF/ON) have identical reported trial fields on each map, including the recorded economy and conquest fields. The table therefore represents all four executions of each map, not only a favorable variant.

| Difficulty | Seed | End turn | Winning rival seat | Victory | Focal original capital held at end |
| --- | ---: | ---: | ---: | --- | --- |
| Immortal | 43250000 | 159 | 1 | Science | No |
| Immortal | 43250001 | 154 | 1 | Science | No |
| Immortal | 43250002 | 147 | 2 | Science | No |
| Immortal | 43250003 | 162 | 1 | Science | Yes |
| Deity | 43250100 | 124 | 3 | Culture | Yes |
| Deity | 43250101 | 138 | 1 | Science | Yes |
| Deity | 43250102 | 144 | 3 | Science | No |
| Deity | 43250103 | 147 | 3 | Science | No |

Each of the four child jobs exits **2**, failing the existing no-policy-action-contrast guard: **0/4 changed OFF/ON action streams at each rung in each source arm**. These failures are retained, not waived or rerun. Completion of the parent receipt-writing wrapper is not success of its child contrast gates. This OFF/ON comparison is not an ablation of the unconditional routing repair. The completed source-arm outcomes remain descriptive negative evidence, not an exposed treatment-effect estimate.

All **16 cross-source journal pairs are byte-identical**. Together with identical reported fields, this shows no recorded field impact in the fixed cohort, but does not prove cross-source canonical action identity: those bytes are not persisted. There is no dedicated repair-activation counter, so its activation frequency is unmeasured. Journal phase rows include Invest/Reduce in several maps and no Take rows; these are planning observations, not executed-shot or actual-capture counts. Six of the eight candidate-ON maps record a focal major declaration; the losses cannot all be explained by failure to declare war. No strength, survival, speed or victory improvement is established, and no Native adoption or highest-difficulty Native victory is verified by this simulator study.

### Integration validation and retained evidence

- All 768 registered source/data/deployment/Cargo hashes match the frozen candidate except the intentionally appended test file. Every original test byte remains its unchanged prefix; the production repair is unchanged.
- `rustfmt --edition 2021 --config skip_children=true --check` passes for both changed Rust files; `git diff --check` passes.
- `python3 tools/rust_quality.py --base origin/main --head HEAD` passes, including the supplemental test: changed lines are formatted and warning-free.
- Source checkpoint `15bf25b8e220567f6e0058e08d9fe13181ab383c` passes every published check. Its paired-cost run `39015419297` measures −0.64% median over five pairs, inside the ±1% noise floor and +8% budget; IQR 1.16 percentage points, resolution ±0.77%, pooled −0.30% over 600 turns. No speedup is claimed. The final evidence/test checkpoint must pass its own gate before merge.
- No engine mechanic changes; no Native process, installed mod, policy/default, difficulty, ledger, pin, readiness threshold or safety margin is changed.

The adjacent `2026-10-17-siege-post-column-crossing.result.json` contains the compact complete trial readout, validation results, source/binary identities and limitations. The adjacent `2026-10-17-siege-post-column-crossing.provenance.json.gz` contains 81 retained files: original registration, runners/auditors, all raw cohort rows and journals, build/source receipts, original red output, initial/final green and full Rust output, the interrupted supplemental compile, quality output and source-checkpoint CI evidence. Its SHA-256 is `c376c79ef3b1f870576622ceb1d8e6fe7639ce977cb7dcb0e428812e90aafc5c` (3,565,892 bytes). Frozen executables and the uncompressed study remain under `/Users/martbot-mbp-m5-max-128/civvis-tactics-results/2026-10-17/siege-post-column-crossing/`; binaries are not included in the archive.

Ship this as an arrival-planning correctness repair, not a strength promotion. The negative full-game evidence keeps coordinated pressure, wall-breaker availability and the production race open as the next investigation; safety and victory requirements remain unchanged.
