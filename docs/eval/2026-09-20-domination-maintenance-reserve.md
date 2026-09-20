# Domination maintenance reserve — 2026-09-20

## Native evidence

King/Online Gran Colombia run `civvis-20260920T085522Z`, pinned to
`cf52d8c644c54197e66562cca22fb0a7a739dad2`, lost religiously to Babylon on
turn 168 with zero verified city captures. Its first Catapult completed on
turn 99 and struck Mari on turn 113 (9 wall HP and 1 city HP). It was lost
on turn 131 while the event reported -15 Gold. Several subsequent unit
losses also accompanied a negative treasury; their events do not explicitly
identify bankruptcy as the cause.

Conscription was in the host's available policy list but absent from the
active deck during the deficit. Turn 125 had 66 Gold at -8.80078 per turn;
turn 128 had 24 Gold at -16.5. The existing emergency maintenance-card rule
required `war_economy`, which is false in the pinned domination controller.
Production recovery already applies to explicit Domination independently
of that flag, so the two protections had inconsistent eligibility.

The host's per-unit bills suggest 11 Gold/turn of Conscription relief on
turn 125 and 13 on turn 128, before any other changes. These are estimates
from the frozen roster, not observed counterfactual income or unit survival.

## Change

Explicit Domination shares the existing emergency maintenance portfolio.
It still requires a major war or a named staged Conquest campaign, an army,
and Gold below `100 + 25 * city_count`. Negative income activates relief;
an already active Conscription or Levee en Masse remains protected until
the reserve recovers, so the discount's own positive income cannot
immediately trigger its removal. Other target lanes and the optional
war-economy behavior retain their existing activation rules.

No gene, engine rule, production priority, or live-game transport changes.
The normal policy legality and replacement machinery chooses an available
card, preferring Levee en Masse when unlocked.

## Validation

Six focused tests pass. The original code failed the wartime and staged
Domination tests. After enabling that gate, a separate Conquest regression
showed Conscription being removed when income became positive; the reserve
retention condition fixes that case. Coverage includes the successor card,
reserve and income boundaries, peaceful and other-target behavior, and
release to the normal portfolio once the reserve recovers.

The full `cargo test --profile ci --locked` suite passed: 3,677 library tests
and 204 binary/integration tests; 49 library and 4 documentation tests were
ignored by the repository. `git diff --check` passed. The branch was checked
against current `origin/main` before the full suite (already up to date).

A frozen replay of all 479 decision frames from turns 1–168 of native
085522 exited successfully for both binaries. Baseline requested no deck
containing Conscription; the candidate requested it on 39 frames, first on
116/1. All 39 exported actionable differences were policy decks. Internal
native actions differed on 81 frames, exclusively policy slot/unslot actions.
Telemetry-only `order_failed`, `order_verified`, and `turn_verified` orders
were excluded from the actionable comparison. The recorded host deck remains
unchanged in replay, so repeated missing-card verification reports are not
new host rejection evidence.

Baseline/candidate elapsed times were 119.86/123.37 seconds in single local
passes; this is not a statistically meaningful performance measurement.
Artifacts: `/tmp/civvis-native-maintenance/events.jsonl`,
`/tmp/civvis-maintenance-replay/`, `/tmp/civvis-maintenance-frame-replay.py`,
and `/tmp/civvis-3615-{full,comparison}.log` on this host.
An engine crash soak is not applicable: this only changes AI policy selection.

## Limits

Earlier research from #3612 is now observed in a separate native run:
`civvis-20260920T092905Z` requested Writing on turn 23, knew it on turn 27,
and had a Campus queued by turn 37. That run does not contain this change.
Neither research timing nor policy selection proves a domination victory.
Native affordability, army survival, city captures, and a winning campaign
remain unverified outcomes for this maintenance change.
