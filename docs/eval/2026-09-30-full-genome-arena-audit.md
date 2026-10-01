# The live seat's tactics in the doctrine arena, on its full genome

_2026-09-30 · `claude-opus55-loop` on `mbp-m5-max-128` · doctrine arena_

## Why a full-genome seat

`advanced` in the arena is the deployed controller with every opt-in off, and
it has no victory contract, so neither the deployment genome nor any
Domination-only branch played there. The seat below is `advanced_domination`
(the Domination contract) plus all 138 `DEPLOYMENT_GENOME` tags and the 26 tags
of `deploy/live-force-on.txt`, minus `doomed-blow-veto-2` — the live seat as it
plays from game 4. The identical-seat control reads exactly zero.

## What it measured (120 or 100 seeds x 2 role swaps, all 13 positions)

| change against the full seat | pooled mean | ± se | t | better/worse | the_storming |
|---|---:|---:|---:|---:|---:|
| restore `doomed-blow-veto-2` (seat is better without it) | +63.2 | 9.8 | 6.44 | 671/447 | +5.2 ± 44.1 |
| drop `shared-danger` (seat is better with it) | +12.4 | 8.7 | 1.44 | 548/504 | +41.7 ± 32.0 |
| add `anvil` | +21.2 | 5.3 | 4.03 | 89/70 | **+286.3 ± 56.6** (62/29) |

`anvil` fires only on the two city boards: `the_storming` +286.3 ± 56.6 with
cities taken +70/-36, `the_relief` -11.2 ± 27.2. Its whole-game ledger row is
mildly positive (adjusted +0.63 ± 0.35 pp over 12,673 seats on).

Added to the full seat and firing on no board (0 of 100 seeds diverged):
`relief-targets-the-siege`, `relief-column-marches`,
`threatened-city-reserve-2`, `wounded-out-of-reach-2`, `siege-is-progress-3`.
Already in the genome: `close-as-a-body`, `screen-the-shooters`,
`defend-where-you-stand`, `standing-still-is-a-risk`, `promote-when-wounded`,
`capture-go-or-stand-down-2`.

On the planner-only stack, dropping each deployed planner gene: `strike-reach`
-47.3 ± 10.1 (keep), `safest-stand` -8.6 ± 4.6 (keep), `swap-rotation-2` and
`fire-plan` no effect; `battle-planner-3` in place of `-2` reads -33.9 ± 8.5 on
`the_storming` for the Domination seat (keep v2).

## What was decided

`anvil` is added to `deploy/live-force-on.txt`. `doomed-blow-veto-2` was
withheld on the pinned live tree on the earlier arena reading; the full-genome
reading confirms it. Main's deployment genome is regenerated from whole-game
screens and is not hand-edited here.
