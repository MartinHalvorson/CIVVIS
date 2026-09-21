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
