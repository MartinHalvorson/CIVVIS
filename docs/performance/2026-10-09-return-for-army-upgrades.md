# Return obsolete combat units to upgrade ground

The fixed Emperor verification attempt `civvis-20261009T113012Z` ended
in a Culture defeat at turn 178. Warrior `131073` remained obsolete abroad
from turn 90 through 167, with 63 first-frame territory-block readings;
Archer `3080205` had 66 readings. These are repeated observations, not
separate failed commands. The run issued 17 genuine upgrade orders.
Archer `2424846` became Crossbowman `4456473` at native `(31,24)` during
turn 94; turn 95 verified it. Ordinary legal upgrades already work.

The immutable turn-100 prefix models ten route edges from each persistently
obsolete unit to owned ground at native `(27,21)`. Each native upgrade costs
60 Gold, and the Warrior's Man-at-Arms requires ten Iron. The observed
cash and material cover the bills. By turn 120, the Warrior's nearby route
is unavailable and the Archer's takes nineteen edges. This supports a
return hypothesis, not a counterfactual execution or victory claim.

## The first hypothesis did not change captured decisions

The first implementation passed the complete governed public suite:
4,470 library, 14 main, 247 orders, 17 integration and four viewer tests.
Yet strict same-prefix comparisons changed zero complete replies in both
captures: 405/405 identical on `114819`, and 417/417 on `113012`.
Historical-control agreement was only 320/405 and 322/417 respectively;
both comparisons failed their historical gate. These are diagnostic
comparisons and provide no native execution or winning credit.

The turn-100 journal names Nazca, owned by minor player 9, as the current
siege objective while the major Russia is also at war. The train has no
guns, one wall-DPS unit and an unready damage budget; its muster has been
closed since turn 88. The first policy required a named major objective,
ran after the siege reservation and excluded any enemy city within six
hexes. Those gates explain why its positive synthetic tests were insufficient.

## Revised decision rule

A Domination unit can prepare an upgrade during a named major offensive or
an ongoing major war even when the immediate objective is a minor. The
improvement must be at least 15 strength; present cash must cover the host
bill plus the existing 30-Gold and maintenance-deficit reserve. Native
successors, Gold quotes and material bills take priority over modeled
direct upgrades. A territory refusal triggers independent cash and material
checks. Other native refusals remain final, and movement never grants
permission to upgrade in the same turn.

The return checks up to eight nearest owned border plots and requires an
actual route of at most twelve edges. It spends available movement along
that route and stops to wait for a fresh legal upgrade turn. Recovery,
appointed war packages and Settler guards retain priority. The hook precedes
the ordinary siege action. Reserved units and assigned siege members stay
with their controllers except for a named finisher in Stage for at least
four turns, assessed within the previous turn, whose matching land group
still cannot meet the siege damage budget. Fresh staging, ready assaults,
other siege phases and stale assessments retain their units.

A threatened home city prevents a return. Enemy military units within six
hexes, any hostile unit within one hex, and hostile cities or known enemy
Encampments within three hexes prevent movement from the current or next
plot. Sea, air and Recon units retain their tasks. These bounds are policy
choices; they do not prove optimal scheduling, unseen-route safety or
future affordability.

## Validation checkpoint

Eight focused tests cover return through a funded legal upgrade, a native
successor that skips an intermediate unit without bypassing its refusal,
owned-territory upgrades, missing material, blocked routes, nineteen role
and budget exclusions, stalled versus protected siege assignments, and a
minor objective during a major war. The stalled-stage regression fails on
the first implementation as intended. Complete validation of the revised
source is in progress. Its native adapter has one extra test fixture field
(`short_since`); the production return helper is byte-identical.

The verification challenge, retirement rule and assigned target are
unchanged. Native wins remain the success criterion. No native winning
gain or runtime improvement is claimed for this policy.
