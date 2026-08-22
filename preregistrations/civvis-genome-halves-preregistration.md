# Pre-registration — which half of the shipped genome carries its deployment deficit

Written 2026-07-31 **before the run below produced any output**. Agent
`claude-evolver`, worktree `civvis-evolver-9d31`, base `16f6f27`.

## Why

Measured earlier today at seed 61,000,000 on the promotion matrix's exact
deployment profile: the embedded gen-14 champion scores **42.5%** paired
(−53 Elo, 8 map directions for / 18 against, p=0.0755) against stock
`advanced`, and trails on every diagnostic column except **faith**, in a
profile where religious victory is disabled. Its genome against
`Weights::default()` is a tall faith-and-wonder build: `faith_builder`
350.4 v 120.0, `wonder_min_bld` 1.164 v 3.0, `builder_per_city` 0.200 v 0.5,
`city_target` 2.408 v 4.0, `settler_min_pop` 4.457 v 2.0.

That is a hypothesis about *which genes*, and it is testable by partitioning.

## The partition, fixed now

The **yield half** is `docs/GENOME.md`'s `economy` (7) plus `expansion` (4),
eleven genes: `city_target`, `settler_min_pop`, `settler_stop_turn`,
`min_city_dist`, `builder_per_city`, `wonder_min_bld`, `faith_builder`,
`d_campus`, `d_commercial`, `d_holy`, `d_theater`. The **rest** is the other
twenty-nine.

| arm | genes taken from the champion |
|---|---|
| `r0-stock-artifact` | none — every gene falls back to `Weights::default()` |
| `r1-champion` | all forty (the shipped genome) |
| `r2-champion-yield` | the eleven yield genes only |
| `r3-champion-rest` | the twenty-nine others only |

Every arm is a JSON artifact, so all four deserialize to `PolicyDeck::Live`.
The control `advanced` is `Weights::default()`, which is `PolicyDeck::Legacy`.
**`r0-stock-artifact` is therefore the rig's calibration rung**: it differs
from the control on the policy deck alone. Every other arm is read *against
`r0`*, not against the control, so the deck is differenced out.

★ Added before the run finished, from the repo's own record rather than from
this rig: `r0` is exactly the existing `advanced_policy_live_control` arm's
comparison, and `docs/EVAL.md` measured that at **300 pairs — 52.3% (+16) on
compact and 54.3% (+30) at deployment, inconclusive, retain stock.** So the
expected reading for `r0` is roughly **54%**, with a ±14-point interval at
forty maps. That is the prior the calibration rung is checked against; the
pre-declared rule ("at or above parity, else discard the screen") is unchanged.

## The run

```sh
ai_eval advanced_evolved advanced --pairs 40 --jobs 4 --seed 66000000 \
  --players 6 --width 74 --height 46 --city-states 9 --turns 250 \
  --speed online --map continents --shape planet --poles poles \
  --randomize-civs --victories science,culture,domination
```

from each arm's own working directory, so `advanced_evolved` resolves that
arm's `evolved/best.json` and the comparison differs on `weights` alone. One
common seed prefix, so all four arms are measured on the **same forty maps**.

## ⚠ Execution note recorded at the time, not after the fact

`target/ci/ai_eval` was **relinked at 10:40:31**, between `r0` finishing and
`r2` starting, because a `cargo test --lib` run picked up the one-line
`ArmKind::treatments()` tag added to `src/elo.rs` for the unrelated
`advanced_plan_city_target` arm. `r0` therefore ran on the earlier image and
`r2`/`r3`/`r1` on the later one.

The change adds a string to an arm-identity table and cannot reach either arm's
construction or any game logic — but `docs/EVAL.md` already records one
conclusion damaged by "the eval's binary was built in another worktree", so
this is not left to inspection. **`r0` is re-run on the final binary at the
same seed once the other arms finish; it must reproduce 53.1%.** If it does
not, every cross-arm comparison in this screen is discarded.

## What this is and is not

**A screen.** Forty maps resolve about ±14 points; nothing here can promote
anything, and the individual arms will almost certainly return
`INCONCLUSIVE`. What forty shared maps *can* do is order four arms measured
against a common control on identical terrain.

## The rule, fixed now

- If `r0` does not land at or above parity, the rig is suspect and every other
  number in this screen is discarded rather than interpreted.
- Otherwise the deficit is attributed to whichever of `r2`/`r3` scores below
  `r0`; if both do, to the larger drop; if neither does, the partition is
  recorded as failing to localise it and the line stops.
- Only an arm scoring **above `r0`** may be nominated, and a nomination is
  gated by an unmodified `ai_eval --matrix --pairs 120` at seed 67,000,000 —
  deployment must PASS and compact must not establish a regression. No knob,
  gene, seed or sample size will be chosen after seeing this screen.

## Compute decisions, recorded as they were taken

- **`r1` (the full champion at seed 66,000,000) is dropped unread.** It was
  queued last and is confirmatory only: the champion already has forty maps at
  seed 61,000,000 (42.5%) and, more importantly, **120 maps per profile through
  the recorded matrix** (compact 57.3%/+51, deployment 45.6%/−30). The decision
  rule compares `r2` and `r3` to `r0`, which `r1` does not enter. It is dropped
  for compute, before any of its maps were read, and this is written before it
  started.
- An earlier `city_target` dose–response at seed 62,000,000 was likewise
  **cancelled unread** when the shipped genome's profile split made a
  champion-based rung the wrong base to sweep.
- The 600-map policy-deck matrix was started, then stopped after six minutes
  and re-queued behind these screens: `docs/SCIENCE_PARALLELISM.md`'s resource
  rule says the large-map evaluator must not run while another batch holds six
  or more cores, and it did. Its prefix is deterministic, so nothing was lost.

## Fixed before `r3` returned: what replacing the shipped artifact would require

Written while only `r0` (53.1%) and `r2` (44.4%) were known.

The matrix's compact profile asks only for **no established regression against
stock**. That is the right bar for a new evaluator arm and the wrong bar for
**replacing `data/evolved/best.json`**, because the incumbent artifact is
recorded at **+51 on compact**. A replacement that merely fails to regress
against stock there would throw that away.

So if `r3` is nominated, the nomination carries one extra fixed condition,
declared now: `r3` is also run against `advanced` on the **compact** profile at
seed 61,000,000 — the same maps the champion scored **56.9%** on — and it must
land **at or above 50%** there. Below 50% it may still be an evaluator arm but
is **not** proposed as a champion replacement, and the artifact stays as it is.
This is a common-control comparison, not a head-to-head; the evaluator cannot
seat two genome artifacts in one process and no change to it is contemplated
here.
