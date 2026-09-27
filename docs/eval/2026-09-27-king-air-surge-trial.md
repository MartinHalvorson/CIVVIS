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
