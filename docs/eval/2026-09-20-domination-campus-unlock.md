# Unlock campuses during a domination opening

Two recent native King attempts left Writing late in an expanding empire:
`civvis-20260920T065948Z` founded its second city on turn 19 and learned Writing
on turn 35; `civvis-20260920T080404Z` reached two cities on turn 20 and did not
learn Writing until turn 65. The latter researched Sailing and Shipbuilding
before Writing, preventing any Campus construction during that interval.
This is a research-order observation, not proof that a Campus would have been
built immediately or that a different order would win the game.

A domination-targeted empire with at least two owned cities now selects Writing
as an opening goal while it remains unknown. The existing prerequisite picker
walks Pottery when needed. This goal runs behind urgent defense, committed war
and air research, siege capability, and luxury connections, ahead of optional
bargains and generic technology scores. It does not interrupt active research.
Other victory targets and the one-city opening keep their current behavior.

The full research regression fails on baseline: the two-city empire chooses
Bronze Working despite Writing being available. Controls exercise prerequisite
selection, rush and ranged-defense priority, active research retention, and the
policy's victory-target/city-count/completed-unlock boundaries.

## Replay artifacts

The baseline production code is `cf52d8c64`. Both replay executables read a
growing prefix of `/tmp/civvis-native-siege-expansion/events.jsonl`, the same
303 frozen frames through turn 105/frame 0 used for #3609. They run persistent
serve sessions with the native verification batch's 19 forced genes and
domination target. Driver: `/tmp/civvis-campus-frame-replay.py`; artifacts:
`/tmp/civvis-campus-unlock-replay`.

A replay can establish changed research orders. Its later host technology and
city states remain recorded observations, so it cannot establish faster actual
research, completed campuses, higher science yield, captures or victory rate.

This changes AI research selection only. Engine crash soak is inapplicable.

## Results

Both pairs exit 0. Only research orders change; telemetry orders are excluded,
and the internal native-action comparison agrees on the changed-frame counts.

| Native source | Frames | First Writing request, baseline | Candidate | Changed frames |
| --- | ---: | ---: | ---: | ---: |
| `civvis-20260920T080404Z` through 105/0 | 303 | 64 | 24 | 5 |
| `civvis-20260920T085522Z` through 75/0 | 214 | 55 | 33 | 3 |

The second source is frozen at `/tmp/civvis-native-campus-fresh/events.jsonl`;
its driver is `/tmp/civvis-campus-fresh-replay.py` and artifact labels are
`fresh-baseline`/`fresh-patched`. Repeated Writing requests before its recorded
completion are not multiple completed technologies. The trials isolate this
research change against `cf52d8c64`; final integration also includes #3610.

Six focused tests pass. After merging current main (`eb080eae9`),
`cargo test --profile ci --locked` passes 3,671 library tests and 204
binary/integration tests, with 49 library and four doc tests ignored.
`git diff --check` passes. Logs: `/tmp/civvis-3612-red.log`,
`/tmp/civvis-3612-green.log`, `/tmp/civvis-3612-full.log`, and the matching
`*-replay.log` files under `/tmp`.
