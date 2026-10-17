# Pending prewar siege policy full game audit

The pending `prewar-siege-board` policy changes behavior in three of four fixed high-difficulty simulator pairs, but **all eight games end in rival Science victories**. Neither arm is observed holding a foreign city at a turn boundary or at the end. The enabled arm gains no additional major-war declaration. This is negative full-game evidence, not a promotion or a verified Native win.

## Fixed source and question

The four-map exploratory registration from October 13, 2026 is completed without replacing seeds, shortening games, changing victory conditions or dropping the unexposed pair. The October 17 execution registration freezes source and executable provenance before inspecting any new outcome. Its primary endpoint is completed focal Domination victories, not score share, force assignment or action contrast.

This audits the **historical pending implementation**, not current `main`: frozen source `605f18a1114e2f96b6c5dd1fb61d06c13f1a39ab`, based on `2c6b39292af1cba6661b57d3878cc6401c9d75a4`. It includes the pending writer's local unpublished test/registry-order commit after published PR #4036 head `f6c5d09284fe44ef9d2283343e622713e4567694`. Current `main` at registration is `fc36519b5e5a62ca42cd2063f286ac08fe78a7e1`, several days newer. The old branch, its worktree and its published head are not edited, pushed or integrated by this audit. All 2,280 frozen tracked blobs match that source tree.

The archive retains the complete five-file runtime delta from the historical base, the changed source files and source-tree listing. Reconstructing that base plus the patch reproduces the runtime source; the unpublished commit's documentation is not needed for execution. These results must not be presented as a current-main policy comparison or a Native treatment estimate.

## Unchanged full game profile

- Four players, 60 by 38 Pangaea, six city-states, Online speed, barbarians, all six victory conditions, natural 250-turn clock and no mercy rule.
- Immortal seeds `202610137000` and `202610137001`; Deity seeds `202610138000` and `202610138001`. Player and barbarian difficulty both match the named rung.
- Seat zero is Gran Colombia targeting Domination and is handicap-exempt. Its opponents are three adaptive CIVVIS controllers, **not Firaxis AI**.
- Both arms use the same historical public compiled force bundle and explicitly enable `objective-board` and `war-policy-via-board`. Only the focal seat's `prewar-siege-board` callback differs. Execution order alternates by pair.
- The original wrapper counts observed peace Siege rows and nonempty assigned forces. Its first-force turn is not a measurement of a complete, staged or combat-ready army. Repeated observations are not independent preparation events.

The original October 13 runner does not compile against the frozen implementation: its `expansion_census` forwarding resolves an inherent method returning a non-optional census where the `Ai` trait requires `Option`. The failed build, original runner and initial registration are retained. Before any game launches, the execution registration is amended to forward explicitly through `Ai::expansion_census(&self.inner)`. The only other runner changes point the force include at the frozen copy and persist the existing complete applied-action bytes after each pair. No controller, game rule or production source is repaired for the experiment.

## Complete outcomes

Every terminal winner is rival seat 1 by Science. `OFF` and `ON` name the focal policy callback; first preparation means the first observed nonempty peace Siege force.

| Difficulty | Seed | OFF terminal turn | ON terminal turn | First ON preparation | ON row/force observations | Action contrast |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| Immortal | 202610137000 | 156 | 154 | 128 | 4 / 4 | Yes |
| Immortal | 202610137001 | 151 | 150 | 105 | 7 / 6 | Yes |
| Deity | 202610138000 | 149 | 149 | None | 0 / 0 | No |
| Deity | 202610138001 | 142 | 142 | 126 | 2 / 2 | Yes |

- Focal wins: **0/4 OFF, 0/4 ON**. Focal Domination wins: **0/4 OFF, 0/4 ON**.
- Foreign-city and foreign-original-capital ownership observed at turn boundaries or at the end: zero in every game. This does not establish the absence of a transient capture between observations.
- Nonempty peace Siege forces are observed in 0/4 OFF games and 3/4 ON games. The OFF Immortal `202610137001` game has one peace Siege row but no assigned force.
- Only Immortal `202610137001` records focal major-war declarations: two in each arm, with the first observed at turn 79 in both. Every other arm records zero focal major declarations. Existing wars can also be initiated by rivals; the declaration count is not a count of all wars.
- Both two-pair jobs exit zero because each has at least one complete-action contrast. Deity `202610138000` has no policy exposure and byte-identical full applied-action records; it remains in the results and denominator.

The three contrasting pairs first differ on a focal-seat action, at zero-based complete-log indices 25,688, 21,703 and 31,305 respectively. All eight exact applied-action records are persisted and hashed, including both copies of the unexposed pair. These indices are not turn timestamps. Terminal outcomes are not truncated to a favorable conquest interval.

## Decision and next bottleneck

The implementation can create peacetime allocations and change actual play, but the observed first assigned forces arrive at turns 105, 126 and 128, while these rivals finish at turns 142–156. Assignment alone neither causes an earlier declaration in this cohort nor produces an observed held foreign city. Four exploratory pairs cannot establish a reliable population effect, and the historical source limits transfer to the current deployment. **No default, ledger, forcing or Native adoption change is justified.**

The next useful conquest investigation is the upstream timing of a named, legal Conquest objective and the build/research/march sequence needed to reach it before the Science clock. It must test completed games and preserve defensive obligations, affordability and legal staging. This audit does not establish which of those upstream decisions is defective and does not justify waiving readiness or declaring from global power alone.

## Validation and reproduction

The frozen runner compiles with `cargo build --release --offline --locked`. Its unchanged historical harness tests pass with `cargo test --release --offline --locked`: **11 passed, zero failed**. They cover difficulty handicaps, paired world/seat identity, focal-only policy callbacks, result-file preservation and conquest/capital observations. The inherited runner still emits its unused-import warning; this audit does not silently rewrite it.

`verify_results.py` independently checks all four registered seeds, both difficulty settings, full game profiles, terminal outcome consistency, observation invariants, all eight action counts/hashes and contrast-gate exits. Registered input and executable hashes remain unchanged through execution. The source-integrity check verifies every tracked frozen blob. Current repository production code is unchanged, so its full Rust suite is not rerun for this documentation-only audit; required hosted checks still gate integration.

The adjacent registration and result JSON files expose the complete plan and all eight outcomes. The content-addressed gzip provenance archive retains the original and corrected runners, failed and successful builds, tests, commands, compiler environment, exact executable, runtime patch/source, raw outcomes, complete applied actions and verification scripts. Decode each blob's base64 `data`, verify its SHA-256 and byte count, then restore files by the archive's `files` mapping. Recreate `frozen-source` from the recorded base plus `pending-policy.patch` before building the restored runner. Its included force file must remain at the recorded relative path.

Archive readback verifies all 47 files and 45 unique contents. The compressed archive is 10,323,874 bytes with SHA-256 `96158c87643b823a4712eb611dcb2dcb2624e9dd9ba36390b738ef4efcc5977f`.

Run the restored `run_cohort.py` only into a fresh directory with no result/log files. It launches all four registered pairs using the fixed seeds above. Existing output files are deliberately not overwritten. The local study root is `civvis-tactics-results/2026-10-17/prewar-siege-fullgame-audit`; operational CI and merge receipts recorded after archiving are not retroactively called preregistered experimental evidence.
