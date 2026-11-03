# Eight-family native Science fuel feasibility pilot

**Completed: the candidate did not demonstrate Coal delivery.** The first eight configured Science families after arming produced 23 explicit Coal-buy offers in two enabled families. All 23 correlated replies were unclosable; none produced a matching `deal_closed` sent command. This does not establish a Lab Science gain or a strength improvement. Keep `science-power-buys-fuel` default OFF.

The registered stop rule completed on November 3 at `02:02:09.621488Z`: the monitor verified that the screen still contained the owned tag and restored its previously empty bytes. The stop receipt binds the final census and collector hash. It did not stop a live game or change the external verification policy.

## Complete primary cohort

Times and run tags are UTC. “Retired” is the existing production-share stop, kept separate from terminal defeats. Actual genome readback agrees with each listed arm.

| Run | Difficulty | Arm | Ending | Coal buys |
|---|---|---|---|---:|
| Nov 2 `201043Z` | Emperor | off | Rival Science, 227 | 0 |
| Nov 2 `202907Z` | Immortal | on | Retired, 126 | 0 |
| Nov 2 `205136Z` | Immortal | on | Retired, 124 | 0 |
| Nov 2 `211245Z` | Immortal | on | Rival Science, 199 | 0 |
| Nov 2 `213554Z` | Immortal | on | Rival Science, 196 | 5 |
| Nov 2 `234025Z` | Immortal | on | Rival Science, 215 | 18 |
| Nov 3 `014143Z` | Immortal | off | Rival Science, 180 | 0 |
| Nov 3 `015442Z` | Immortal | off | Retired, 129 | 0 |

There were five enabled and three disabled families, five terminal rival-Science losses and three production-share retirements. The latter occurred at 81/506, 85/511 and 70/488 production, respectively, after three qualifying turns below one sixth of the leader. No victory was observed. These are feasibility outcomes, not a pooled win-rate comparison.

## Execution and finance

The two exposed primary families generated 56 positive candidate-note rows. Notes are not separate purchases: the actual 23 offers requested `RESOURCE_COAL=20`. Each reply is joined by the native `from` player and explicit `asked_turn` to its offer's `target` and turn; resource IDs in the replies are not guessed.

Nine replies had a positive `pay` quote and all nine exceeded their own ceiling. The other fourteen replies were also unclosable; a zero quote is not a free accepted purchase. `pay` is the handler's Gold + 25×GPT metric, not a verified payment. The recorded nominal offer caps and full quoted terms remain in the JSON and raw slices. No accepted payment or delivered stock is attributed to these requests, and missing stock fields remain unknown.

The gross-income guard also limited exposure: family `211245Z` showed unpowered Labs with positive Coal income and generated no candidate buy note. That follows version one's defined positive-income exclusion; it is a possible net-supply blind spot, not evidence that a submitted order failed. A smaller purchase block is another follow-up hypothesis. Neither change was applied during this pilot.

Twenty-six assigned other-lane families remain separate. One assigned Domination family had 15 candidate notes and five explicit Coal-buy offers, with no matching sent close. Three additional Coal offer rows lacking a direction are retained as unclassified. They are not added to the Science cohort or attributed to this candidate.

## Context and interpretation

Preregistration `ff1df764c` was pushed before the screen was armed at `2026-11-02T19:01:33.564184Z`. The companion immutable plan and arm receipt define the first-eight-family boundary, continuation joining, other-lane retention and ownership-based restoration. The prior game retained its existing assignment.

An external operator changed future difficulty from Emperor to Immortal around 20:13 UTC. That policy was preserved. Binaries also changed: the first five families used executable hash `bdb0125b…`, the sixth `3a260648…`, and the final two `4f269c73…`. After removing only the tested tag, the first six share normalized-genome hash `3b1af166…`; the final two use `bad9651d…`. There is no matching difficulty/binary/normalized-profile bucket containing both arms. Do not pool these histories into a treatment effect or promote a default from them.

The direct power exporter was read back before arming from native `185013Z` turn 1/frame 1: free/temporary/required power zero, `fully_powered=true`, and active-project power false. That established the observation path at zero demand. The final census retains native city power, plant/Lab ownership, Science and yield-source text without attributing changes caused by growth, construction or policies to fuel.

## Validation and reproduction

Five collector tests passed after integrating current main. The collector's bytes still match the monitor's frozen SHA-256. The archive retains the final stop census, arm/plan, ownership restoration, monitor source/log, all 34 configured-family summaries and observed genotypes, primary power readings, all retained buy/Coal deal rows, and candidate notes. Every selected raw line is checked against its recorded hash and original source line; every whole source file still matched the stop census when packaged. No setup/pending family or beyond-boundary Science family was present at that stop snapshot.

The adjacent `.final.json` binds the archive's hash and member count. `SHA256SUMS.json` binds every other member. Extract it and run `python3 final-analysis.py` to recompute the aggregate from the frozen census; that script makes no live-game commands. The general read-only collector remains `tools/civ6_science_fuel_pilot.py`. No strength ledger or reserved research confirmation seed was used.
