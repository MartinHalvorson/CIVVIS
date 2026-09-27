# Bomber generation continuity: native defect and King regression screen

Status: **IN PROGRESS**. Candidate correctness fix; no strength claim or native
Domination win. The registered checks below determine whether it is promoted.

## Native trigger

Native `civvis-20260927T110717Z` lost to Science on turn 239. Gran Colombia kept
its own original capital, held one foreign major city from turn 231, and held no
foreign original capitals. Radio and Aluminum income appeared on turn 172,
Advanced Flight on 181, two usable airfields on 191, the first Bomber on 203,
and two aircraft on 228.

The first aircraft upgraded to a Jet Bomber by turn 214. On turns 224–226 the
active surge nevertheless reported `0/4 bombers`, despite observing that Jet.
It compared aircraft and queues against the original Advanced Flight unit name.
The omission affects package counts, launch estimates, quota accounting,
production priority and ordering. After Stealth Technology, the old Bomber is
obsolete and the producer cannot request the legal Jet successor.

The 25-turn gap between the first and second aircraft has additional causes.
Maracaibo's Bomber queue automatically became the costlier Jet Bomber on turn
204, preserving progress. It reached 337/350 production on 217, but its airfield
was pillaged and the queue disappeared on 218. Popayán then trained the second
Jet through 227. Correcting package bookkeeping does not establish that those
native delays disappear or that the game would be won.

## Candidate

Count every air-domain `air_bomber` in the standing and queued package, excluding
fighters. Preserve an existing successor queue's priority by subtracting its own
commitment from the candidate quota. Choose the strongest unlocked, nonobsolete
Bomber from the civilization's actual catalog; retain the Advanced Flight unit
as the planning anchor before it unlocks. The research goal remains Advanced
Flight. Cost, range and upkeep planning use the selected generation.

Before two aircraft stand, range planning uses the trainable generation. Once a
launch wing stands, at least two aircraft must individually have enough range
from an owned city base to the objective. A single Jet cannot lend its longer
range to an older partner. Legal production menus and the existing resource,
treasury, threat and quota guards still apply. This does not change the early
alternative-airfield prototype withheld in PR #3825.

## Registration

Registered at `2026-09-27T12:29:44.673357+00:00`, before candidate native replay or
fresh games. Evidence root:
`~/civvis-war-evidence-20260927/bomber-generation-continuity/`.

Both frozen sources replay all 700 observed native frames, including same-turn
replans, with Domination, deployed air-surge-2 ON and all 21 forced genes.
Both must finish every frame with identical treatments. Baseline must reproduce
at least one undercount of observed Jet Bombers. Candidate must produce an
active package report with upgraded aircraft and no count mismatch against its
corresponding observation. Fixed future boards establish bookkeeping and
proposals, not counterfactual conquest or victory.

Eight fresh source pairs use seeds `38260000`–`38260007`. The profile is four
players, focal Gran Colombia with Domination target, Pangaea 60×38, six city
states, Online, 250 turns, King players and barbarians, focal human handicap
exemption and all victory conditions enabled. Keep the 21 deployed forced
genes, SHA-256
`439757684071b807eb4151e055b4a9adf6ac24b7b8b6c0b5da7327b5344dcea0`.

Four independent two-seed workers alternate source order by seed. The existing
single-seed harness runs focal OFF then ON for each source; ON across sources is
primary and OFF is secondary. Freeze binaries before launch, inspect outcomes
only when every worker is terminal, and do not tune or replace seeds. Source
changes also affect rival CIVVIS controllers. The simulator is not native
Firaxis difficulty equivalence or an actor-isolated causal comparison.

Promote as a correctness fix only if generation, queue, range and quota tests,
full required checks and native replay pass, and the eight primary source pairs
show no decrease in aggregate Domination wins or final foreign original capitals
and no increase in own original capital losses. Invalid or incomplete profile
or provenance withholds promotion. This is a regression screen; any strength
claim requires a separately registered larger replication.

## Validation and results

Pending. Baseline focused tests reproduce four assertion failures: upgraded
package count, legal successor production, two-Jet range, and mixed wing count.
The original four production-queue regressions pass on baseline. Compilation
and test setup errors are retained separately from these expected failures.
