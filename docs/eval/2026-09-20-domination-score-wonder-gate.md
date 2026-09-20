# Keep Domination production out of score-only wonder races

## Native evidence

In King / Gran Colombia / Domination run `civvis-20260920T132244Z-cont1`,
Popayán begins Eiffel Tower at turn 155. The journal prices it at 76 for the
Conquest plan. At turn 162 it has 193 of 810 production invested and needs
another 23 turns. The empire is at war, has made no verified military city
capture, and is still assembling its siege force. Later in this continuation, a Tank
captures Liverpool at turn 197 and the controller requests razing it; the city
appears in our roster that turn and disappears at 198. The run ultimately loses
to Georgia's Culture victory at 225. This verified military capture occurs
outside the frozen prefix below and before this candidate existed, so it is
not evidence for the candidate.

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

The three generic score gates now refuse new races for an explicit Domination
target. A queued or previously invested wonder retains the original valuation.
Science's existing guard and the Culture, Score, and strategic-wonder lanes keep
their existing rules.

The isolated comparison uses baseline `e6def77d1` and candidate `6c32bda01`,
both including the independently shipped siege-column movement fix. The earlier
`152a392a0` baseline belongs only to the diagnostic probe above. Both isolated
arms complete all 124 frames: current baseline 30.68 seconds, candidate 25.08
seconds. These are concurrent functional runs, not a performance benchmark.
Exactly 62 frames change exported orders after excluding telemetry. All 124 raw
internal action streams differ; model IDs and stateful planning are not a
separate count of meaningful host changes.

| Observed decision | Baseline request | Candidate request |
| --- | --- | --- |
| 131 / 0, Quito | Potala Palace | Builder |
| 155 / 0, Popayán | Eiffel Tower | Cuirassier |
| 155 / 0, other cities | No corresponding unit request | Artillery and Pike and Shot |
| 171 / 0, Cartagena de Indias | Országház | Tank |

The prefix contains 16 baseline wonder requests (including repeated and
next-queue requests) and none in the candidate. Artillery production requests
increase from 17 to 24, Cuirassier from 2 to 13, and Tank from 11 to 18. These
counts include retries and **are not units produced**. Frozen observations keep
the original host construction history; the candidate retains its alternative
production commitments across those observations, so later requests cannot be
read as a host experiment.

To check the investment boundary independently of that altered history, a
fresh brain loads the turn-156/frame-0 observation, with Eiffel Tower already
under construction. Baseline and candidate produce identical exported orders
and internal actions for that loaded frame.

Five focused tests pass: new Eiffel Tower refusal; bargain/tally variants;
queued-wonder valuation; interrupted investment valuation; and unaffected
Culture/Score lanes. Before implementation, the two new-race tests failed and
the two initial controls passed. The interrupted-investment test was added
with the implementation.

`cargo test --profile ci --locked`: 3,725 library and 205 other tests pass,
49 library and four doc tests ignored. `cargo fmt --all -- --check` and
`git diff --check origin/main...` pass. The branch was already current with
`origin/main` (`e6def77d1`) before final checks. No engine rules change, so an
engine crash soak is not applicable.

Local artifacts: `/tmp/civvis-domination-wonder-frame-replay.py`,
`/tmp/civvis-domination-wonder-replay/source-events.jsonl`,
`{current-baseline,candidate}-orders.jsonl` in that directory,
`/tmp/civvis-3632-final-comparison.json`,
`/tmp/civvis-3632-ongoing-comparison.json`, and
`/tmp/civvis-3632-{red,focused,full}.log`.
