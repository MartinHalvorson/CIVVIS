# Reachable siege firing posts

Public firing-post assignment ranks clear, free, traversable tiles by formation
position, exposure and geometric distance, without checking the approach route.
Melee-post assignment already checks `siege_route_step`. This investigation
registers a production-unchanged reproduction before changing gun assignment.

The fixture offers a nearby visible firing pocket whose only entrance crosses
the excluded inner city ring, and a farther firing tile with an open route.
Catapult and Archer cases require assignment to the reachable tile, actual
movement through the normal recorded tactical step, and a legal wall-damaging
shot after arrival. A third case seals every approach and requires no firing
post. Existing route, line-of-sight, current-position, staging, support and
capture tests remain in the same discovered siege suite. Baseline results are
pending; production assignment is unchanged.

## Native motivation and limits

The completed external Gran Colombia four-player game
`civvis-20261009T081707Z` loses to Culture on turn 160 with no city captures.
It records six friendly ranged district attacks, including two on Whanganui.
Its why journal repeats gun approaches that stop short of their posts. The 49
selected entries include repeated frames: 12 simulated movement refusals, 10
out-of-movement stops, nine no-further-route stops and 18 no-route/no-pass-through
stops. They are not 49 distinct native failures or proof of an alternative
successful firing operation.

Private native revision `5df81886b` has a substantially different siege
controller, including pass-through, queue and entry guards absent from public
main. Both versions choose candidate firing positions without the route check,
but the native failure's exact cause is unproven. Source and read-only evidence
remain in `native-081707-siege-source-comparison-receipt.json` and its linked
journal receipt under retained goal artifacts. No native lane, pin, private
source, installed mod or process is changed. Synthetic results will not prove
native adoption, completed city pressure or a Domination win-rate improvement.
