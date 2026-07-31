# Terminal counterfactual returns

`q_counterfactual` is an evaluator-only causal data emitter. It observes an
actual `AdvancedAi` move, branches the exact pre-action state across the chosen
move and same-unit siblings, and holds the opponent-doctrine rotation fixed
within each replica. It does not alter gameplay or promote a learned policy.

## Return contract

The historical default is explicit as `--return-mode score-share`. It stops a
branch after `--horizon` turns and records a win/loss when one occurred, or the
focal civilization's living-major score share otherwise. This remains the
default and produces the same CSV for the same command as before this change.

`--return-mode terminal` ignores `--horizon` after the branch point and plays
through the configured turn cap. Each replica emits `1.0` for a focal winner
and `0.0` otherwise; the CSV `return` remains the mean of its replicas. A
branch without a declared winner at the cap is an integrity failure: the run
writes no dataset rather than silently substituting score share. The normal
score victory at a configured turn cap is a real game result, so it is accepted
as terminal evidence.

The CSV schema is intentionally unchanged so the existing strict loaders can
consume either corpus. That makes the mode a data-provenance obligation: record
the full command alongside an artifact and never mix return modes in one
training or calibration split.

## Fixed headless comparison

This evaluation was fixed before generating the fresh corpus. It compares the
two return definitions on the same 24 Standard games, seeds `990200` through
`990223`, with four players on a 44x28 map, no city-states, a 200-turn cap,
observations at turns 50/75/100/125, three same-unit alternatives, and four
matched opponent-doctrine replicas.

```text
q_counterfactual --games 24 --players 4 --width 44 --height 28 --turns 200 \
  --city-states 0 --warmup 50 --spacing 25 --decisions-per-game 4 \
  --alternatives 3 --replicas 4 --horizon 80 --return-mode score-share \
  --seed 990200 --jobs 8 --out /tmp/q-bounded-990200.csv

q_counterfactual --games 24 --players 4 --width 44 --height 28 --turns 200 \
  --city-states 0 --warmup 50 --spacing 25 --decisions-per-game 4 \
  --alternatives 3 --replicas 4 --horizon 80 --return-mode terminal \
  --seed 990200 --jobs 8 --out /tmp/q-terminal-990200.csv
```

The two runs must have identical game/turn/seat/unit/candidate-feature rows and
zero rejected branches, repeated-branch mismatches, or observation errors.
The terminal corpus must resolve every continuation, have binary replica values,
and have each `return` equal the mean of those replicas. The comparison reports
how often the bounded and terminal winner agree and how much decision-level
spread survives to the actual game result.
It is a measurement experiment, not a model-selection or gameplay gate.

## Result

Both fixed commands completed and wrote 359 candidate rows from 94 decisions,
with 1,436 matched doctrine continuations. The row audit found **zero**
game/turn/seat/unit/candidate-feature mismatches between the two files, zero
non-binary terminal replicas, and zero disagreement between a terminal
`return` and the mean of its four replicas. Both runs had zero rejected
branches, repeated-branch mismatches, and observation errors.

| fixed-corpus diagnostic | 80-round score-share | terminal game result |
|---|---:|---:|
| continuations resolved to a victory | 566 / 1,436 | **1,436 / 1,436** |
| decisions with spread above 0.005 | **55 / 94 (58.5%)** | **15 / 94 (16.0%)** |
| mean candidate-return spread | 0.0311 | 0.0612 |
| expert is the first best candidate | 52 / 94 (55.3%) | **87 / 94 (92.6%)** |
| a sibling beats the expert in every replica | 6 / 94 | **0 / 94** |

The two modes selected the same top candidate at only **52 / 94 decisions
(55.3%)**. Their apparent action signal had little overlap: 13 decisions had
spread in both modes, 42 only under the bounded proxy, two only at terminal,
and 37 in neither. More concretely, a sibling was strictly top under the
bounded result but not terminal at 37 decisions; only five sibling improvements
survived both definitions, while terminal found two improvements absent from the
bounded result.

This is a rejection of the narrow claim that the existing 80-round score-share
ordering can stand in for a terminal action objective on this profile. It is
not evidence that counterfactual action learning is impossible: terminal labels
are deterministic, complete, and causal. They are simply much sparser here,
and this 24-game measurement corpus has no robust sibling improvement. Do not
train, threshold-tune, or promote an override from these files. A future Q
experiment should treat terminal returns as the target, use substantially more
independent games, and reserve deployment-profile data for untouched external
calibration.
