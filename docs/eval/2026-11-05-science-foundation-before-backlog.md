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

Seven focused chooser tests, six existing first-launch tests and nine existing old-techs tests pass on candidate source `cdd88e2f0ec08332b58d4629069c4f707fdb5aed`; the 14-test append-point suite also passes. Initial attempts exposed fixture list/set API errors and stale city-defense references from direct unit removal (five tests passed, two chooser tests failed before selection); the fixture now uses the unit-removal API. Registry/macro placement attempts also failed the append-point suite and were corrected before querying. The registry row is appended after all existing rows and before the empty tail markers, preserving every existing bit; the suite requires future synthetic insertions at every marker to preserve that order. These failed/stopped attempts are not passing validation.

External experiment artifacts: `~/civvis-tactics-results/2026-11-04/science-foundation-before-backlog/`. The intent is recorded before querying. The protocol uses two complete current-source OFF controls, then an ON discovery stopped at the first changed order, followed by two ON repetitions against their own complete-payload oracle. The executable is shared; actual genomes, arguments, reads, source hashes and EOF must be checked. Historical native captures supply the inputs; they do not establish full historical parity for the newer controller. The four prospective native attempts are complete; a separately registered, hypothesis-selected old-techs-enabled pair remains in progress.

### Qualified action effect

Candidate executable SHA256: `b41452c41cf1a60536bc6b0fb80cbe9304363731cf18d88cf2ed9640daa72083`. Two OFF controls each consumed all 249 preserved requests and 4,326 actual reads. Three ON prefixes each consumed 201 requests and 3,493 actual reads. Every process reached EOF with exit zero; independent verification checked full per-arm payloads, all consumed byte versions/count/order, actual argument differences and measured genome fields. Only this named treatment/forced bit differs.

The first changed order is **turn 133, frame 0: Rifling → Sanitation**. All earlier complete current-source payloads agree, and the other orders at that first changed request agree. This is the prerequisite Chemistry was missing in a state with **five** Universities; the earlier six-University count applies at turn 150. It is a proposal on preserved history, not an executed native action or a strength estimate.

The compact hash-bound firing artifact is `docs/gene_screens/fires/science-foundation-before-backlog.json`, with full proposal histories and verification receipts in the linked `.native.tar.xz` archive. Its read hash identifies the final consumed version at the changed request; the documented ordered input prefix remains part of the experiment. The firing validator accepts the artifact.


## Prospective native cohort: two saved Deity/Online worlds

The first two chronological preserved near-100 Science worlds (`033244Z` and `034948Z`) were selected before inspecting their continuation outcomes. Within each world both arms used the same candidate executable, original frozen launcher/Lua/deployment settings and source background; only this new flag differed. Their measured backgrounds have **old-techs off and air-surge-2 on**, so this cohort does not directly exercise the old-techs-enabled diagnostic interaction.

All four loaded at 96/0 and passed actual argv/cwd/genome/binary/Lua/configuration, seat/seed and city-identity checks. Canonical SHA256 records and actual captured reads verified: **866 decisions / 15,032 reads**. These are checksum/integrity checks and one observed PID per arm, not continuous process monitoring or cryptographic signatures.

| World / arm | Native outcome | Chemistry owned | Rocketry owned | First Spaceport complete | Earth Satellite complete | Moon Landing complete |
|---|---|---:|---:|---:|---:|---:|
| 033244 / OFF | Science loss 187 | 141 | — | — | — | — |
| 033244 / ON | Operator-retired at 150; censored | 143 | — | — | — | — |
| 034948 / ON | Science loss 176 | 120 | 134 | 157 | 167 | — |
| 034948 / OFF | Science loss 181 | 120 | 133 | 151 | 157 | 168 |

Dashes mean unobserved before termination, not impossible or zero. The ON retirement was requested through the CIVVIS page's new-game control; it is neither a natural loss nor a win. No failed load or no-activation arm was discarded.

Each pair's initial mechanical states agree with typed JSON comparison, ignoring only top-level `run`, `t`, `utc`, `frame`. The first differing first-state turns were **120 and 98**, before observed treatment decisions. The first ON run recorded no foundation-rule decision before retirement. The second ON run recorded and the host verified Replaceable Parts 115, Chemistry 118, Radio 125 and Rocketry 129. This proves native execution of the intended priority; it does not attribute later timing/outcome differences solely to the flag.

**This cohort establishes no strength improvement.** Both selected worlds had already survived to the save band, there is one pair per world, and native trajectories varied before treatment opportunity. The gene remains default-off and unpriced. In the first OFF world, city count fell from six at 141 to two at 150 and one at 163; science fell from 114.625 to 29.8984 to 21.2539. Rocketry was host-verified selected at 163 but never completed before the rival won. A research-ordering fix alone cannot recover that lost economy.

In the second ON world Satellites was owned at 148, before its Spaceport completed at 157. The capital produced roughly 37–46 per turn through construction and early launches. Earth Satellite finished at 167; Moon Landing was still incomplete at 176. Launch production is therefore a separate observed bottleneck. At 150, Space Initiative's governor was established in Seoul while the first pad was being built in Gyeongju; governor alignment merits a separate investigation, not a claim that this experiment tested it.

A separately registered OFF/ON pair on the original `002152Z` save near 57 retains its actual old-techs-enabled background. It is hypothesis-selected because its captured input demonstrated the action effect; earlier prerequisite-import experiments on that world are already known. Its results will stay separate from this prospective cohort and from any population win-rate estimate.
