# Return obsolete combat units to upgrade ground

The fixed Emperor verification attempt `civvis-20261009T113012Z` ended
in a Culture defeat at turn 178. It supplies a decision-making hypothesis,
not a winning result: Warrior `131073` remained unmodernized abroad from
turn 90 through 167, with 63 first-frame territory-block readings; Archer
`3080205` had 66 such readings over that interval. These are repeated unit
observations, not separate failed commands. The same run issued 17 genuine
upgrade orders. For example, Archer `2424846` became Crossbowman `4456473`
at the same native position `(31,24)` during turn 94, and turn 95 verified
the upgrade. Ordinary legal upgrades already work.

The immutable turn-100 terrain/state prefix models ten route edges from
each of the two persistently obsolete units to owned ground at native
`(27,21)`. Both native upgrade quotes cost 60 Gold; the Warrior's Man-at-Arms
offer requires ten Iron, and the Archer's Crossbowman needs no material.
The treasury and strategic stock cover these bills. By turn 120, the same
Warrior's nearby home route is unavailable and the Archer's takes nineteen
edges. This motivates an earlier return; it does not establish that the
counterfactual route would execute or win the game.

The policy brings an otherwise unclaimed land combat unit back before its
ordinary campaign march when a named Domination offensive exists, the
offered improvement is at least 15 strength, and the current treasury covers
the bill plus the existing 30-Gold and maintenance-deficit reserve. Native
successors, Gold quotes and material bills take priority over modeled
direct upgrades. A territory refusal is followed by separate affordability
and resource checks. All other native refusals remain final.

The unit returns to owned border ground rather than a City Center. It checks
up to eight nearest candidate plots and requires an actual route of at most
twelve edges. It uses available movement along that route and stops on
arrival to wait for a fresh legal upgrade turn. It leaves appointed war
packages, reserved finishers, linked formations and Settler guards to their
existing controllers. Recovery, raids, doctrine actions and city defense
precede this hook. Visible or remembered enemy military units and enemy
cities within six hexes of the current or next step prevent the return.
Sea, air and Recon units retain their existing tasks.

These bounds are conservative policy choices. They do not prove optimal
upgrade scheduling, future affordability, unseen-route safety or native
execution. The verification challenge, retirement rule and assigned target
are unchanged. Native wins remain the success criterion.

## Validation checkpoint

Six new tests cover the campaign fallback through arrival and a funded legal
upgrade, a native successor that skips an intermediate unit without bypassing
its stale refusal, ordinary owned-territory upgrades, missing strategic
material, blocked routes, and seventeen role/permission/budget exclusions.
Focused local validation is in progress at this checkpoint. Complete governed
local tests and final CI must finish before shipping. No native winning gain
or runtime improvement is claimed for this policy.
