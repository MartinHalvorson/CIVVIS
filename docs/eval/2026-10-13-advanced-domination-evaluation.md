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

All four pairs finished. Both arms lost every game to Science, held zero
foreign major cities or capitals, and had byte-identical applied-action
records within each pair. Immortal seeds ended at turns 161 and 163; Deity
seeds at 122 and 150. The focal seat kept its home capital in the first
Immortal seed and lost it in the other three. It declared no major war in
three seeds and one at turn 86 in the second Deity seed. Both two-pair
commands exited 2 because their existing reach guard rejects an entirely
identical batch. These are completed negative probes, not passing strength
comparisons or evidence for enabling the selected policy. No source or seed
was changed in response.

Full local validation passed all 4,980 default-plus-developer-tool tests with
four workers, zero retries and no fail-fast; 51 pre-existing skipped cases
remain skipped. Independent CI on implementation `ff4e53b59` passed every
required check, including the default Rust suite, all 24 evaluator tests,
Rust quality and paired cost. Formatting and diff checks passed. This change
extends the evaluator's accepted setup; it changes no simulator rules or AI
policy, so an engine soak is not applicable. Source,
binary digest, registration and outcome/conquest logs are retained under
`~/civvis-tactics-results/2026-10-13/native-movement-adoption/`. Native adoption
of the separately published movement fix is recorded in its own task and does
not change this evaluator's simulation results.
