# Early development audit: Standard and Quick Immortal domination

Evidence cutoff: native states through turn 91 of `civvis-20261104T055705Z`. Both runs use Gran Colombia at Immortal on Small Continents. The earlier run is Standard; the latter is Quick. The latter subsequently ended at turn 91 with `reason=killed`, `partial=true`, and no outcome; its opening observations are usable, but it supplies no win/loss result. This is a diagnosis, not a treatment win-rate estimate.

## What changes the next action

Early expansion has two distinct failure paths. The Standard run `053740Z` did not observe a second owned city until turn 76, despite a Settler appearing in the capital queue by turn 10. The Quick run `055705Z` observed its second city at turn 21 and third at turn 26, but lost control of Guayaquil at turn 53. Faster founding alone cannot solve both cases.

The strongest next investigation is to improve the defence and site quality of early expansion together. Quick Guayaquil and Quito both began with two housing. At turn 35 both had population two and the host reported a housing growth multiplier of 0.25. Quito remained at two housing through turn 75. By turn 87 its Granary had raised housing to four. This is direct native evidence of a long growth restriction, not an estimate from the simulator.

Guayaquil was damaged and had positive loyalty per turn in the last observation before it disappeared. These fields do not prove the mechanism of its loss. Do not label it a loyalty flip from city disappearance alone. The later Cartagena de Indias was founded at the same coordinates (60,25) at turn 75; losing and replacing that settlement consumed another expansion cycle.

The Standard opening needs a separate production-continuity replay: its capital repeatedly switched between Settler and military queues before its first expansion. Queue observations establish interruption, not wasted production or proof that retaining the Settler would survive the attack. Bogotá disappeared from our ownership at turn 85. Preserving the first expansion must be evaluated with capital survival as a guard.

## Selected native city observations

Each row uses the last exported state for that turn (including replans), rather than counting several frames as independent city-turns. Missing queue fields mean no item observed.

| Speed/run | Turn | City | Population | Housing | Housing growth multiplier | Queue |
|---|---:|---|---:|---:|---:|---|
| Standard/053740Z | 10 | Bogotá | 2 | 6 | 1 | UNIT_SETTLER |
| Standard/053740Z | 20 | Bogotá | 2 | 6 | 1 | UNIT_SETTLER |
| Standard/053740Z | 41 | Bogotá | 3 | 6 | 1 | UNIT_SETTLER |
| Standard/053740Z | 75 | Bogotá | 4 | 7 | 1 | UNIT_SPEARMAN |
| Standard/053740Z | 76 | Bogotá | 4 | 7 | 1 | UNIT_SPEARMAN |
| Standard/053740Z | 76 | Caracas | 1 | 5 | 1 | empty |
| Standard/053740Z | 85 | Caracas | 1 | 6 | 1 | UNIT_SPEARMAN |
| Quick/055705Z | 21 | Guayaquil | 1 | 2 | 0.5 | empty |
| Quick/055705Z | 26 | Guayaquil | 1 | 2 | 0.5 | BUILDING_MONUMENT |
| Quick/055705Z | 26 | Quito | 1 | 2 | 0.5 | empty |
| Quick/055705Z | 35 | Guayaquil | 2 | 2 | 0.25 | BUILDING_WALLS |
| Quick/055705Z | 35 | Quito | 2 | 2 | 0.25 | BUILDING_WALLS |
| Quick/055705Z | 41 | Guayaquil | 2 | 2 | 0.25 | BUILDING_WALLS |
| Quick/055705Z | 41 | Quito | 2 | 2 | 0.25 | BUILDING_WALLS |
| Quick/055705Z | 52 | Guayaquil | 3 | 2 | 0.25 | UNIT_SPEARMAN |
| Quick/055705Z | 52 | Quito | 3 | 2 | 0.25 | UNIT_WARRIOR |
| Quick/055705Z | 53 | Quito | 3 | 2 | 0.25 | UNIT_SETTLER |
| Quick/055705Z | 75 | Cartagena de Indias | 1 | 2 | 0.5 | BUILDING_GRANARY |
| Quick/055705Z | 75 | Quito | 3 | 2 | 0.25 | BUILDING_GRANARY |
| Quick/055705Z | 87 | Cartagena de Indias | 2 | 4 | 1 | UNIT_TRADER |
| Quick/055705Z | 87 | Quito | 3 | 4 | 0.5 | UNIT_ARCHER |

## Why the existing housing treatment can yield

`src/ai/advanced/granary_claims_the_bound_queue.rs:55` excludes the threatened city and recently attacked cities. `bound_granary_may_defer` preserves Walls and Settlers and, during any major war, military unit queues. The live force-on configuration includes this treatment. Its presence alone therefore does not establish that a growth-bound city can claim its Granary. The native `why.log` narrows the diagnosis: turn 32 explicitly names Quito as threatened; turns 50 and 52 give it Defend needs of 192 and 207 with no assigned force. At turn 75 the log explicitly records “Quito claims its Granary at its housing”, population three against housing two, ten turns, ahead of an idle queue. The Granary treatment did activate. This evidence strengthens the defence/site-quality investigation and weakens the case for simply raising Granary priority. Do not remove the defence guards globally from this observation.

## A specific defence routing mismatch recurs on Quick

At turn 32 the journal gives the sole two-unit force 17 to Defend Bogotá, then logs its Anvil at Quito. At turn 36 force 22 is likewise assigned to Bogotá, then forms its Anvil at Quito. Each Board line explicitly reports one force, avoiding ambiguity from selecting one of several force assignments. These observations establish a planner/formation disagreement; they do not establish where a unit ultimately moved or a saved-city counterfactual.

The merged but unarmed `anvil-follows-the-defense` candidate addresses this exact disagreement while retaining the global warning, nearest-force and range checks; see [its native evidence](2026-11-03-anvil-assignment.md). It is absent from this run's forced treatment list. Reusing that candidate on this new Quick input is a more direct next step than inventing another defence policy. First reproduce the original replies from captured reads, then compare OFF/ON through the first changed actionable order and verify the assigned city's identity. Only then evaluate live execution and full-game results. This audit does not activate the candidate or claim a strength gain.

## Priorities and acceptance evidence

1. First replay the existing Anvil candidate on Quick turns 32 and 36, then inspect broader early defence allocation and settlement choice at turns 50 and 52. At turn 50 Guayaquil had nine assigned defenders while Bogotá had two and Quito none; by turn 52 the board assigned eleven to Guayaquil and none to the other two. These are planner assignments, not proof that eleven units reached the city. Check arrival times, effective local strength and reachable safe sites with better water before changing allocation or site scoring. Preserve capital survival and measure retained city production. Quito's later Granary claim is a positive control; a simple priority increase is not yet justified.
2. Replay the Standard first-Settler interruptions, starting with turns 14–23 and 41–43. Distinguish queue suspension, completion, capture and overwritten orders. Evaluate a bounded continuity rule against native attack pressure before live activation.
3. Evaluate ranged finishing separately. The open ranged finishing PR addresses a different bottleneck; the existence of good combat exchanges cannot compensate for a weak production and science base. Avoid combining opening, housing and combat changes into one uncontrolled live comparison.

For subsequent complete games, retain speed, revision, active treatments, second/third city observation turns, city losses, production and science at speed-normalized checkpoints, and actual victory type. Use first and last state frames consistently. Quick turn 67 is approximately Standard turn 100 by the 67/100 cost ratio; equal numbered turns are not equal economic time. Several nearby runs were deliberately interrupted, so they cannot be scored as natural losses or wins. Native replay proves emitted behavior only; full-game outcomes are needed to establish improved strength.

## Failure telemetry caveat

Through order turn 90, Standard `053740Z` reports 192 `CONGRESS_GUARDS_THE_LEADER/combat_policy_not_acknowledged` failures and 21 `MOVE_TO/did_not_move` failures. Quick `055705Z` reports 231 and 92 respectively. Policy acknowledgements, superseded orders and actual unmoved units require separate counts. A total `order_failed` count is not a movement-failure rate, and repeated replans are not independent turns.

## Reproduction and source integrity

Sources are the run-local `events.jsonl`, `decisions.jsonl`, and completed-run `summary.json` under `~/civvis-civ6-runs/control/`. The Quick decider snapshot identifies revision `4cde232756de364389b56af2359b8de40c5b5a2d` and SHA-256 `ad628e49e701fda696512c59809554b50d97f5752cf4dc9a761eb28c075de67d`. Check native city fields `pop`, `housing`, `housing_growth_mult`, `producing`, `damage`, `loyalty` and `loyalty_per_turn`; units use `kind`, not `type`.

For reproducible cutoff receipts, SHA-256 below is over the last turn-91 state serialized with Python `json.dumps(state, sort_keys=True, separators=(',', ':')).encode()`. Later exports do not change the selected cutoff.

- `civvis-20261104T053740Z`: `fa85c0053f697bde4d59f286ef88d9bf8a412646610d54ebb900e53267d1c5a1`
- `civvis-20261104T055705Z`: `9637b08f7dfcbbc20fd1f3b930d9ca653398d2b49e9744373f64301dad3c5622`
