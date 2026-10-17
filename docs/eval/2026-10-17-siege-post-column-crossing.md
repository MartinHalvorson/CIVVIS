# Siege firing posts behind a friendly column

## Registered question

Registered on October 17, 2026 against production `712a48e638dcdd2464b321f9fff9acbb4ee4eb23`, before adding the regression cases or changing production. No Native game, process, deployment pin, difficulty, safety floor, treatment bundle or verification gate is changed by this task.

`siege_posts_keeping` accepts a new or retained firing tile only when `siege_route_step` finds an ordinary first step. That router deliberately treats friendly soldiers as obstacles. In contrast, `approach` already executes a legal `MoveTo` through a friendly column when an assigned post exists. The existing direct-approach test manually supplies a post, so it does not test whether assignment admits that move. Hypothesis: a gun with an otherwise legal firing tile reachable through its own screen this turn receives no post and fails to arrive.

This is a source-derived hypothesis, not an established cause of the previous 32 simulator losses or a measured Native failure. The prior cohort's open-city diagnostic includes unposted shooters; it does not identify this routing predicate as their cause.

## Fixed behavioral tests

Add Catapult and Archer cases using the existing `walled_city` fixture, a narrow dry corridor, a friendly Swordsman between the shooter and an empty range-two firing tile, and unchanged legal movement rules. Require natural post assignment, recorded tactical movement through the stationary screen, arrival at the post and subsequent legal wall damage. Do not manually assign a post to make the positive case pass. Require preservation of a valid previous post through the same column.

Negative controls: insufficient remaining movement to cross the screen, an enemy screen, a sealed terrain pocket and a water-only approach remain inadmissible. Existing reachable-pocket, no-route, staging, entry and full Rust tests must pass. Execute the new tests against unchanged production first; retain the original assertions and baseline failures before implementing the repair. Only test-fixture mistakes may be corrected, with the original failure recorded.

The intended repair is limited to firing-post admission: retain the ordinary route as first choice, then accept a complete engine-legal path to the post this turn only if every traversed tile respects the existing dry-land and reserved inner-ring constraints. Execution still uses existing approach, entry-danger checks, stacking and recorded tactical guards. No new movement rule or weaker endurance/entry gate is permitted.

## Fixed full-game probe

After the regression baseline, freeze baseline and candidate `victory_eval` binaries built with `cargo build --profile ci --locked --features developer-tools --bin victory_eval`. Record their source and executable hashes. The sole production source difference must be firing-post admission; all other compiled source, the deployment ledger and `deploy/live-force-on.txt` remain identical.

For each source arm, run `--domination-pair unwalled-city-spares-the-train --games 4 --start-seed <seed> --difficulty <rung> --out <new-file>` once at Immortal seeds 43250000–43250003 and Deity seeds 43250100–43250103. Retain all journals with `CIVVIS_PAIR_EXPLAIN`. This is 32 complete game executions over eight distinct maps: source arm × difficulty × four seeds × the existing policy OFF/ON contrast. The OFF/ON switch is not an ablation of the routing repair. The source change applies to every controller using the repaired code, not just the focal seat.

Retain the harness's four-player, 60×38 Pangaea, six-city-state, Online, barbarian, all-victory, natural 250-turn setup. Gran Colombia focal seat zero targets Domination and is handicap-exempt; the opponents are adaptive CIVVIS controllers, not Firaxis AI. No early stopping, seed replacement, outcome-conditioned rerun, relaxed gate, or conversion of a no-contrast exit into success. Wait for all four source/rung jobs to terminate before inspecting any outcomes.

Report every game, source-arm contrast, actual completion status, focal wins, victory type, sampled foreign-city/capital holds and the existing policy contrast exit codes. Full canonical action records are compared only within each policy pair by this harness and are not persisted for cross-source comparison. Ownership observations occur at full-turn boundaries and final state; zero observed foreign holds cannot exclude transient capture/recapture inside a round. Journal planning rows do not prove executed arrival or shots. These simulator outcomes cannot establish Native adoption or highest-difficulty Native victory.

## Results

Pending. No performance or victory gain is claimed at registration.
