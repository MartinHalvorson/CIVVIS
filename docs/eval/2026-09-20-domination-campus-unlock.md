# Unlock campuses during a domination opening

Two recent native King attempts left Writing late in an expanding empire:
`civvis-20260920T065948Z` founded its second city on turn 19 and learned Writing
on turn 35; `civvis-20260920T080404Z` reached two cities on turn 20 and did not
learn Writing until turn 65. The latter researched Sailing and Shipbuilding
before Writing, preventing any Campus construction during that interval.
This is a research-order observation, not proof that a Campus would have been
built immediately or that a different order would win the game.

A domination-targeted empire with at least two owned cities now selects Writing
as an opening goal while it remains unknown. The existing prerequisite picker
walks Pottery when needed. This goal runs behind urgent defense, committed war
and air research, siege capability, and luxury connections, ahead of optional
bargains and generic technology scores. It does not interrupt active research.
Other victory targets and the one-city opening keep their current behavior.

The full research regression fails on baseline: the two-city empire chooses
Bronze Working despite Writing being available. Controls exercise prerequisite
selection, rush and ranged-defense priority, active research retention, and the
policy's victory-target/city-count/completed-unlock boundaries.

## Replay artifacts

The baseline production code is `cf52d8c64`. Both replay executables read a
growing prefix of `/tmp/civvis-native-siege-expansion/events.jsonl`, the same
303 frozen frames through turn 105/frame 0 used for #3609. They run persistent
serve sessions with the native verification batch's 19 forced genes and
domination target. Driver: `/tmp/civvis-campus-frame-replay.py`; artifacts:
`/tmp/civvis-campus-unlock-replay`.

A replay can establish changed research orders. Its later host technology and
city states remain recorded observations, so it cannot establish faster actual
research, completed campuses, higher science yield, captures or victory rate.

This changes AI research selection only. Engine crash soak is inapplicable.
