# Urgent military denial chooses the active campaign front

## Native failure

`civvis-20260920T140616Z` ended at turn 173 in a Mali Religious victory
(`won:false`, victory 4, team 2, 2026-09-20T14:35:27.059Z).
The native brain was pinned to `e6def77d11eb4ce5999e31e4a8bfc169fbe698a3`.
China remained the primary war while Mali was already the recognized urgent
victory-denial target. The one-war peace desk repeatedly offered Mali peace
because it was the second front: turns 113, 118, 123, 150, 155, and 160.
The host actually left war with Mali at 123/1 and 160/1. At turn 150 the
explanation reported our military strength 475 against Mali's 248; this
particular peace request was a front-selection decision, not a rout.

## Change

Before preserving the existing one-war front, select the actionable military
victory-denial rival if its threat is urgent and it is already an active
enemy. Explicit operator target orders retain their existing behavior.
Nonmilitary denial, disabled denial, nonurgent pressure, and rivals at peace
do not trigger the reassignment. This does not add a declaration rule.
The selected rival becomes the single primary front for planning and peace;
the former front becomes eligible for second-front peace. Ordinary rout,
losing-tide, and other diplomatic safeguards remain in force.

This reuses the existing denial thresholds and known-city/legal-target
checks. Those checks do not establish that our army can reach or defeat the
rival before its victory. Switching fronts also carries a travel cost.

## Validation

Seven focused regressions cover the military target and peace offer together,
nonurgent/disabled denial, a religious counter, a rival at peace, an explicit
operator target, persistence when urgency fades plus rout peace, and a
cityless threat. Before the change: two expected failures, five controls pass.

After the change all seven pass. After merging main `e16f35e56`, the full
`cargo test --profile ci --locked` suite passes: 3,745 library tests plus
205 binary/integration tests; 49 library and four doc tests remain ignored.
`cargo fmt --all -- --check` and `git diff --check` pass.

## Frozen native replay

Replay the complete 497 unique decision frames from the same game with a
fresh persistent brain per binary and identical Domination/Gran Colombia
arguments and the game's 19 forced genes. Baseline `44a394961` and candidate
`e9133707c` share that baseline's production code except this front-selection
change. The subsequent main merge is covered by the full test suite above.
Both replay processes exit 0: baseline 172.82 seconds, candidate 187.11.

Excluding `order_failed`, `order_verified`, and `turn_verified` telemetry,
59 frames change exported orders; 44 change raw internal native actions.
The first change is 113/1. Every peace request changes from Mali (player 2)
to China (player 1), on the same six frames: 113/1, 118/0, 123/0, 150/1,
155/0, and 160/0. The explanation changes the campaign to Mali's Tawdenni
when each Mali war starts. Other differences are unit movements, holds,
attacks/pillage, and one combat-policy replacement; no economic/research
order category changes.

These are alternative requests against frozen observations, not executed
orders. The recorded host still makes peace with Mali at 123/1 and 160/1,
so the candidate returns to China after those observations. This replay
cannot measure the travel cost of changing fronts, actual denial success,
new captures, or a different victory outcome. The observed game remains a
Religious loss; no Domination win is claimed.

Local artifacts: `/tmp/civvis-urgent-denial-replay/`,
`/tmp/civvis-urgent-denial-frame-replay.py`,
`/tmp/civvis-3634-comparison.json`, and
`/tmp/civvis-3634-{red,focused,full,fmt}.log`.
AI-only change; no game rules, mirror contracts, or host driver changes, so
an engine crash soak is not applicable.
