# First-launch research pilot: unresolved, no promotion

The fixed 64-game standard Emperor pilot is complete: 384 seats, six-player
74×46 Continents, nine city-states, Online 250-turn clock, all target lanes
including adaptive, and seeds 202611020000–202611020063. Only the tested gene
varies: every other genome bit is identical across all seats. The build is
`d1fba6decb76`; its binary and source receipts are retained in the archive.

| Reading | Enabled | Control |
|---|---:|---:|
| Wins / seats | 18 / 106 (17.0%) | 46 / 278 (16.5%) |
| Alive at game end | 102 / 106 | 260 / 278 |
| Assigned-Science wins / seats | 6 / 16 | 14 / 37 |
| Assigned-Science reported Moon Landings | 14 / 16 | 35 / 37 |
| Earth-to-Moon median, completed Science pairs only | 4.5 turns (14 pairs) | 4 turns (35 pairs) |

The primary win estimate is +0.43 percentage points, with standard error
4.03 points clustered by game and 95% interval −7.47 to +8.34 points. Score
share differs by −0.02 points. This is unresolved: it establishes neither
improvement nor a null effect. Survival and selected complete-case timing
readings are descriptive, not separate promotion tests; project timings
exclude seats that never completed both projects.

Fifteen enabled Science seats report Earth Satellite and fourteen report
Moon Landing, so the stage is reached. Assignment and milestones do not
prove that optional modernization or Recovery actually invoked the changed
research branch: per-choice activation was not logged. The earlier native
Printing→Satellites captured proposal is separate evidence, not a native
execution or win result.

Keep the gene off in both controllers and the live bridge. Defer the reserved
128-game confirmation (seeds 202611021000–202611021127 remain unused) until
activation/exposure is understood better. There is no positive timing signal
here to justify spending that batch ahead of the concrete native Spaceport
loss now under investigation. This is a priority decision under uncertainty,
not rejection of every possible benefit from the research rule.

The companion JSON retains the complete header, exact reporter estimate,
source hashes and descriptive counts. The raw archive includes all rows,
source/build receipts, focused tests and both text and proper JSON analyses.
The original private file named `analysis.json` contained text; it is archived
as `analysis-original.txt`. The proper JSON was produced by the same frozen
binary with `--analyze rows.jsonl --json analysis-machine.json`. No strength
ledger ingestion or default regeneration is performed by this report.
