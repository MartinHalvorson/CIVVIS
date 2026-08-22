# The one remaining measured failure, and where the fix goes

## The gap (repo-recorded, not mine)
73 city depletions → **33% with a melee taker ready** → 75% conversion → 18
captures. Two thirds of siege work opens a city nobody can walk into. Result:
0.33 cities captured per game, **zero capitals ever**, no peace treaty in 12
full-length games. Every other subsystem I bounded is a *saturation*; this is
the only measured *failure*.

## Why it is still open
- **#333** "Put a taker next to a city whose garrison is spent" — 33 h since
  its last commit, which is still only the placeholder claim. Two coordination
  notes from me, unanswered.
- **#366** oracle/`taker` grant — 30 h stale.
- Meanwhile main merged **four** PRs touching `src/ai/advanced.rs` in 20 h
  (#487, #492, #486, #480), so concurrent edits there are demonstrably normal.

## Two candidate fixes, and they are different
- **(A) #333's**: move melee *toward* a city about to fall. Positioning.
- **(B) gate the shot**: do not fire the depleting ranged/siege attack unless a
  melee taker is already adjacent. Sequencing.

(B) is not what #333 claims, and is arguably better: it never spends the
depletion window it cannot use. Its risk is real and must be measured — holding
fire lets the city heal **+20/turn toward a cap of 200**, so a badly-tuned gate
wastes the siege entirely rather than mistiming it.

## Where (B) goes
`src/ai/advanced.rs`:
- candidate generation ~**10091**: `Action::Attack` is pushed when
  `spec.is_melee_capable() && distance == 1`; ranged/siege candidates are
  pushed just above.
- scoring ~**9281** `tactical_attack_value`, which already prices a capture at
  `520 + pop*14 + districts*24 + wonders*45 + capital*180 + target*100`.

The gate belongs in `tactical_attack_value`, not in candidate generation:
**subtract** from a ranged/siege attack that would leave the city depleted with
no adjacent friendly melee. Scoring it down rather than removing it keeps the
shot available when nothing better exists, which matters because
`advanced_city_strikes` loops until no candidate applies.

## How it gets measured
Not on a proxy. `search_dose`-style paired construction on **wins**, plus a
fires-check counting how often the gate actually suppresses a shot — an
ungated run and a gate that never fires produce identical output, and this
work has already shipped one ablation that measured nothing and read like a
result.

## Status
Not started. Replication of the `strategic_deep` promotion null is in flight
and takes priority; starting 17k-line-file surgery while that is pending and
while four other evals contend for the box would be scattered.
