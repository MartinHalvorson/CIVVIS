# Native governor exchange continuation

The archived Pingala v2 witness at turn 154/0 contains two model assignments,
Pingala to native city 65536 and Magnus to native city 131073. Its emitted
orders contain only Pingala's assignment. The existing one-governor-operation
filter discards the return leg. A changed proposal therefore did not establish
that the complete exchange was emitted or executed.

The bridge now retains only a reciprocal pair of assignments to two observed,
owned governor posts. It still emits one governor operation per frame. A newer
native state must show the first governor at the destination before the bridge
releases the second governor's return assignment. Pending memory survives
fresh-board reconstruction. Appointment and promotion follow-ups still require
replanning rather than being cached.

The continuation cancels on changed city identity or ownership, missing or
neutralized governors, an unexpected post, a third governor occupying the return
city, backward frames, or more than three turns without completion. It retries
a released return assignment at most once per turn. Nongovernor orders retain
their relative ordering.

Native reference: the shipped
`DLC/Expansion1/UI/Additions/GovernorAssignmentChooser.lua:377-380` requests
`PlayerOperations.ASSIGN_GOVERNOR` with the governor, city owner and city ID;
lines 389-392 handle replacement. This adapter requires readback of that
assignment, not establishment, before issuing the return leg.

This is an order-transport repair. No new gene, rating, default-policy promotion,
native execution result, or strength claim is introduced. Unit fixtures emulate
readbacks explicitly; they are not native game captures. The prior shared-control
candidate lost to rival Science on turn 179 and produced no Pingala exchange;
that completed arm is not rerun or reclassified by this repair.

Regression tests cover first-move readback, same-frame holds, bounded retries,
expiry, changed cities and governors, earlier title operations, and rejected
nonreciprocal pairs. The full optimized suite, separate doc tests, and source-bound
native binary are required before integration; results are recorded in PR #4595.
