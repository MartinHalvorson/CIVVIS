# City capture during unit recovery

The public preservation module initially blocks every unit in its recovery
set until it reaches full health. Its finisher admission also rejects every
recovering attacker before considering the already existing survival bounds.
This draft first adds a separate regression suite without changing that
production behavior.

## Native evidence and limits

The completed Gran Colombia four-player native game
`civvis-20261009T054512Z` lost to Culture on turn 223. Its summary records 80
unit kills and no city captures. A read-only combat audit accounts for those
kills: 45 barbarians, 18 player 3, 16 player 2 and one player 4. All 36 recorded
friendly attacks on districts used Bombers; city centers use the district
event type. Fourteen attacked Gao, a non-capital Malian city at native
`(18,22)`. Bomber combat readbacks reach zero city health on turns 222–223,
while the state exports still give ownership to Mali.

Modern Armor `15400963` requests two attacks on Gao on turn 222. Both next-turn
verdicts are `target_unharmed`, with no attributed melee combat event. On turn
223, its exported orders fortify it and then move it away. The planner logs
projected captures, followed by preservation at 73 health with an upper reply
of zero. Its prefilter action payload is not recorded, so this evidence does
not establish the exact cause of either failed attack or prove an alternative
native operation would have captured the city.

The native revision was `a66650568cff2b2d6c4ccb2db59a1c55fb72256f`.
Its `src/ai/advanced/unit_preservation.rs` is byte-identical to the public
module at investigation start, SHA-256
`fd3d54fdec3778abcfb629093900b0468f9e6b1ac154ba39eeeb8708c41da576`.
The rest of the controller is not source-equivalent. Original events,
decisions, why log and summary hashes remain unchanged in
`native-054512-capture-gap-receipt.json` under the retained goal artifacts.

## Registered reproduction

The fixture starts a 73-health Modern Armor next to an unwalled enemy city at
one health. A second enemy city prevents the capture from manufacturing safety
by eliminating the observed opponent. The native strike strength preview
`95 versus 92` is retained as a conservative retaliation bound. The ordinary
finisher already considers this capture survivable; adding the recovery latch
should not erase it in either finisher admission or the final order filter.
The preserved action must legally change ownership and occupy the city, while
the input board remains unchanged.

Controls retain ordinary recovery movement, nonrecovering captures, standing
walls, a city that cannot be captured, lethal native retaliation, an approach
from two tiles away, a ranged strike, and peace. Two additional controls retain the enemy-reply survival bound
when a capture itself is guaranteed and reject an attack with no movement.
The pushed baseline registers ten tests; the final candidate suite registers
twelve. Baseline and candidate results are pending. The independent escort work in
PR #4011 retains its separate tests and joint-movement policy. Its changed
hunks are not rewritten by this draft's finisher-admission investigation.

The first test checkpoint's quality gate reports one unused trait import in
the new test module. That import is removed without changing any assertion;
the warning is separate from the expected recovery-admission failures.

No native lane, pin, policy, installed mod or private source is changed.
Synthetic regression results will not establish a native capture or win-rate
gain. Adoption, completed native readbacks and matched outcomes remain needed.
