# Asymmetric Domination evaluation at Immortal and Deity

The fixed-profile `victory_eval --domination-pair` probe rejected levels 7 and 8
before creating a game. It now resolves `--difficulty` through the canonical
eight-rung registry. Invalid names remain errors; omission retains the archived
King-major/Emperor-barbarian configuration. Explicit choices set both major and
barbarian difficulty. The focal Gran Colombia Domination seat remains exempt
from difficulty bonuses, while three adaptive CIVVIS rivals receive them.

The unchanged parser failed both new advanced-rung cases at fixture checkpoint
`b25a69f09`. Candidate `ff4e53b59` passed all 24 evaluator tests. The new cases
create real initial games and verify that the recorded profile matches the
actual major/barbarian difficulty, the focal seat has one ordinary military
unit and no yield or combat bonus, and rivals receive four military units,
24% Science/60% Production and +2 strength on Immortal, or five military units,
32% Science/80% Production and +3 strength on Deity. Existing default-profile,
same-world, policy-isolation and output-preservation tests remain covered.
The prior test for Emperor rejection now uses an invalid rung because every
registered difficulty is deliberately supported.

The built candidate was registered before outcomes for four same-seed policy
pairs: `siege-is-progress-3` withheld/enabled only for seat zero, two Immortal
seeds 202610130700–202610130701 and two Deity seeds
202610130800–202610130801. Each pair keeps Tiny 60x38 Pangaea, four players,
six city-states, Online speed, all victories and the 250-turn clock. Both legs
use the same compiled public forced-policy bundle. These are simulator reach
probes; they do not reproduce Firaxis rival AI or prove native win-rate gains.
No source tuning follows their outcomes.

Full local validation, independent CI and probe review are pending. Source,
binary digest, registration and logs are retained under
`~/civvis-tactics-results/2026-10-13/native-movement-adoption/`. Native adoption
of the separately published movement fix is recorded in its own task and does
not change this evaluator's simulation results.
