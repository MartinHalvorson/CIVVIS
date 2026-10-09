# Native upgrades keep unit assignments

The fresh-board bridge remaps AI memory through native unit IDs. In
`civvis-20261009T131304Z`, the two Spearmen 1769488 and 3145749 legally
upgraded on turn 103. They became Pikemen 5242900 and 5308451 at offset
(32,16) and (34,16). Their experience remained 9 and 0 respectively.
The previous mapping drops both soldiers because their old native IDs no
longer appear, despite the bridge separately recognizing the upgrades.
Siege takers, shooter posts, recovery and escort bindings all consume that
same unit mapping. This establishes a continuity gap, not a win-rate gain.

The bridge now checks issued upgrades before prior-turn order settlement
consumes their state frames. An alias requires a later frame within one
turn, the same seat, an absent old ID, the host's exact offered successor,
a previously unseen replacement ID at the same position, and unchanged
experience, level, promotions and formation. A blocked host offer,
confirmed combat death, competing replacement or competing old owner
prevents an alias. Ordinary native IDs retain priority. The alias only
extends the existing fresh-board memory handoff; it does not select an
upgrade or change a game rule. Missing metadata conservatively drops the
assignment. Movement after the last exported position can also prevent a
match, so coverage is intentionally partial.

Primary host references are the shipped
`Base/Assets/UI/Panels/UnitPanel.lua:468-483`, which reads the command's
`UnitCommandResults.UNIT_TYPE`, and
`Base/Assets/UI/UnitFlagManager.lua:1810-1815`, whose `OnUnitUpgraded`
callback finds the unit by the supplied ID. The concrete old/new ID
relationship above comes from the recorded native states and upgrade
receipts, rather than an assumption about the callback's ID.

The regression fixture retains the observed IDs, positions and experience
from those two upgrades. Additional cases reject conflicting evidence,
missing observations, reused IDs and confirmed deaths while preserving
ordinary survivors. Local and native validation results are recorded in
the PR after the commands finish. Winning native verification games remain
the outcome measure; no outcome improvement has been demonstrated.
