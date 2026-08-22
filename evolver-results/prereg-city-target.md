# Pre-registration — expansion target of the shipped genome at the deployment profile

Written 2026-07-31, **before any outcome seed in this line has been read**.
Agent `claude-evolver`, worktree `civvis-evolver-9d31`, base `16f6f27`.

## The claim under test

The shipped champion (`data/evolved/best.json`, gen 14) carries
`city_target = 2.408`. `Weights::default()` — the stock `advanced` control —
carries `4.0`. The gene's bounds are `(2.0, 12.0)`.

`city_target` is **live on the deployed path**, contrary to the
"reached only through `unwrap_or_else`" note in `docs/GENOME.md` and
`advanced.rs`'s `city_target_floor` comment. Those describe
`AdvancedAi::assess`'s `plan.desired_cities`. But `docs/OPENINGS.md` §19
records that after the opening book an **untargeted adaptive** agent reaches
`advanced_production` only for `Recovery`; every other adaptive plan ends in
`BasicAi::cities`, and that governor gates Settlers on

```rust
((n_cities + settlers) as f64) < self.w.city_target      // src/ai.rs:4623
(n_cities as f64) < self.w.city_target                   // src/ai.rs:4220 (gold purchase)
```

`ai_eval` reports 100% adaptive plans for `advanced`, so this is the path the
evaluated agent takes. At `city_target = 2.408` the production governor stops
queuing Settlers once the empire holds two cities and none walking.

Three independent lines point the same way:

1. `docs/GENOME.md`'s `gene_leverage`: replacing the **expansion** block with
   uniform draws from its own bounds *helped* at 1.8 SE — the only block where
   scrambling beat the shipped values.
2. The live league (`civvis-spectator-src/league/league.json`, round 3217) has
   bred, over 1011 six-player games, a leader `g28-28` at 19.7% outright wins
   against stock `advanced`'s 16.4%. Its `city_target` is **9.681**.
3. The champion was bred at 4p/24×16 = 96 tiles per player. The promotion
   gate's deployment profile is 6p/74×46 = 567 tiles per player. `docs/AI_GAPS.md`
   §5 records the champion as **+58 and PASS on the profile it was bred on and
   −9, inconclusive, at deployment**.

## Screens (already declared as screens, not gates)

- `advanced_evolved` (gen-14 champion) vs `advanced`, deployment profile,
  40 pairs, seed 61,000,000 — reads the **cities** diagnostic column.
- `g28-28` vs `advanced`, deployment profile, 10 pairs, seed 63,000,000.
- **Dose–response**: the champion genome with `city_target` set to
  2.408 (shipped) / 4.0 / 7.0 / 10.0, every other gene byte-identical, each
  against stock `advanced` on the **same** map set, deployment profile,
  seed 62,000,000. One gene moves; the arms differ from the control on
  `weights` only.

A screen may nominate a candidate. **No screen may promote one.**

## The candidate-selection rule, fixed now

The gated candidate is the `city_target` rung with the **highest paired-map
score** in the dose–response screen, *provided* the screen's cities-per-seat
diagnostic rises with the rung (the mechanism must fire). If the top rung is
the shipped 2.408, the line is recorded as a null and nothing is gated. Ties
go to the **lower** `city_target`, so the shipped value wins any tie.

## The gate, fixed now

```sh
ai_eval <candidate> advanced --matrix --pairs 120 --jobs 6 --seed 64000000
```

run from a working directory staging the candidate genome at
`evolved/best.json`, so `advanced_evolved` resolves to it and the comparison
differs on `weights` alone. Seed 64,000,000 is disjoint from every screen seed
above and from the recorded seeds in `docs/EVAL.md`.

The unmodified matrix rule decides: the **deployment-online** profile must
return `promotion gate: PASS`, and the **compact-standard** profile must not
establish a regression. Any other outcome leaves the shipped genome unchanged
and the result is recorded as a null.

No horizon, gene, rung, seed, sample size or profile flag will be chosen after
seeing that result. If the gate passes, the deliverable is a replacement
`data/evolved/best.json` plus the write-up; if it fails, the deliverable is the
recorded null and the correction to `docs/GENOME.md`'s "`city_target` is only
reached through `unwrap_or_else`" claim, which is wrong for adaptive plans and
is the reason this gene was never swept on the path that reads it.

## What would refute the mechanism rather than the treatment

If cities-per-seat does **not** rise across the dose–response rungs, then
`city_target` is not binding at deployment and every outcome number in this
line is uninterpretable. That check is read first.
