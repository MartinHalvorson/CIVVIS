# Pre-registration — how much of the search's gain survives a search the exhibition could run?

Written **before** the run, 2026-07-28. PR #519.

## Why

`strategic` measured **~1833 ms per game-turn** at the exhibition's own profile
(6p 74×46 Online) against its **~250 ms** budget — 7× over, and ~200× `advanced`.
Every registered search variant moves the budget *up*. The deployable question is
the untested one: **how much of the gain survives at a cost the exhibition can
afford?**

`strategic_cheap` = `review_every` 80 (half the reviews), `horizon` 20 (half the
rollout), `rotate_lanes` (≈7 branches → ≈3). Measured cost at 4p 24×16 Online:

| agent | 8 games | vs `advanced` |
|---|---|---|
| `advanced` | 6.5 s | 1.0× |
| `strategic` | 73.3 s | 11.3× |
| **`strategic_cheap`** | **6.9 s** | **1.06×** |

A 10.6× reduction, landing at essentially free.

## Run

```
ai_eval strategic_cheap advanced --players 4 --pairs 300 --turns 250 \
  --speed online --seed 6700000 --jobs 8
```

Reference on the same profile: `strategic` v `advanced` measured **+28 Elo**
(54.0%, p=0.0184, gate INCONCLUSIVE).

## Prediction (mine, recorded before the run)

**I predict a null — most of the +28 lost.** All three knobs were cut at once,
and `docs/EVAL.md` records that a shallow estimate is *not rank-preserving* with
respect to a deep one, so pruning discards real signal; `rotate_lanes` alone is
already a recorded null. The reference gain is +28 with a CI that includes zero,
so there is not much to retain.

What would make it positive: the same document's other finding that the search
"only works at all because 22–56% of branches reach a decided game" — decided
branches return exactly 1.0/0.0 and are horizon-cheap, so a shorter horizon may
lose less than the linear cost saving suggests.

## Decision rule, fixed now

- **PASS** requires the unmodified gate. Anything less is recorded as a null and
  the entrant stays evaluator-only. No seed re-rolls.
- Even a **null is informative and worth the run**: it would put a number on the
  cost-performance frontier, which nothing has measured.
- A positive result needs a confirmation at the **exhibition profile** (6p
  74×46) before any deployment claim — the last two estimates taken at
  4p 24×16 both failed to transfer.
