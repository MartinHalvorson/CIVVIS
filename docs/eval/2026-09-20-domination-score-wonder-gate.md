# Keep Domination production out of score-only wonder races

## Native evidence

In King / Gran Colombia / Domination run `civvis-20260920T132244Z-cont1`,
Popayán begins Eiffel Tower at turn 155. The journal prices it at 76 for the
Conquest plan. At turn 162 it has 193 of 810 production invested and needs
another 23 turns. The empire is at war, has made no verified military city
capture, and is still assembling its siege force.

The strategic-wonder evaluator deliberately assigns no special Conquest value
to Eiffel Tower. The separate host-only `live-wonder-race` bonus nevertheless
opens a generic score race for this explicitly targeted Domination seat. The
arm already prevents this for an explicit Science target. Its bargain and
score-tally alternatives can also open a race independently.

## Diagnostic probe

A frozen prefix of cont1 includes 124 decisions from turn 131/frame 0 through
172/frame 2. On the same `152a392a0` binary and native configuration, disabling
`live-wonder-race` changes 53 exported frames and all 124 internal action
streams. Both replays finish in 28.29 seconds. The comparison excludes
`order_failed`, `order_verified`, and `turn_verified` telemetry.

At turn 155/frame 0, the baseline requests Eiffel Tower. The disabled-bonus
controller instead requests a Cuirassier there, plus Artillery and Pike and Shot
elsewhere. This broad diagnostic also changes existing wonder queues, so it is
not the proposed production change. It motivates testing a target-specific gate
that preserves the bonus for a queued or previously invested wonder.

These are alternate requests on frozen native observations, not host execution,
new units, captures, or wins.

## Candidate and validation

Pending the narrow implementation, focused controls, and replay comparison.
