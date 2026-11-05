# Early peace gate diagnosis, 2026-11-05

Native Deity Online Science run `civvis-20261105T151911Z` lost at turn 163.
The turn-59 siege journal reported Kraków unwalled, strength 71 against a
55 bill, zero of four units staged, and an archer fourteen tiles away with
no safe approach. At turn 60 the plan wanted five cities and held four.
These observations motivate checking the peace veto; they do not establish
which predicate actually blocked its offer.

`war_policy_siege_against` compares the entire land field army to a siege
requirement. The siege train journal describes a different, assigned force.
The new deterministic Science fixture places its entire combat army more
than ten tiles from the enemy city. It verifies that paper feasibility
still suppresses otherwise eligible early peace and records that exact
blocking predicate. This is a constructed regression, not a reproduction
of the native game's private controller history.

The controller now reports the first failed early-peace predicate at Detail
level, including its observed declaration turn and city-count goal. The
predicate order and eligibility remain unchanged. An eligible reading
explicitly says later diplomacy vetoes still apply; it does not claim an
order was sent or peace accepted. A missing observed declaration is not
labelled an AI declaration: wars already running on first observation are
also excluded by the existing rule.

Do not replace this veto with `staged_campaign_units`: that helper is for
peacetime declarations and excludes positions within two tiles and target
territory, including active wartime attackers. Do not simply delete the
feasibility guard: the original module records a negative simulator probe
when it was removed.

Next measurement: capture this predicate trace in an ordinary native game
from before a war begins. Establish the binding predicate and actual peace
submission/readback before testing a narrower Science-specific veto. No
strength gain or default-policy promotion is claimed here.

Local validation is recorded in the PR after the focused suite completes.
Native raw evidence and exact-source manifests remain in the operator's
`civvis-tactics-results/2026-11-04/wartime-campus-diagnostic-151911/` archive.
