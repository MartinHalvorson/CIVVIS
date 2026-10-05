# Productive Builder coverage experiment

The goal is faster production growth against high-level rivals. This experiment tests whether an additional Builder purchase can service productive worked plots that the existing worker fleet cannot cover. High-level production parity and actual Firaxis gains remain unverified.

## Diagnostic evidence

The control is native source `84a64d8a3cf9769afec4940f4b611608ac0851d2`. A temporary read-only trace recorded the actual reserve, gold, income, purchase quote, blocking, current productive worked plots, field charges and queued Builders at the existing treasury call. Four previously consumed seeds produced 120,977 identical executed actions and identical final serialized worlds with and without the trace. There were sixteen diagnostic executions, including a separate explicit-action verification pass; these are zero fresh strength samples.

| Difficulty / seed | City calls | Calls with raw worked production gain ≥4 | Affordable productive calls |
| --- | ---: | ---: | ---: |
| Emperor 61006800 | 403 | 42 | 0 |
| Emperor 61006801 | 369 | 7 | 2 |
| Deity 61006900 | 158 | 0 | 0 |
| Deity 61006901 | 370 | 78 | 11 |

At Deity 61006901 turn 74, the treasury held 256.8 gold against a 120 reserve and a 116 Builder quote. Two cities had five and seven raw production worth of legal worked improvement jobs; the empire's single field Builder had one charge. Similar affordable calls occurred at turns 77 and 78. The Emperor opportunities already had substantial field charges. Affordability alone establishes neither safety nor realized production.

The equivalence JSON records full action counts and hashes; the affordability JSON includes every affordable productive call and zero-opportunity seeds. Raw trace CSV/TXT files are retained here, with full final worlds and action logs archived under the durable evidence path recorded in the equivalence JSON.

## Candidate and prospective evaluation

The prototype purchases additional Builder coverage through the existing treasury policy for the explicit Domination controller. It requires two cities, a field or queued Builder, positive income sufficient to replace the purchase bill, military coverage, and peace with major rivals. It excludes recovery, strategic threats, recently attacked cities, local barbarian alarms, low loyalty, amenity deficits and host-blocked purchases.

The preflight considers currently worked, owned, unimproved plots inside city range, using actual legal non-clearing improvements. It credits every nearby positive field Builder charge and Builders at nearby queue heads that complete within ten Standard turns. The bought charges must cover at least three raw production and forecast a return of 125% of the full Builder production bill within thirty Standard turns, allowing ten turns for work. It retains the actual reserve and all queue progress. The forecast is deliberately approximate; completed work and game outcomes determine whether the rule is useful.

The pilot manifest fixes four paired Emperor and four paired Deity seeds, with twelve Emperor and eight Deity confirmation pairs reserved. The same native map, speed, focal civilization, explicit controller and deployed gene ledger apply to both arms. The focal seat is handicap-exempt while rivals receive their difficulty bonuses. Checkpoints include production, cumulative production, strongest-rival production, science, culture, military, survival, actual Builder gold buys and completed productive-type improvement actions. All outcomes, zeros and missing checkpoints are retained.

The advance thresholds and runtime hashes were recorded before fresh play. Component promotion requires Deity turn-75 production ≥+5% and cumulative production ≥+2%, nonnegative Emperor values, bounded later production and other yield losses, no additional eliminations, and actual purchase/work effects. These thresholds cannot establish the complete production-parity objective.

## Pilot result and decision

Reject the prototype. Every checkpoint, all eight final worlds, and all 215,603 executed actions per arm were byte-identical. There were no additional executed Builder purchases or completed productive improvements. This screen therefore measures an inert policy on these seeds rather than the benefit of buying additional workers. The limiting guard or purchase legality condition was not instrumented in this strength screen.

| Difficulty | Paired turn-75 games | Production | Cumulative production | Mean focal / strongest-rival production | Eliminations per arm | Wins per arm |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Emperor | 3 | 68.167 → 68.167 | 2362.617 → 2362.617 | 0.537 | 0 | 0 |
| Deity | 4 | 29.312 → 29.312 | 1500.638 → 1500.638 | 0.144 | 2 | 0 |

The Emperor game ending at turn 67 is absent from turn-75 and later checkpoints; another ended at turn 105. One Deity game ended at turn 104. The paired summary retains their final outcomes and every missing checkpoint. Eliminated seats contribute their recorded zero production when the game continues. No confirmation games were played; all reserved confirmation seeds remain untouched.

The prototype and positive/negative action-system tests are preserved in its source-history checkpoint and exact prototype patch. The final change archives the diagnostics and negative pilot, removing the temporary trace and runtime purchase rule. No default, rule, ledger or production-parity improvement is delivered by this experiment.

## Validation

The initial four focused tests passed. They verify the actual purchase quote, reserve and queue preservation; coverage from field and soon-completing queued charges; eight safety/affordability rejection cases; and actual movement plus a completed worked Mine. The completed-Mine test was strengthened to check the modeled city's production delta before the full locked suite. The prototype full locked suite passed: 4,556 tests, zero failures, 54 ignored. The strengthened city-production assertion passed. The initial source-quality check passed. Final checks after runtime removal and main synchronization are recorded separately in the validation receipt.


The final integrated tree passed 4,563 tests with zero failures and 54 ignored, using `cargo test --profile ci --locked -- --test-threads=4`; changed-line quality passed. The initial default-thread integrated attempt failed two server deadline tests (303 ms versus a 250 ms frame bound, and three observed turns in a two-second window). The complete unchanged suite passed on retry with four threads. Both attempts are retained in the validation receipt and durable logs. Source and dependency files match integrated main.
