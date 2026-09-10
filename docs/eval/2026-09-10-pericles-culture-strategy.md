# Pericles: convert the culture economy into visiting tourists

## Strategy recommendation

For ordinary Gathering Storm games, the baseline to test is wide, solvent
expansion into early Great Works, followed by tourism multipliers and a
Faith-funded finish. This is a strategic recommendation, not a measured claim
that one build order dominates every map or leader.

Pericles gains **culture**, not direct tourism, from suzerainties: +5% per
city-state. The Acropolis supplies a cheap cultural district, extra adjacency,
and an envoy on completion. Use that feedback to reach useful civics sooner,
then turn those unlocks into actual tourism. Sources:
[Pericles](https://www.civilopedia.net/en-US/gathering-storm/civilizations/leader_pericles/),
[Acropolis](https://www.civilopedia.net/en-US/gathering-storm/districts/district_acropolis/).

The recommended sequence is:

1. Settle defensible, productive cities while keeping enough military and
   income to survive. Reserve legal hills for Acropolises during city planning.
   A fixed city quota cannot substitute for available land, loyalty and upkeep.
2. Complete Acropolises and Amphitheaters early enough to compete for Writers.
   Activate recruited people into compatible, accessible slots immediately.
   Purchase useful Great Works when affordable. Empty museums and accumulated
   Great Person bodies are unfinished investments.
3. Use Acropolis envoys, quests and policy slots to secure affordable useful
   suzerainties. Evaluate a contested extra envoy against gaining a different
   suzerainty; do not endlessly bid into one expensive contest.
4. Meet rival civilizations and establish tourism access. Seek a first trade
   connection to each rival and buy their Open Borders. Repeated routes to the
   same rival compete on economic yields, not repeated tourism multipliers.
   [Tourism overview](https://civ6pedia.x0.com/GS/?l=en_US&p=TOURISM_1).
5. Develop a museum strategy that can actually fill its slots. Advance science
   to useful tourism unlocks: Printing strengthens writing, and Computers
   increases tourism. Research priorities should reflect the tourism sources
   already present or realistically available.
   [Printing](https://www.civilopedia.net/en-US/gathering-storm/technologies/tech_printing/),
   [Computers](https://www.civilopedia.net/en-US/gathering-storm/technologies/tech_computers/).
6. Build Faith income before the finish. Use Naturalists where legal park sites
   exist; use Rock Bands at reachable suitable venues after Cold War. Prefer
   the strongest cultural defender when venue quality and access support it.
   Reserve Faith according to real purchase costs and useful targets rather
   than accumulating a permanent unspent balance.
   [Naturalist](https://www.civilopedia.net/en-US/gathering-storm/units/unit_naturalist/),
   [Rock Band](https://www.civilopedia.net/en-US/gathering-storm/units/unit_rock_band/).

Specialized relic, wonder and map-dependent approaches deserve separate test
arms. The retained Pericles results below do not compare those approaches or
establish a strongest leader. Their immediate lesson is to complete the
Great Work pipeline before optimizing late tourism percentages.

## Retained live evidence

Read-only census on 2026-09-10, covering retained directories through
`civvis-20260910T113451Z`. The seat readback identifies Pericles, Emperor,
Online speed, Gathering Storm and mixed victory conditions. These are changing
controller revisions and maps, not a frozen paired experiment.

Seventeen terminal Pericles/Culture segments represent seventeen distinct
run families in this selection. All are losses: eleven rival technology wins,
four rival culture wins, one rival diplomatic win, and one local defeat.
Fourteen segments contain an exact turn-150 state; all fourteen report zero
visiting tourists. Median tourism per turn at those marks is 4; three marks
report zero tourism per turn. Missing marks are excluded, never filled with
zero. Continuation marks describe only the retained segment, not a complete
history of its original game.

Selected host observations:

| Run suffix | Turn | Cities | Completed theaters | Amphitheaters | Museums | Great Works | Cultural GP bodies | Tourism/turn | Visitors |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `100145Z-cont2` | 150 | 7 | 4 | 4 | 4 | 1 | 8 | 4 | 0 |
| `100145Z-cont2` | 218 | 8 | 5 | 5 | 4 | 1 | 10 | 4 | 0 |
| `104739Z` | 150 | 6 | 3 | 3 | 2 | 1 | 2 | 4 | 0 |
| `104739Z` | 235 | 6 | 4 | 4 | 4 | 1 | 4 | 11 | 0 |
| `113451Z` | 150 | 6 | 1 | 1 | 1 | 0 | 0 | 0 | 0 |
| `113451Z` | 216 | 7 | 1 | 1 | 1 | 0 | 0 | 4 | 0 |

All suffixes have prefix `civvis-20260910T`. The `100145Z-cont2` board is a
particularly useful activation investigation: substantial cultural buildings
and ten cultural Great Person bodies coexist with only one Great Work at the
end. These counters do not prove why activation failed; inspect compatible
slot types, pillage, location, charges, legal host actions and order acceptance.
The `113451Z` board instead shows weak district coverage and no Great Works.
Those are different bottlenecks and should not receive the same intervention.

In `104739Z`, final culture reached 278.926 with five suzerainties, but tourism
was only 11 and visitors remained zero. The single observed Great Work at turn
150 was writing in the Palace. Culture growth by itself did not complete the
offensive tourism economy.

The route scoring caller already maps an explicit victory target to its
objective before invoking `culture_route_bonus` in `advanced.rs`. Temporary
Expansion or Recovery therefore does not remove the explicit Culture route
premium. No route behavior fix is justified by that suspected issue.

## Repeatable report and next experiments

From the repository, run:

```sh
python3 tools/civ6_culture_report.py ~/civvis-civ6-runs/control \
  --run-glob 'civvis-20260910*' > /tmp/pericles-culture.json
python3 -m unittest discover -s tools -p test_civ6_culture_report.py
```

The report filters on the host-read leader and requested Culture target,
retains binary hashes/revisions and treatment metadata, and emits each segment
with its family ID. It selects the highest replan frame per turn, breaks frame
ties by last observation, and reports exact marks and the final observed
state. It maps victory IDs using that seat's exported victory types. An
unrelated player's defeat is not a terminal local result. Missing counters
remain null; malformed event lines are counted. Summaries absent from a live
run are omitted; an empty selection is an error.

The largest **known** rival domestic count is diagnostic only: unmet rivals
can set a higher global bar. Great Work counts are occupied host entries, not
slot capacity. Building counts include existing pillaged buildings, while
district counts require completion; neither establishes usable capacity.
The first positive tourism observation is not claimed to be its creation turn,
especially for a continuation. The tool reports no pooled win-rate estimate.

Prioritize the next tests in this order:

1. Replay the building-rich, body-rich `100145Z-cont2` states through the current
   controller, including the recent Great Person production changes. Verify
   legal activation and subsequent host Great Work/tourism readback. Do not
   count an issued activation order as a filled slot.
2. Compare that with `113451Z` to isolate district coverage and recruitment.
   Measure first completed Acropolis, first occupied writing slot, expansion,
   cash flow and lost cities. Keep emergency defense effective.
3. After sources work, test broader foreign route coverage and tourism
   purchase priorities. Track distinct markets rather than raw route count.
4. Freeze binary, treatment set, leader, difficulty, speed, modes and opponent
   configuration for paired strategy trials. Retain seeds and seat readbacks;
   deduplicate continuations by family. Include all losses, draws and aborted
   runs in accounting. Separate activation correctness, simulator outcomes,
   and completed live-game outcomes when assessing improvement.

This change provides diagnostics and a strategy baseline. It does not change
controller behavior or claim a live culture win-rate improvement.
