# Science foundation before advanced backlog

`science-foundation-before-backlog` is a default-off experiment. It tests whether a Science seat should open usable Research Labs and nearby Rocketry before optional advanced army/backlog or air research. It does not assert a strength improvement.

## Native motivation

The qualified legacy continuation of `civvis-20261105T002152Z` issued research at frame zero: Advanced Flight at turn 123, Replaceable Parts at 129, then Rifling at 133, Refining at 136, Steel at 138 and Combustion at 142. The journal places the turn-123 research after a Recovery assessment with Gwangju threatened and four Defend objectives; a later same-turn Science assessment must not be assigned to that earlier order. At turn 133 the initial assessment was Science, and the journal explicitly identifies the tank upgrade as the forcing goal.

That continuation had six Universities and no Research Labs at turn 150; Chemistry and Rocketry were still missing. At turn 133, Replaceable Parts and Radio were known and Chemistry needed Sanitation plus Chemistry. These are facts from one selected saved world, not a representative estimate. Several military technologies remain useful prerequisites later in the space race; the hypothesis concerns ordering and parallel infrastructure construction.

## Predicate and priority

The flag requires a raced Science target, enabled Science victory and a Science decision objective. Recovery, a threatened city or a rush closes it. An owned land military unit with a missing pre-Modern successor technology also closes it, even if that unit is a minority in a modern stack.

Before Chemistry, at least one owned Campus family district must have every prerequisite building of the Research Lab. After Chemistry the target is Rocketry. Every missing immediate parent of the target must already be available to research, preventing a long new beeline. Owned Rocketry ends the window and restores the existing backlog/air priorities. The flag does not turn on `first-launch-keeps-research` or protect Satellites.

The match arm follows walls, openings, appointed war breakthroughs, decisive packages and siege tiers. It also yields to the chooser's barbarian emergency, standing-army fuel and named production goals. Only legal available prerequisites of its selected goal are admitted beyond the rolling era window. Current research is never interrupted.

## Validation status

Implementation and focused chooser tests are in progress. Initial attempts exposed fixture list/set API errors and stale city-defense references from direct unit removal (five tests passed, two chooser tests failed before selection); the fixture now uses the unit-removal API. Registry/macro placement attempts also failed the append-point suite and were corrected before querying. The registry row is appended after all existing rows and before the empty tail markers, preserving every existing bit; the suite requires future synthetic insertions at every marker to preserve that order. These failed/stopped attempts are not passing validation.

External experiment artifacts: `~/civvis-tactics-results/2026-11-04/science-foundation-before-backlog/`. The intent is recorded before querying. The protocol uses two complete current-source OFF controls, then an ON discovery stopped at the first changed order, followed by two ON repetitions against their own complete-payload oracle. The executable is shared; actual genomes, arguments, reads, source hashes and EOF must be checked. Historical native captures supply the inputs; they do not establish full historical parity for the newer controller. Native continuation outcomes remain pending.
