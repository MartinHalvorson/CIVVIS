# Russia reaches the Apostle unlock before a later government

Native control: `civvis-20261109T154422Z`, Russia/Peter, Deity,
six-player Small Continents, Online speed, Gathering Storm. Its frozen decider
was `4a05dd664c00023e5a35cab1574a2ad21e03352d`, with SHA-256
`5ab52e306402a9471bd072832e0b9d0289b4e8912e760523b550d7d1e006ef0a`.

The seat founded Buddhism at t19 and built a Shrine at t32. The captured
t49 frame has Political Philosophy, no active civic, one city, 183 Faith,
15 Faith per turn and 3.89844 Culture per turn. The original native decision
selects Games and Recreation. Replaying that sealed input in resumed one-shot
mode reproduces the civic proposal and explains it as the most valuable step
toward Divine Right. The same proposal remains in a diagnostic build of
`872b048d043f21284504ee9426e0a125a9638558`, which includes the queue and lost-source
fixes. The Religion civic-goal source is unchanged between that revision and
this task's base.

The native sequence studies Games and Recreation from t50, Defensive Tactics
from t60, Drama and Poetry from t66, Mysticism from t75 and Theology from t77.
Theology completes at t81. Games and Recreation and Defensive Tactics are not
Theology prerequisites. This records the observed detour; it does not estimate
how many turns a different path would save under changing Culture and combat.

## Policy

The existing opt-in `religion-seat-fields-apostles` treatment reaches Theology
once a Religion-target seat has founded and lacks that civic. The existing
civic-goal seam puts this ahead of the later-government ladder, while keeping
the first-government and urgent civic priorities already above it. Ongoing
civic research continues. The separate Reformed Church treatment still owns
the later goal and is not enabled by this change.

No civilization-specific preference or new default is introduced. The same
Religion target and founded-religion gates cover Russia and other founders;
other victory targets and nonfounders retain their paths. The goal retires
when Theology is known. Legal prerequisite selection remains in the normal
civic chooser.

## Native rule and verification

Installed Firaxis `Base/Assets/Gameplay/Data/Buildings.xml:111` assigns
`BUILDING_TEMPLE` `PrereqCivic="CIVIC_THEOLOGY"` and
`PrereqDistrict="DISTRICT_HOLY_SITE"`.
`Base/Assets/Gameplay/Data/Units.xml:924` contains
`<Row Unit="UNIT_APOSTLE" PrereqBuilding="BUILDING_TEMPLE"/>`.
Theology opens the Temple needed for the treatment's Apostle purchases.

The t49 sealed native input is 4,380,422 bytes with SHA-256
`966ff2ef3b7f2e616e1a7deba67d5b2f4455d43ed2f5d98584ba141d5c78f1c5`.
The resumed replay is a planning diagnostic. It does not reproduce the full
persistent native history and does not establish acceptance, timing gains or
religious wins.

Focused regressions must exercise the actual civic chooser against the later
ladder, including an Expansion posture, and cover direct Theology selection,
first-government priority, active study, opt-in and lane/founding gates, and
the separate Reformed Church handoff. Required optimized CI validates the
candidate. No earlier native Temple, Apostle or victory is claimed by source
inspection or a passing model test.
