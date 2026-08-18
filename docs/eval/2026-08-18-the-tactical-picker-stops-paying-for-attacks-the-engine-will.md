# The tactical picker stops paying for attacks the engine will refuse

_2026-08-18 · `agent/mbp-m5-pro-64/claude-09ea8434`_

## What was asked

The greedy tactical picker proposes one candidate per enemy tile inside a
unit's reach, selected on enemy-tile membership and distance alone. Each
candidate then costs **two `speculative_clone`s** — one for
`tactical_attack_value`, one for `forcing_reply_penalty`, itself a nested
search. `docs/SIMULATOR_PERFORMANCE.md`'s standing rule says the payer is an
expensive derivation that is recomputed **or discarded**. A candidate the
engine will refuse is the discarded kind, in full.

How many are there, and what does removing them buy?

## How it was measured

A refusal census, by instrumenting the `Err` arm of `Game::apply` behind an
environment variable and splitting authoritative applies from speculative ones
(`visibility_suppressed` distinguishes a `speculative_clone`). One 150-turn
six-player game at the deployment shape, seed 7700000.

Then three questions in order, because a speed claim is worthless if play moved:

1. **Does it change play?** Simulator reports compared byte for byte against
   `origin/main`, four seeds; and `advanced_legal_tactical_candidates` against
   `advanced` through the paired evaluator, 24 pairs / 48 games at 6p 74x46,
   6 city-states, Online, 150 turns, seed 7810000.
2. **What does it cost?** Paired A/B, alternating the two binaries seed by seed
   and summing child user CPU: 3 reps x 4 seeds, 6 players, 74x46, 9
   city-states, 150 turns, Online, `--jobs 1`. Never sequentially — this box
   runs other work and a sequential A-then-B measurement on it has swung 15%.
3. **Is it right?** The gate uses the engine's own predicates, exposed as
   `pub(crate)` rather than re-derived: `combat_target_visible`,
   `unit_has_line_of_sight`, `unit_can_melee_target_domain`.

## What it measured

**The census, on the authoritative board, one game:** 1,150 refused orders
total, of which the combat ones are

| refused order | count | reason |
|---|---:|---|
| `Ranged` | 159 | target is not visible |
| `Ranged` | 112 | line of sight blocked |
| `Attack` | 124 | unit cannot attack into that domain |

**Play: unchanged.** 12 of 12 simulator reports byte-identical across four
seeds. The evaluator returned **24 neutral splits on 24 maps**, 0 sweeps either
way — the signature of two agents playing the same games, and the reason the
arm was withdrawn rather than kept: there is nothing for it to price.

⚠ This corrected a hypothesis worth recording. The expectation was that a
refused candidate could win the argmax and leave the unit doing nothing, which
would have made this a strength fix. It cannot: scoring a candidate applies it
to a clone, and the clone refuses it too, so it never scores well enough to
win. The refusals that reach the authoritative board come from other
construction sites, and this change does not reach them.

**Work: −5.80% user CPU** (135.93s → 128.05s over 24 paired games). The
harness's noise floor is 0.02% on 8 games per arm, so this is far outside it.

## What was decided

**Shipped, unconditional and without an arm.** An answer-identical change has
nothing to withhold and nothing to screen; a flag would only be a second thing
to keep in step. The three predicates move to `pub(crate)` with doc comments
saying which `do_*` applies each, so the next controller to want one finds it
instead of re-deriving it — `unit_has_line_of_sight` in particular is *not* the
public `line_of_sight_from`, which cannot know the firing unit's
`see_through_woods` and would withhold a legal shot.

This is a cost result, not a strength result, and must not be cited as one. It
buys about a twentieth of the simulator's CPU back at identical play. The 211
refused combat orders still reaching the authoritative board each game are a
separate, open finding.
