# Bring siege bombers into city range

In native run civvis-20260921T161428Z, the first Bomber was based at native
(23,23), eleven tiles from Edessa, with an attack range of ten. On turn 154
the recorded board offered bases within city-strike range, and a healthy
Cuirassier was already next to the city. The air planner always preferred
any positive current mission to rebasing, even a mine-bombing mission while
the siege objective remained out of range.

The separate exporter repair (#3674) makes that mine-bombing mission reach
the host. This change addresses the choice of mission: an explicit
Domination campaign can rebase for a more valuable city strike.

The override applies only during Conquest, against the active enemy major's
city, with no threatened home city and a healthy, non-embarked land melee
capture unit within three tiles. The bomber must currently be outside city
range and the selected legal base must put it within range. On a speculative
copy, the planner applies the rebase and allows one later sortie, then uses
the existing air-strike valuation to compare that city attack with the best
current mission. It keeps the current mission unless the city strike scores
higher. Other victory lanes, home defense, unsupported sieges, and more
valuable immediate kills retain the existing choices.

This is a static forecast, not a simulation of the opponent's intervening
turn. The real aircraft receives only a rebase order, never a same-turn
rebase-and-strike sequence.

## Evidence

The initial regression fails on the previous choice (AirPillage rather than
AirRebase). Six focused tests cover the non-capital siege, absence of a melee
capture unit, a valuable immediate kill, another victory lane, an ineffective
city strike, and home-defense precedence. The existing airstrip-priority
regression also passes.

An initial paired replay on the production behavior of 12a52ccea covers all
497 recorded decision frames through turn 180/2. It moves the first bomber's
rebase choice forward to 153/1 and also rebases the second bomber on 156/0.
Nine exported frames and twelve internal-action frames differ; some are
counterfactual retry/receipt consequences because the original recorded
future never executed the new rebase requests. This comparison predates
integration of #3674 and is not evidence of native execution or a victory.


## Final integrated comparison and validation

The final paired replay includes the #3674 air-pillage exporter on both sides.
Baseline production source is 6810f5e00; candidate source is 4032a8ad9. Both
consume the same 497 recorded decision frames. Twelve exported frames and
twelve internal-action frames differ. The first bomber exchanges mine bombing
for rebasing at 153/1, 154/0 and 155/0; the second exchanges current attacks
for rebasing at 156/0 and 157/0. Across the recorded history, five rebase
requests replace seven air attacks and three later rebase requests; these
counts also include retry-state effects and do not represent five executed
rebases. The old future never performed the new requests, so replay receipts
cannot establish native success, subsequent city damage, captures or victory.

Final local validation passes: 4,091 Rust tests (53 ignored), all six focused
regressions, the existing bomber airstrip-priority regression, 14 append tests,
and eight four-player 180-turn soak games (seed 367600). Scoped formatting,
changed-line Rust quality against 6810f5e00, and diff whitespace checks pass.
The completed native game used older code and ultimately lost to Culture on
turn 211. This change has not yet demonstrated a native Domination win.
