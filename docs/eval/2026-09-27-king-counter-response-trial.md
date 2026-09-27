# King Gran Colombia victory-counter routing trial

Registered at **2026-09-27T08:37:13.279394+00:00**, before fresh trial games.
Frozen source `43a039f8ced2c1647674a93fcc7ca02c37737606` has the same production tree as parent main
`3825e10a910c139f48c7bbe2fba2e34fda6254f9`. This includes the corrected withheld-policy identity and native
conquest accounting. There is no controller, deployment, default or evaluator
change in this evidence-only task.

Eight fresh seeds **38210000–38210007** compare the existing host
`counter-in-lane` policy. ON is the deployed control; OFF is the experimental
ablation. Both arms use Gran Colombia in seat zero, assigned Domination,
four majors, six city-states, Tiny 60×38 Pangaea, Online 250, all victories,
and **King players and King barbarians**. Seat zero is exempt from AI handicap;
rivals are adaptive CIVVIS live-bridge controllers, not native Firaxis AI.
Only the focal policy changes. The same 21-option force bundle has SHA-256
`439757684071b807eb4151e055b4a9adf6ac24b7b8b6c0b5da7327b5344dcea0`. Four two-pair blocks alternate execution order.

Complete and preserve all eight pairs, including every loss, identical action
history and absent-contrast exit-2 rejection. Outcomes are read only after all
blocks terminate. There is no within-block source tuning or seed substitution.
Read back the actual option and assigned target before launching fresh games:
OFF uses `--without counter-in-lane`; ON uses the existing host default, without
an unsupported HostOnly `--with` argument. Record source and binary hashes.

The native preflight uses the complete King game
`civvis-20260927T082417Z` prefix through turn 80 (220 state frames). OFF and
ON keep the future observed boards fixed. A replay tests policy identity and
orders on that history; it cannot demonstrate new captures, survival or wins.
The live game continues on its original pinned build throughout this trial.
Its expansion plan during early war is diagnostic context, not evidence that
this policy caused the opening or that disabling it is better.

The primary outcome is Domination wins. Also retain major declarations,
first major-city and original-capital observations, final original-capital
control and home-capital losses. Score and other victory types cannot qualify
as a Domination benefit. This option answers Science and score pressure, so
its comparison cannot isolate Science pressure alone.

A promising pilot requires more OFF Domination wins than ON, no increase in
home-capital losses and no decrease in final foreign original capitals.
It would require a separately registered 16-pair replication before considering
native activation. Otherwise keep the deployed host setting ON. Eight pairs
are directional evidence, not a promotion screen or a reliable win-rate estimate.

Evidence root: `~/civvis-war-evidence-20260927/king-counter-response/`.
`preregistration.json` and `native-prefix.json` preserve the registration and
native input hash. No fresh trial game had started at registration.

## Completed pilot

All four blocks completed with exit zero at **2026-09-27T08:51:02.400988+00:00**.
All eight pairs were then read, with no replacements or treatment changes.
Four pairs had identical complete applied-action histories; four differed.
Both arms had **0/8 Domination wins and 0/8 focal wins**, no observed foreign
major city or original capital, and three home-capital losses. First major-city
and foreign-capital observations are null in every arm. Final foreign original
capital counts are zero throughout. Seven pairs ended in an opponent Science
victory and one in an opponent Culture victory.

| Seed | Winner and victory, both arms | Finish turn OFF/ON | Identical actions | First major war OFF/ON | Focal major declarations OFF/ON | Home capital at end, both |
| --- | --- | --- | --- | --- | --- | --- |
| 38210000 | Georgia Science | 218/218 | yes | 135/135 | 1/1 | lost |
| 38210001 | Sweden Science | 204/204 | no | 129/129 | 1/1 | held |
| 38210002 | Inca Science | 182/183 | no | 174/181 | 0/0 | held |
| 38210003 | Babylon Science | 189/189 | yes | 103/103 | 1/1 | lost |
| 38210004 | Mongolia Science | 203/203 | no | 166/166 | 0/0 | held |
| 38210005 | Ethiopia Science | 183/183 | yes | 118/118 | 0/0 | lost |
| 38210006 | Inca Science | 191/190 | no | 184/— | 1/0 | held |
| 38210007 | India Culture | 171/171 | yes | 165/165 | 1/1 | held |

Execution order is OFF/ON on even seeds and ON/OFF on odd seeds. Focal first
major declarations were observed at turn 135 (38210000), 129 (38210001),
103 (38210003), and 165 (38210007), identically in both arms. Seed 38210006
has an additional OFF declaration at turn 184; ON has none. Neither arm
declared on a major in the other three pairs. A war observation can include
being attacked and is separate from a focal declaration. Each arm made one
minor declaration, on seed 38210002.

The total foreign-city metric also counts nonmajor cities. It reports two
cities ever and one at the end on seed 38210001, and one ever and zero at the end on 38210004,
identically across arms. The major-city collector reports zero: those totals
do not supply evidence of progress toward foreign major capitals.

**Decision: retain host policy ON.** The registered requirement for more OFF
Domination wins failed. This pilot supplies no reason to launch its contingent
16-pair replication or activate the ablation. The four changed histories show
the option can affect behavior, but this sample does not establish a benefit
or equivalence. The native game remains on its original build.

## Readback and native preflight

The evaluator constructor probe read `counter_in_lane=false` for OFF and
`true` for ON, with `Some(Domination)` in each arm. Both persistent native
replays completed all 220 frames and exited zero. Their actual startup records
differ in exactly the `counter-in-lane` treatment, retain identical forced
bundles and report the Domination target. Neither forces the HostOnly option.

The replay produced **zero changed order frames**. Both arms report expansion
on all 219 frames with a reported strategy; one frame is unreported. Complete
JSON differs on 219 frames solely in the note recording the withheld option.
These recorded future boards test the opening only; they
cannot show a new conquest or win.

## Frozen provenance and validation

The binaries were built from registration commit `0583bcbb4544ca43608ef4f46a8982693f38dea9`.
Its only difference from frozen source `43a039f8ced2c1647674a93fcc7ca02c37737606`
is this registration document; the production trees are identical.

| Artifact | SHA-256 |
| --- | --- |
| frozen civvis_orders | `23e797618293c0e41b215517db8a77585042f0990c42308a619a70b856b19a95` |
| frozen victory_eval | `afade64f074882b49a46ee5add09898ce7b74eb70573ecf8d9dbdd7768ed7264` |
| constructor-readback.jsonl | `9224eb9837d0db8c34e410919b8a88b9cdcb6846b4b342eb18f819a380839d5e` |
| native-policy-readback.json | `a22b0ebfb9045da7fc8435a082de1353c01b7bfed91b44ae7dfd27ae0bd84092` |
| analysis.json | `636ad0d504807e1a54019d86ef7a47ec9aabbb0312ac50c41c6db5278e413abe` |
| block0.jsonl | `d2af44c007caabfc88c6d01a3abef6e4af76fc94d3ba85248046ffa94d5c939e` |
| block1.jsonl | `862a2f228a86b77a75ed61dee22fa0b0f8cb6a81470989a1b8b021574b112c3f` |
| block2.jsonl | `1ad167bedc166dd9c236c375c1e245731a5cfe9c268453ae6ca8f74ee953d089` |
| block3.jsonl | `75567c76de1ecb75d7d420f2bc197a8143036996d5e42f5d560a9a40440d31ea` |

Build: `cargo build --profile ci --locked --features developer-tools --bin civvis_orders --bin victory_eval`.
Each frozen evaluator block ran
`--domination-pair counter-in-lane --difficulty king --games 2 --start-seed
SEED --out PATH`, with first seeds 38210000, 38210002, 38210004 and 38210006.
Output paths were exclusive. All eight actual profiles, focal civilization,
assigned target, forced policies and execution orders matched registration.

`cargo test --profile ci --locked` passed: 4,418 tests, zero failures, 53 ignored.
The full suite includes the startup withholding-identity tests. The separate
constructor readback and native replay are the focused trial checks. An engine
soak is inapplicable to this evidence-only document; production code is unchanged.
Documentation command checks passed (seven tests).

The evidence root retains every raw JSONL outcome and block log, immutable
binary copies, registration, constructor and replay inputs/outputs, source and
binary hashes, start/completion receipts, analysis and validation logs.
`artifact-manifest.json` hashes the retained evidence.
