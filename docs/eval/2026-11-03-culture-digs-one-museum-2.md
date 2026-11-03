# Withhold the second Culture Museum candidate

**Decision: retain the prototype as an experiment; ship no strategy or registry change.** Four captured native boards produced zero changed host-order lists. The proposed Archaeologist reservation excludes major war and Recovery, and the source loss was in both regimes. The activation check failed after all 24 queries completed successfully.

## Native observation

Immortal Culture `civvis-20261103T002240Z` completed Archaeological Museums on turns 153 and 175. At turn 160, Szeged offered `UNIT_ARCHAEOLOGIST` for 200 production and eight turns. Across the run there were 38 distinct city-turns with a legal Archaeologist offer, but no owned Archaeologist in any observed state. It lost to rival Science on turn 182, score 617, with zero Artifacts, seven Great Works, 42 Tourism per turn and four visiting tourists. Mary Leakey remained unable to activate.

The original `culture-digs-one-museum` gene was **not armed** in that game. `lane-delegates-production-2` was armed, but the Recovery condition dispatches strategic production. The missing Archaeologist branch in the baseline governor is a code observation, **not a demonstrated cause of this loss**. The initial investigation conflated those facts; the captured comparisons corrected that explanation.

Shipped basis: `Base/Assets/Gameplay/Data/Units.xml:759` defines the Archaeologist with `PrereqCivic="CIVIC_NATURAL_HISTORY" ExtractsArtifacts="true"`. `Base/Assets/Gameplay/Data/GreatPeople_Scientists.xml:44` gives Mary Leakey `ActionRequiresCityGreatWorkObjectType="GREATWORKOBJECT_ARTIFACT"`. An empty Museum alone does not satisfy that requirement.

## Prototype and checks

The separately selectable `culture-digs-one-museum-2` prototype retained version one's Museum preference and added one idle Museum queue for an Archaeologist. It required major peace, no Recovery, a nearby owned land dig, a legal quote within twenty standard turns, and time to build and use the unit. Active or queued diggers blocked another; threatened and recently attacked cities retained priority. Routine rescoring preserved the investment and emergency defense could still redirect it.

Fourteen focused checks passed, including both production governors, duplicate prevention, original/off behavior, war and Recovery exclusions, and the host quote overriding modeled build time. They establish behavior on constructed boards, not useful exposure or strength.

The comparison was fixed before candidate results: turns 150, 153, 160 and 175, frame zero. Each exact captured input was queried six ways: original executable off and version one; candidate executable off, version one, version two and a version-two repeat. All full ordered host-order lists matched their required preservation/repeat controls. **Version two changed none of the four.** The driver then exited 1 at its explicit “candidate has no demonstrated activation” assertion. Retain that failure; it is not a timeout or an unsuccessful native game command.

An exploratory census of the latest twelve configured Culture families found two with Archaeological Museums: 39 first-frame Museum turns in total, none at exported major peace. This is observed-history eligibility, not a counterfactual trajectory. An earlier Museum could change later exposure; the census does not rule that out.

## Disposition and reproduction

The final change publishes evidence only. Both existing defaults and all native pilot settings remain unchanged. No strength screen, ledger promotion or reserved research confirmation seed was used. Investigate war/Recovery production and local threat before redesigning the reservation; removing guards merely to obtain a changed order would not establish a better policy.

The adjacent JSON binds the archive's SHA-256 and member count. `SHA256SUMS.json` binds every other member. The archive retains the candidate's exact five Rust source files and patch, source/build receipts, all 24 queries, the failed activation log, the initial six-query diagnostic, native observations, shipped-rule lines, eligibility census and tests. Candidate source commit `252a65cc19a5a5f7e3836639a455ac6bcae3d965` remains in this PR's history. The frozen candidate executable is private; its hash is recorded in the JSON.

The adjacent `compare.py` is the exact executed helper; its CLI requires the matching repository, native control root, original and candidate executables, and a new output directory. It uses disposable mirrors and never connects to a live game. The archived preliminary `query.py` incorrectly filtered an unused production display field by `kind=city`. Its full host orders and `native_actions` remain authoritative; the comparison helper uses full host orders throughout.
