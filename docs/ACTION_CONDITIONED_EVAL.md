# Action-conditioned evaluation screen

`q_counterfactual` emits the right causal unit for a learned move policy: one
real decision, the chosen move, and same-unit legal alternatives branched from
the identical pre-action state. The historical Q executables each implemented
part of the reader and gate. That made it too easy for a new experiment to
drop malformed rows, mix decisions across a split, or inspect an external
profile after development had already failed.

`tools/action_conditioned_eval.py` is the reusable boundary for the next
experiment. It is intentionally an evaluator, not a trainer or a gameplay
integration:

- it requires the current `q_counterfactual` header (`34` state features,
  `133` action features and exactly four `r0`–`r3` doctrine replicas);
- it refuses incomplete or repeated decision groups, non-finite values,
  chosen rows that are not first, and a declared return that differs from the
  four replica mean;
- it loads only the frozen `civvis-q-advantage-v1` linear artifact and applies
  the same feature-block mask and role interaction contract as
  `q_advantage_train`;
- it scores every candidate without fitting or threshold search;
- it reports regret, lift, and override outcomes macro-averaged by independent
  game; and
- it opens `--external-data` only after a separate `--selection-data` clears
  the declared coverage gate.

The fixed margin is required on the command line. This is deliberate: the
screen cannot tune an abstention threshold on the profile it is reporting.
The default selection bar is 5% game-macro coverage and positive point lift;
the external bar is 5% coverage and a positive 95% lower confidence bound on
gated lift. A failed selection returns exit code `3` when an external file was
requested and prints that the external file remained unopened.

## Protocol

Train or freeze a model on a development corpus outside this tool. Then run a
fresh selection corpus with an exact seed range, for example:

```text
python3 tools/action_conditioned_eval.py \
  --model /tmp/q-advantage.json \
  --data /tmp/q-standard.csv --data-seed 944000 --data-games 24 \
  --selection-data /tmp/q-selection.csv \
  --selection-seed 948032 --selection-games 32 \
  --min-margin 0.010
```

Only if the selection report passes should the command be rerun with the
untouched deployment profile:

```text
python3 tools/action_conditioned_eval.py \
  --model /tmp/q-advantage.json \
  --data /tmp/q-standard.csv \
  --selection-data /tmp/q-selection.csv \
  --selection-seed 948032 --selection-games 32 \
  --external-data /tmp/q-online.csv \
  --external-seed 947000 --external-games 32 \
  --min-margin 0.010
```

The margin above is an example declaration, not a recommended value. Choose
one margin in the preregistration and keep it fixed through selection and
external evaluation. Do not use this screen to reopen the rejected
`q_override_train` or `q_pairwise_calibrate` corpora; a new learner needs a
fresh independent calibration and its own decision record.

## Reading the report

`ungated lift` measures the ranker's top-scored action even when its margin is
weak. `gated lift` gives the expert zero change whenever the sibling's score
margin does not clear the fixed threshold. Coverage is the fraction of
decisions overridden, averaged per game, so a long game cannot dominate a
short one. The lower confidence bound is the normal 95% bound over per-game
gated lifts and is used only for the untouched external gate.

No report from this screen authorizes a production policy by itself. A passing
external screen earns a separate mirrored gameplay A/B with its own promotion
gate; a failed screen leaves the incumbent scripted controller untouched.
