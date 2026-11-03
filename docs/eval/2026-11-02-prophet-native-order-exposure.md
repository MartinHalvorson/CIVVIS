# Native Prophet handover changes a peace request at turn 40

The repaired `religion-lane-hands-over` activated in native game
`civvis-20261102T232731Z`: configured Immortal Mali, assigned Religion.
At turn 39 the seat had no founded religion or pending Prophet. At
turn 40/frame0, the host added the Prophet class to its exhausted list
while still listing only two founded religions. The live controller
changed its primary target to Science. Its executable was the recorded
`3ff83fba4` snapshot (SHA begins `3a26064884da`).

The actual process argv and cwd were observed while it was running. Two
original-executable controls reproduce all 134 complete native replies
through turn 60, including frame identities and every ordered input-read
byte. A preliminary turn-40 qualification also passed twice. The later
60-turn endpoint was fixed before comparing counterfactual orders.

The action comparison keeps those exact identity/read gates while allowing
non-actuation metadata, such as the intended target, to differ. The native
driver records the decision metadata and extracts the five order fields
(`civ6_brain.py:794–805`); it separately logs the returned note
(`1395–1408`). The replay asserts that the captured orders use only those
fields. An additional observed-program control matches all 134 order lists.

| Program and flag | Target at first changed order | Turn 40/frame1 |
|---|---|---|
| Observed `3ff83fba4`, on | Science | Requests peace with player 1 |
| Earlier `bedefb280`, on, two repeats | Religion | Omits that request |
| Observed program, flag off | Religion | Omits that request |

Each of the three initial counterfactual runs matches the first 88 native
order lists. On request 89 (turn 40/frame1), its complete ordered input
reads and frame identity still match, but it omits exactly one order:
`peace / subject=1 / MAKE_PEACE / x=0 / y=0`. Every other order matches,
including its position in the list after removing that one request.
Realized treatment profiles are read back; the off variant removes only
the handover flag. All variants stop at their first different host order,
before using an unchanged captured world after an altered action.

The native handler independently records `peace_request`, target 1,
`submitted=true`, `threw=false` at turn 40. This establishes submission,
without establishing accepted peace or saved production. The actual game
later loses to a rival Technology victory at turn 192. This is action
exposure evidence; strength remains unmeasured. Its assigned lane remains
Religion in the separate Science fuel pilot census.

Replay delivery remains a limitation. Two later parameterized repetitions
hit the unchanged 60-second request deadline at turn 36/frame0 and turn
21/frame1. Both failures are retained. An experimental delivery variant
leaves the last recorded snapshot as a regular file between requests; its
two original full-payload controls pass, but a later variant still times
out. The underlying cause is unresolved. The public reproducer defaults
to the original delivery mode and fails closed on deadline, identity or
read-byte disagreement. Failing variants do not count as action comparisons.

The companion JSON verifies the 934,248-byte archive and all 102 member
hashes: original observations, sealed inputs, full native replies, commands,
genomes, submitted-request readback, terminal summary, valid comparisons,
both failed repetitions, and source/provenance receipts. The exact recorded
experimental source hash was recovered and included. Reproduction requires
the matching executables and recorded cwd; disposable mirrors receive all
outputs. No host orders are published by the experiment. There is no default
change, evidence-ledger ingestion or use of reserved model seeds.
