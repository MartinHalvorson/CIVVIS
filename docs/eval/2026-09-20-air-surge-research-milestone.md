# Finish the appointed air breakthrough — 2026-09-20

## Native failure

King/Online Gran Colombia run `civvis-20260920T092905Z`, pinned to
`bd61b7195de83798d913388742ec912c2a56f126`, appointed an aircraft campaign
against Sais on turn 110. Its research followed the Advanced Flight path
through Radio, which was known on turn 142. It then requested Metal Casting
(142), Printing (145), Ballistics (146), and Siege Tactics (149), before
finally requesting Advanced Flight on turn 150. The first Bomber was
observed by turn 163 and attacked units from turn 167. City attacks began
on turn 187 against substantially stronger defenses. The game ended in a
culture loss on turn 215, with zero verified enemy city captures.

`BasicAi::era_window_techs` permits the oldest unfinished era and the next
two. On turn 142, Printing, Metal Casting, and Siege Tactics kept that floor
at Renaissance (3), excluding Advanced Flight (6) even though Radio had
satisfied its prerequisite. The aircraft campaign's forced goal was still
present, but it could not select a technology removed from its candidates.
Native `TECH_THE_WHEEL` correctly maps to `wheel`; it was not a missing
Ancient technology or a mirror-name defect.

## Change

When the aircraft campaign owns the forced research goal, admit its exact
milestone if `Game::available_techs` says it is already legal. This follows
the existing explicit Science milestone exception without widening the
normal research window or admitting the aircraft prerequisites beyond it.
An earlier-priority goal, including an appointed land breakthrough, keeps
its existing precedence. Research already in progress is retained.

No engine rules, technology prerequisites, campaign appointment thresholds,
or native transport behavior change.

## Validation

The five regression tests cover peaceful and wartime aircraft appointments,
unappointed research, prerequisites, an existing research commitment, and
a higher-priority land campaign. Before the fix, the appointment test
selected Electricity instead of Advanced Flight; the other four passed.

All five focused tests pass after the fix. After merging current main
`191c3099ed25a2791ab8b7fb3323385086da3db6` (#3616), the full
`cargo test --profile ci --locked` suite passed: 3,687 library tests and
204 binary/integration tests; the repository ignored 49 library and 4 doc
tests. Formatting and `git diff --check` passed.

A paired replay of all 631 native frames from turns 1–215 used base
`460ebd6eb8abd9aee59161eae87bbe1f14573912` and that same base plus this
research change, before the #3616 integration. Both exited successfully.
Baseline first requested Advanced Flight on 150/0; candidate on 142/0.
Exactly five exported actionable frames changed (142/0, 143/0, 145/0,
146/0, 149/0), exclusively research choices. The same five frames changed
internal native actions. Comparison excludes `order_failed`,
`order_verified`, and `turn_verified` telemetry. Repeated candidate requests
reflect the recorded host's unchanged research state, not repeated completed
technologies.

Single-pass replay elapsed times were 334.81/328.51 seconds. They do not
establish a performance improvement. Artifacts on this host:
`/tmp/civvis-native-air-milestone/events.jsonl`,
`/tmp/civvis-air-milestone-replay/`,
`/tmp/civvis-air-milestone-frame-replay.py`, and
`/tmp/civvis-3617-{full,comparison}.log`.
Engine soak is not applicable to this AI-only candidate-selection change.

## Limits

Frozen observations do not execute the alternative research order. Earlier
requests cannot establish earlier completion, Bomber production, native
city capture, or domination victory. The eight-turn native detour identifies
a delay to remove; it does not prove that removing it wins the game.
