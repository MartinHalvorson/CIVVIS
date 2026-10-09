# City capture during unit recovery

The public preservation module initially blocks every unit in its recovery
set until it reaches full health. Its finisher admission also rejects every
recovering attacker before considering the already existing survival bounds.
The thirteen-control unchanged-production baseline now proves both defects.
The candidate permits the capture for an unassigned land melee unit while
retaining recovery memory for its other orders.

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
from two tiles away, a ranged strike, and peace. Additional controls check an exposed enemy reply at post-exchange health,
a lethal burning tile despite a guaranteed capture, and an attack with no
movement. The first pushed baseline registered ten tests; the corrected suite
now registers thirteen. Candidate results are pending. The independent escort work in
PR #4011 retains its separate tests and joint-movement policy. Its changed
hunks are not rewritten by this draft's finisher-admission investigation.

The first test checkpoint's quality gate reports one unused trait import in
the new test module. That import is removed without changing any assertion;
the warning is separate from the expected recovery-admission failures.

The first behavioral baseline stops before the intended capture cases on two
fixture errors. Arena mode forces war despite clearing the relation, so the
peace control now selects the ordinary Continents mode before clearing it.
The original ordinary-move destination coincides with a preservation retreat;
the corrected control uses a longer legal move and verifies its original
arrival. Four other new controls pass before fail-fast stops the first run.
The corrected baseline must reproduce the intended recovery failures before
any production implementation is accepted.

No native lane, pin, policy, installed mod or private source is changed.
Synthetic regression results will not establish a native capture or win-rate
gain. Adoption, completed native readbacks and matched outcomes remain needed.

The complete twelve-test local baseline on `fa8307aa5` exits 101 after all
cases execute: nine pass, both intended recovery cases fail, and the added
enemy-reply fixture fails its inappropriate ordinary-finisher assertion.
The existing danger field shields city garrisons from unit strikes. That
fixture now directly checks the wounded attacker on its exposed approach
tile, and an added burning-tile control verifies a lethal 75-damage hazard.
The thirteen-test baseline still leaves production behavior unchanged and
must report precisely the two intended failures with eleven controls passing.
A prospective exception will require survival both at the captured city and
on the exposed approach at post-exchange health; it will retain recovery
memory and all existing retaliation, legality and whole-turn safety checks.

## Candidate and validation

The complete thirteen-control baseline on `81c70e045` exits 101 with exactly
the two intended failures and eleven passing controls. Registered Rust/test/
Cargo hashes remain unchanged. Independent CI `37904606128` also reproduces
the erased capture and passes all eleven controls; its fail-fast run stops
before scheduling the finisher case. Earlier fixture and quality failures
remain separate from these verified behavior failures.

The candidate independently replays one adjacent land melee capture, optionally
followed by fortification. The observed enemy city must already be unwalled,
at zero or one health, and at war. A legal replay must change ownership and
leave the actor alive on that city. Its conservative post-exchange health must
survive both the captured-tile reply and an exposed original-position probe
which retains every originally observed enemy. It therefore cannot rely on
city garrison shielding or an observed-world elimination to license the
exception. The whole-turn fixed point also checks that the city remains owned
and the actor still occupies it after the remaining proposed orders.

Recovery memory remains set. Ordinary movements, ranged attacks, approaches,
standing walls, uncertain captures, lethal host retaliation, exposed replies,
lethal hazards and exhausted attackers keep the existing recovery behavior. A
living assigned Settler guard also keeps recovery; a fourteenth candidate
control checks that it does not take this exception. The original capture
control additionally checks retained recovery memory after physical occupation.

Candidate focused local, complete Rust, cost and final integration checks are
pending. Main integration will preserve and revalidate all twenty-one escort
controls from the merged PR #4011. No native adoption or win-rate gain is
credited by this synthetic candidate.
