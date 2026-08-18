# The tactical picker stops paying for attacks the engine will refuse

_2026-08-18 · `agent/mbp-m5-pro-64/claude-09ea8434`_

## What was asked

The greedy tactical picker proposes one candidate per enemy tile inside a
unit's reach, selected on enemy-tile membership and distance alone. Each
candidate then costs **two `speculative_clone`s** — one for
`tactical_attack_value`, one for `forcing_reply_penalty`, itself a nested
search. `docs/SIMULATOR_PERFORMANCE.md`'s standing rule says the payer is an
expensive derivation that is recomputed **or discarded**, and a candidate the
engine will refuse is the discarded kind, in full.

How many are there, and what does removing them buy?

## How it was measured

A refusal census, by instrumenting the `Err` arm of `Game::apply` behind an
environment variable and separating authoritative applies from speculative ones
(`visibility_suppressed` marks a `speculative_clone`). One 150-turn six-player
game at the deployment shape, seed 7700000.

Then three questions, in this order, because a speed claim is worthless if play
moved:

1. **Does it change play?** Simulator reports compared byte for byte against
   `origin/main`; the `ANCHOR_BEHAVIOUR_FNV` fingerprint across its five
   profiles; and `advanced_legal_tactical_candidates` against `advanced`
   through the paired evaluator, 24 pairs / 48 games at 6p 74x46, 6
   city-states, Online, 150 turns, seed 7810000.
2. **What does it cost?** `tools/speed_ab.py`, 8 paired games, seeds
   7320000–07, 6p 74x46, 9 city-states, 150 turns, Online, `--jobs 1`.
3. **Is it right?** The gate uses the engine's own predicates, exposed rather
   than re-derived.

## What it measured

**The census, authoritative board, one game:** 1,150 refused orders, of which

| refused order | count | reason |
|---|---:|---|
| `SlotPolicy` | 487 | no free slot for that card |
| `Ranged` | 159 | target is not visible |
| `Attack` | 124 | unit cannot attack into that domain |
| `Ranged` | 112 | line of sight blocked |

**Play: unchanged.** Simulator reports byte-identical, the anchor fingerprint
unchanged, and the evaluator returned **24 neutral splits on 24 maps**, 0
sweeps either way — the signature of two agents playing the same games.

**Cost: nothing, in either direction. −0.18%, inside the ±0.2% noise floor.**

⚠ **This round is worth reading for the two things it got wrong before it got
that number.**

*First, the hypothesis.* The expectation was that a refused candidate could win
the argmax and leave the unit doing nothing — a strength fix worth an Elo gate.
It cannot: scoring a candidate applies it to a clone, and the clone refuses it
too, so it never scores well enough to win. The 211 refused combat orders that
do reach the authoritative board come from other construction sites —
`rush_siege_step` applies a `Ranged` on a bare distance check — and this change
does not reach them. That remains open.

*Second, and more expensive: the first implementation was a **+6.43%
pessimization**, and a hand-rolled harness reported it as **−5.80%**.* Two
independent failures stacked:

- **The API.** `combat_target_visible` recomputes `player_vision_now` — which
  **clones an entire `TileBits`** — and `visibility_viewers` on every call. The
  engine's own `legal_actions` builds both frames once and passes them to
  `combat_target_visible_at`. Calling the convenience wrapper once per candidate
  tile put a per-tile allocation exactly where the work was being removed from.
  Hoisting the two frames per unit, built lazily because most units reach no
  enemy tile at all, is what took +6.43% to −0.18%.
- **The harness.** The hand-rolled paired script reported the *opposite sign*
  from `tools/speed_ab.py` on the same change. `tools/speed_ab.py` landed in
  the tree that same day (#1980) for precisely this reason, and it is the only
  thing that should be used. Do not rebuild it from the prose; the prose is
  what everyone has been rebuilding it from, and this is what that costs.

## What was decided

**Shipped as a correctness change, explicitly not as a performance one.** It is
answer-identical and free, and what it buys is that the controller stops
emitting orders the engine will refuse — which matters to anything that trusts
the candidate set, and to the census as a diagnostic. It buys **no** measured
CPU and **no** measured strength, and must not be cited for either. Its
evaluator arm was withdrawn: an answer-identical change has nothing to price.

The lasting artifact is the doc comment now on `combat_target_visible`, naming
the trap and its measured cost, and `combat_target_visible_at` exposed beside
it so the next caller finds the batched one first.

## Footnote: a refinement to the same day's gate round

`2026-08-18-the-promotion-gate-stops-charging-every-split-map-a-coin-fli.md`
says the new interval's advantage "arrives from 25 maps up". That was
generalized from three recorded score vectors and is shape-dependent, not a
threshold: a 24-map run recorded here (`advanced_engine_faith_price` against
`advanced`, 5 sweeps / 17 neutral / 2 against) has a betting CI of
35.2%..78.9% against Wilson's 36.9%..73.8% — wider on both ends. Both gates
call that run INCONCLUSIVE, so nothing was decided differently. The operative
claim of that round is its Monte Carlo power table, which is measured over
2,000 trials per cell and is unaffected: equal power at 20 maps, materially
better at 40 and 60.
