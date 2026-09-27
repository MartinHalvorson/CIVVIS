# Four-player King Bomber policy ablation

Registered before fresh games on 2026-09-27, against main
`0cb9505c5a41288d0b83aa7c314e30364459651f`. The native King games already
report `air-surge-2` enabled. ON is the deployed control; OFF is the ablation.
This is a new fixed-policy comparison, separate from PR #3809's earlier
source-by-Strike-Reach experiment.

The native game `civvis-20260927T091843Z` lost to Culture at turn 170. It had
Metal Casting by turn 120 but no Niter stockpile or income, and its native
production menu offered Trebuchets instead of Bombards. The known Niter plots
were outside our territory. This supplies a reason to investigate the deployed
air route; it does not prove the policy caused the loss. The next game
`civvis-20260927T094432Z` lost to Culture at turn 182, despite air appointments
at turns 111, 129 and 164. Its first recorded independent Aerodrome order was
at turn 159. We must distinguish an enabled option, an appointment and a fielded
wing before attributing any conquest benefit.

## Registered comparison

Complete eight fresh same-seed pairs, **38230000–38230007**, in four independent
two-pair blocks. Preserve every terminal result and every no-contrast rejection.
Each block runs OFF/ON on its first seed and ON/OFF on its second. Read outcomes
only after every block terminates. Do not replace seeds, tune source or change
treatments during the trial.

The existing `victory_eval --domination-pair air-surge-2 --difficulty king`
runner fixes Gran Colombia in seat zero with an assigned Domination objective,
four major players, six city states, Tiny 60×38 Pangaea, Online speed, all
victories and the natural 250-turn horizon. Both players and barbarians use
King. The focal seat is exempt from the AI handicap. Rivals are adaptive CIVVIS
live-bridge controllers, not native Firaxis AI. Both arms carry the same 21
forced policies, whose SHA-256 is
`439757684071b807eb4151e055b4a9adf6ac24b7b8b6c0b5da7327b5344dcea0`.
Only focal `air-surge-2` changes; version one stays disabled in both arms.

Reuse the immutable evaluator and native replay binaries built at
`0583bcbb4544ca43608ef4f46a8982693f38dea9` for the completed King counter trial.
Its complete diff from current main contains only two evaluation documents and
Python conquest accounting/its tests. No compiled controller, evaluator, rules,
gene ledger or forced-policy input changed. Verify and preserve binary hashes,
the source diff and the actual startup treatment/target records before launching
fresh games. Reuse avoids an unnecessary binary change in this policy trial.

Native preflight replays the complete terminal King history
`civvis-20260927T091843Z`, including all state and tile frames, persistently in
both arms. ON uses the existing default; OFF uses `--without air-surge-2`.
The recorded future boards stay fixed. Compare actual startup option identity,
assigned target, proposed research, production and action sequences. These are
proposals on a fixed observed history, not new native captures or wins. The live
game continues on its own pinned controller while the trial runs.

The primary endpoint is focal Domination wins. Report other wins separately,
foreign major city and original-capital milestones, final foreign original
capitals and home-capital losses. Scores, minor cities and mere appointments do
not establish Domination benefit. Startup option identity must differ as
registered; missing contrast is a retained rejection rather than a successful
comparison.

A promising ablation requires more OFF Domination wins than ON, no increase in
home-capital losses and no decrease in final foreign original capitals. Such a
result requires a separately registered 16-pair replication before considering
native activation. Otherwise retain the deployed policy ON. This eight-pair
pilot cannot establish a reliable win-rate effect or causal native benefit.

Evidence is stored under `~/civvis-war-evidence-20260927/king-air-surge/`.
`preregistration.json` records the exact registration time, sources, force list,
seeds, protocol and decision rule. Raw native evidence, startup records, binary
copies/hashes, all outcomes and logs, completion receipts and analysis are kept.
There is no controller, deployment-default, host policy or evaluator change.

## Validation

Required checks are the actual native startup/readback and replay completion,
all eight completed fixed-profile pairs with their actual profiles validated,
documentation command checks, `git diff --check`, and the full Rust suite.
Engine performance soak is inapplicable to this documentation-only change.

```bash
python3 -m unittest discover -s tools -p test_docs_commands.py
cargo test --profile ci --locked
git diff --check
```

## Native preflight

Both persistent replay processes completed all 479 state frames and exited zero.
Their actual startup records show `air-surge-2` absent for OFF and present for
ON, with version one absent in both, the same forced policies and Domination
assigned. The first actionable difference is at turn 100: OFF proposes
Construction while ON proposes Military Tactics.

Complete order payloads differ on 54 frames. Removing the bridge's
`order_verified`, `order_failed` and `turn_verified` feedback rows leaves
**44 frames with changed action proposals**. The differences include research,
production, next-production hints and unit orders. Strategy labels have the
same distribution in each arm: 305 Expansion, 21 Recovery, 152 Conquest and
one unreported frame. This distinguishes changed behavior from a changed
startup marker, but supplies no counterfactual capture or victory evidence.

The registration timestamp is **2026-09-27T10:10:16.981189+00:00**. Fresh games
started only after native option readback and replay completion. Both frozen
binaries are byte-for-byte copies of the preceding King trial's immutable
artifacts:

| Artifact | SHA-256 |
| --- | --- |
| `civvis_orders` | `23e797618293c0e41b215517db8a77585042f0990c42308a619a70b856b19a95` |
| `victory_eval` | `afade64f074882b49a46ee5add09898ce7b74eb70573ecf8d9dbdd7768ed7264` |

Their original build command was `cargo build --profile ci --locked --features developer-tools --bin civvis_orders --bin victory_eval`.
The native history SHA-256 is
`030cdb6e91c9456582f1aebffd5afe95d0a077ead47788c9a60c9e7e2840b4aa`; its original path is
already preserved in `native-input.json`. The evidence directory retains full
decisions and explanation logs for both arms and both complete-payload and
action-only differences.

## Completed eight-pair pilot

Fresh games began at **2026-09-27T10:14:34.756475+00:00**. All four blocks
terminated with exit zero at **2026-09-27T10:20:47.465952+00:00**; outcomes were
then read together. All eight complete applied-action histories differ between
arms. Every actual profile, focal civilization, assigned target, forced-policy
list and execution order matched registration. No seed or treatment was replaced.

Both arms had **0/8 focal wins and 0/8 Domination wins**. Each arm ended in five
opponent Science wins and three opponent Culture wins. OFF observed no foreign
major cities or original capitals and lost no home capitals. ON observed ten
foreign major cities and one foreign original capital, all in seed 38230007;
that capital remained held at the end. ON lost its own original capital in seed
38230006. These are eight-pair pilot observations, not a native win-rate estimate.

| Seed | Winner and victory, both arms | Finish OFF/ON | First major war OFF/ON | Focal major declarations OFF/ON | Home capital OFF/ON |
| --- | --- | --- | --- | --- | --- |
| 38230000 | Maya Culture | 199/197 | 138/158 | 1/1 | held/held |
| 38230001 | Portugal Science | 207/207 | 132/143 | 1/1 | held/held |
| 38230002 | Ottomans Science | 196/199 | 169/152 | 1/1 | held/held |
| 38230003 | Inca Science | 209/216 | 150/150 | 0/0 | held/held |
| 38230004 | Persia Culture | 172/200 | 144/157 | 2/1 | held/held |
| 38230005 | Spain Culture | 166/164 | 162/133 | 0/1 | held/held |
| 38230006 | Maya Science | 190/194 | —/154 | 0/0 | held/lost |
| 38230007 | Germany Science | 201/215 | 93/149 | 2/1 | held/held |

The ON major-city milestone in seed 38230007 was turn 150; its first foreign
original-capital milestone was turn 158. All OFF milestones and every other
ON major-city/capital milestone are null. A first war observation includes
being attacked and is distinct from a focal declaration. OFF issued seven
major and two minor declarations; ON issued six major and three minor
declarations. Counts of all foreign cities also include minor cities, so those
broader totals cannot replace the major-city or original-capital endpoints.

**Decision: retain deployed ON.** OFF failed the registered requirements for
more Domination wins and no decrease in final foreign original capitals. The
contingent 16-pair ablation replication is therefore not launched. The observed
ON conversion in one seed identifies a useful case to trace, but it does not
establish a general benefit or solve the native supply and timing problem.
No host settings, deployment defaults or controller code changed.

## Final provenance and checks

The registration checkpoint is
`c05a1be710b09d679b124e9be5cdbcb83aa9b430`; its source difference from the
claimed frozen baseline is this document only. Native games continued on their
own pinned controllers. The completed game `civvis-20260927T094432Z` is a
native Culture loss at turn 182; its successor `civvis-20260927T100740Z` verified
four players, King, Gran Colombia and Simón Bolívar, Tiny Pangaea, Online and
Gathering Storm on merged revision `0cb9505c5a41288d0b83aa7c314e30364459651f`.
The native result and replay proposals remain separate from simulator outcomes.

| Evidence | SHA-256 |
| --- | --- |
| `preregistration.json` | `c6128849589d574bff901058ea1387989d350147b7e4268a4a0d99eabb11a332` |
| `native-policy-readback.json` | `effaed188c876d63c23992d29c089257d033a7a4bdaf77f02e0d4449711b6f32` |
| `native-action-differences.json` | `70a84516aa2bbcc8118518741542fc0ef153f260f8142eb931088a4f2c753fbd` |
| `analysis.json` | `586e6a4741dc1779db2f09b6fc74de259446572287af6c29953a9f5bcb286298` |
| `block0.jsonl` | `98e53d6e3f7d2d0b7e7c10bf0fba4fd0d05271fa356b72b46e326ee243b2b0d4` |
| `block1.jsonl` | `ee9626b2262338a58c3056baf415b1b8b1856828d7cb6bef192570a6c51d0c41` |
| `block2.jsonl` | `e9e53a728fb7940dc7d46fadb61c1a1bffd72788a04ebb146712d89eaec6894d` |
| `block3.jsonl` | `1a16aa140b93986a994fdee87938791e09d346ec534a8dbe602f91a4610eef77` |

Each block's frozen evaluator command was
`--domination-pair air-surge-2 --difficulty king --games 2 --start-seed SEED --out PATH`,
with starting seeds 38230000, 38230002, 38230004 and 38230006. Output paths were
exclusive. Full decisions, both-arm startup identities, original native input,
immutable binaries, all raw results/logs, source differences and start/completion
receipts are preserved. `artifact-manifest.json` hashes the retained evidence.

`cargo test --profile ci --locked` passed: **4,418 passed, zero failed, 53 ignored**.
Documentation command checks passed, seven tests. `git diff --check` passed.
No engine behavior changed, so no performance soak was required.
