# Native standing-army research census

This is a post-hoc diagnostic of the fixed eight-game native material cohort
from #4120, not a new strength comparison. All eight games lose; none has a
foreign major original capital observed held. Do not choose replacement games,
promote a default or assume that delaying the Bomber beeline helps conquest.

Before running the census, retain every actual native research action and
its source decision line, every exported research transition, and each turn's
first exported state. Report known technologies, current research and quoted
cost/progress, Gold/income, unit-kind counts and major-war status. At turns
60/75/90/100/120/150 and terminal, retain missing milestones explicitly. For
Archers, Horsemen and Catapults, report first observed successor technology and
upgrade readiness/blocks. Preserve all observations and unsupported/ambiguous
mappings; absence of a native successor quote is not an affordability verdict.

The first development observation has eight Archers, six Horsemen and three
Catapults at turn 100 while researching Industrialization at peace. Machinery
and Castles are selected only at 143 and 146, after Advanced Flight at 128.
These are selected-action turns, not completion turns. The forced-goal chooser
puts the air surge ahead of wartime modernization, and that modernization
helper requires an active major war. Read the complete cohort before choosing
whether there is a priority defect or a sensible investment tradeoff.

Any later policy must be separately specified before its evaluation, retain
the existing air-beeline benefits as an explicit tradeoff, and demonstrate an
actual selected research change plus native acknowledgement. This census alone
cannot show that an earlier upgrade would be paid, survive or capture a city.

## Complete cohort result

The apparent late-Machinery problem is concentrated in the first development
game. The other seven games know Machinery by turn 99, five by turn 75.
This rejects a broad inference that the whole cohort's capital failure is
caused by leaving Archers' successor technology until after Bombers.

| Stem suffix | Machinery first observed known | Castles | Military Engineering | Advanced Flight |
| --- | ---: | ---: | ---: | ---: |
| `123652Z` | 144 | 147 | 136 | 135 |
| `124842Z` | 70 | 154 | 116 | 145 |
| `130400Z` | 62 | 130 | 94 | not reached |
| `131248Z` | 73 | 79 | 83 | 163 |
| `132534Z` | 69 | 161 | 121 | 152 |
| `134257Z` | 99 | 113 | 104 | not reached |
| `134959Z` | 75 | 101 | 72 | 147 |
| `140632Z` | 78 | 154 | 117 | 189 |

These are first exported observations of completed technologies, separately
retained from selected-action turns. For example, Advanced Flight selected
at 128 in the first game is first observed known at 135, Machinery selected
at 143 is first observed known at 144, and Castles selected at 146 is known
at 147. Two games end at turn 139 without Advanced Flight; their turn-150
milestones stay missing. Native research verification and failure receipts
remain in the report without being replaced by model selections.

At turn 75, the fourth game's four Archers and the fifth game's six Archers
have host-quoted Crossbow successors blocked by insufficient treasury. The
second game's five and seventh game's seven Archers are blocked by friendly
territory. At turn 100, the sixth game's six Archers know Machinery: five
are blocked by friendly territory and one has an unblocked quoted successor.
The third game's three Catapults similarly have a quoted Trebuchet successor
blocked by friendly territory. These examples are observed blockers, not a
proof that moving home or saving Gold would improve the campaign.

The first game remains a real outlier: at turn 100 its eight Archers, six
Horsemen and three Catapults lack the model's immediate successor technology,
while it pursues Industrialization at peace. The wartime modernization helper
requires an active major war; the air surge owns a higher research arm. This
is a specific research-priority tradeoff, not a native quote or a counterfactual
success. Castles also lands late in several games that have few or no Horsemen
at the earlier milestones, so a late date alone does not measure delayed army
modernization.

The collector labels immediate successor technologies from `data/units.json`
and separately stores exact native upgrade targets, prices and block strings.
The host can skip an immediate tier: a Catapult may quote Bombard once its
technology is known. Native research quotes list unresearched nodes and their
cost/progress; they do not by themselves establish current selectability.
The shipped base technology rows are `Technologies.xml:177–178` (Machinery
requires Iron Working and Engineering), `:181` (Castles requires Construction)
and `:225` (Advanced Flight requires Radio). Expansion/native observations
remain the controlling evidence for the actual trial.

No production research-priority change is justified by this census alone.
The next conquest investigation should examine the actual shared constraints
on paid modernization, movement and successful city assaults, while retaining
the measured benefits of the existing air beeline as a tradeoff. This result
does not establish a native strength or performance improvement.

The five-member census archive is 430,428 bytes, SHA-256
`9c86d8a32acbe3030f773e40632e1f1db812b9e8d5bd217fca380c1231a810b9`; every member is verified on readback. It includes both the final
collector/report and the initial report before the Military Engineering and
receipt columns were added. Every native source file is hashed in the report.
