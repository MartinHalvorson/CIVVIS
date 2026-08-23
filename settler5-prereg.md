# Pre-registration: settler_min_pop = 5

Written BEFORE the run, so the verdict cannot be chosen after seeing it.

## Claim
Raising `settler_min_pop` from the shipped 2 to 5 — a city must reach pop 5
before it builds a Settler, i.e. slower and taller expansion — improves the
agent.

## Evidence so far (score share, the SELECTION proxy)
| seed | maps | edge |
|---|---|---|
| 1200000 | 20 | +0.0283 ± 0.0187 |
| 1300000 | 20 | +0.0184 ± 0.0237 |
| 1600000 | 80 | +0.0174 ± 0.0069 |
| pooled | 120 | **+0.0187 ± 0.0062 (3.0 SE)** |

## The run
`policy_eval --players 4 --maps 120 --seed 1700000 --treatment legacy
--control legacy --gene settler_min_pop --value 5`

Both arms use the legacy policy deck, so the null policy-deck change is not
confounded in. 500-turn games, paired, seat-mirrored, fresh seed.

## Decision rule, fixed now
- **PASS** requires map directions FOR > AGAINST with sign p < 0.05.
- Terminal score is reported beside it and is NOT part of the rule. Wins and
  score measure different things; score is what nominated this and cannot also
  ratify it.
- Anything else is a null and the shipped value of 2 stands.

## What would refute the claim
Map directions at or below parity. A 3.0 SE score-share effect that does not
convert to wins would mean the change grows a better economy without winning
more games — a shape `docs/EVAL.md` records for lane-routing changes and one
this repository has been fooled by before.
