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

Frozen replay and full-suite results will be recorded before integration.
AI-only change; no game rules, mirror contracts, or host driver changes, so
an engine crash soak is not applicable.
