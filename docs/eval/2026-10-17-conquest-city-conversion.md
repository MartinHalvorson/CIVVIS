# Registered city-health fire-support experiment

Status: development candidate; not a demonstrated strength improvement and not
eligible for promotion before its registered evidence gates pass.

## Question and frozen protocol

Can an active Domination siege retain or request the artillery it needs after
the wall pool reaches zero, rather than treating a full-health city as having
no remaining fire-support requirement?

The preregistration is
`2026-10-17-conquest-city-conversion.registration.json`, frozen at
2026-10-17 15:55:46 UTC before this policy implementation or fresh outcomes.
Its SHA-256 is
`08647352a381bc52a888c8fdb61c173ce708bd6a0dc10eb8ae71cd60082ccfd4`.
Four previously observed losing maps are development cases, not fresh strength
evidence. Eight independently drawn fresh maps, split equally between Immortal
and Deity, must be completed with both arms; seeds cannot be replaced.

## Verified baseline observation

On development map Immortal 47651167238, Mali's Bamako at [30,10] has zero
standing wall health and 200 city health at each observed turn boundary from
72 through 80. A separate full-world observer confirms those values in the
actual engine state, not merely the controller's public memory. This does not
mean no damage happens between observations: city regeneration can erase it.
The focal treasury is not in the existing negative-income recovery gate during
this interval. Engineering completes at turn 77; an artillery request alone
would not demonstrate timely arrival or capture.

All four development maps were rerun unwrapped and traced with the immutable
baseline binary. Complete actions and focal snapshots match the earlier
reference bytes; traced plans and journals also match. A second eight-game
execution with the new full-world observer produces the same actions,
snapshots, traced plans and journals. Its unwrapped and traced authoritative
world-state output also matches exactly. All sixteen new control executions
finish successfully, with no journal-budget truncation.

The immutable baseline dependency is commit
`e2fbee4dfa6e7c659cd44cf089ed9219fd4c5d07`, tree
`cb721b4ed4aeae53d8d4dd22d09abcb16b860ec0`. All 2,757 source blobs are
independently verified. Relative to main
`198eda719c104e719b28d5ebb8c6e0bc93a4dff7`, only evaluation documents and a
`cfg(test)` module differ; production source is equivalent. The preserved
baseline binary SHA-256 is
`d1924d71ac3b7a1bde12ea5f2ce226e653749d22e53bdbce6e5fc5784729cca8`.

## Candidate and safeguards

The candidate extends the existing matched-gun calculation and delegated first
weapon reservation to a fresh, active, non-Hold siege on the one-war front.
It requires existing matched-gun, siege-train and positive-damage-budget
treatments. A high-health unwalled city must have stopped improving for the
existing speed-scaled patience interval; a live assigned land gun can retain
its health-reduction role while health falls. It does not require private
enemy Masonry knowledge. Peace, stale assessment, Hold, missing siege or front,
recent progress without an assigned gun, and disabled prerequisites remain
excluded. Existing production feasibility and economy recovery gates remain.

No new treatment, force profile, genome, difficulty, victory condition,
handicap, Native controller setting, or verification rule is changed. The
registered games use four majors and six city-states on a 60x38 Online Pangaea
map, all six victories enabled, with only the focal player handicap exempt.
This reduced simulation roster is not equivalent to a verified Native Deity
Domination win. The external probe's dependency lock and CI profile differ
from the repository's CI configuration, equally for both experimental arms;
there is no claim of deployed-binary equivalence.

## Validation and promotion

The initial six-test regression execution has four behavior assertion failures
and two passing negative controls before the production change. An additional
assigned-gun continuity test is added with the implementation. One subsequent
compile attempt fails because the new fixture uses the wrong map accessor;
that fixture error is repaired and its failed log is retained separately.
The corrected focused run and full suite are pending at this checkpoint.

Freeze the complete candidate source manifest, commit/tree, binary, observer,
dependency locks and effective force inputs before any fresh game. Complete
candidate A/A controls on all four development maps before the fresh cohort.
Promotion requires at least one fresh candidate Domination win, no fewer focal
wins or additional Religious losses than baseline, and actual health reduction
and captured cities, not merely artillery orders or military power. A failed
candidate must not be promoted, and does not complete the continuous conquest
optimization objective.

The external study directory is
`civvis-tactics-results/2026-10-17/conquest-city-conversion`; final outcomes and
portable provenance will replace this pending-status checkpoint.
