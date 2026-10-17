# Rejected city-health fire-support experiment

Decision: reject the candidate. It produces no fresh focal or Domination wins
and does not convert the primary breached city. The production change and its
new test module are restored to baseline; only the protocol and evidence ship.
The continuous strongest-difficulty conquest objective remains unachieved.

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
evidence. All eight independently drawn fresh maps, split equally between
Immortal and Deity, complete with both arms and no seed replacement.

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

## Results

| Fresh eight-map cohort | Baseline | Candidate |
| --- | ---: | ---: |
| Focal wins | 0/8 | 0/8 |
| Focal Domination wins | 0/8 | 0/8 |
| Religious losses | 1/8 | 1/8 |
| Maps with observed foreign-major-origin holdings | 1/8 | 1/8 |

The single foreign-major holding map is the same in both arms: Immortal
67208704343, with Nueva Imperial first observed under focal ownership at turn
79. Its complete focal action stream is unchanged, so that holding is not a
candidate gain. Holdings alone are not proof of military capture. Only two of
eight fresh pairs have different focal action streams, Immortal 69607428722
and 68428726463; each has two health-fire reservation journal mentions. Neither
wins or holds a foreign-major-origin city. The other six pairs remain exact on
the focal action stream. All four candidate development maps also remain
losses without observed foreign-major-origin holdings.

On primary development map Immortal 47651167238, the first observed Catapult
is fielded at turn 82 instead of baseline 93. Candidate Catapults 599 and 627
advance into a field fight, but neither has a logged ranged order against
Bamako. Both are absent by turn 89. The actual unwalled city health never falls
below 200 at observed boundaries from turn 72 onward, and the focal player
never owns it. The candidate still loses to Mali's Science victory at turn
149, versus baseline 150. Earlier weapon production is therefore not the
missing demonstration of siege conversion. It identifies a further delivery
and field-combat question, not a proven routing or escort bug.

All 40 new full executions finish: 16 baseline observer controls, eight
candidate development A/A executions, and 16 fresh paired executions. These
are only eight fresh independent maps, not 40 strength games. Candidate A/A
actions, focal snapshots and full-world observations match byte for byte on
all four maps; terminal results match except the explicit trace field. No
journal-budget truncation is observed. Both arms satisfy the no-fewer-wins and
no-additional-Religious-loss gates, but the required fresh Domination win and
primary capture mechanism fail. Do not promote the candidate.

## Rejected candidate and safeguards

The candidate extended the existing matched-gun calculation and delegated first
weapon reservation to a fresh, active, non-Hold siege on the one-war front.
It required existing matched-gun, siege-train and positive-damage-budget
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
Domination win. The library change is compiled for every controller; rivals
retain their ordinary ledger setup, while only the focal force bundle is
explicitly overridden. This is not an isolated fixed-baseline-rival test.
The external probe's dependency lock and CI profile differ
from the repository's CI configuration, equally for both experimental arms;
there is no claim of deployed-binary equivalence.

## Validation and promotion

The initial six-test regression execution has four behavior assertion failures
and two passing negative controls before the production change. An additional
assigned-gun continuity test is added with the implementation. New-fixture map
accessor and constant-scope compile errors are repaired, with failed logs
retained separately. The continuity fixture also initially expects two guns;
the existing regeneration/endurance model actually requires three in that
position. Its corrected assertion explicitly verifies that two-gun take time
exceeds the endurance horizon and three guns can finish. The corrected focused
suite passes all seven tests; the adjacent siege-production filter passes 51.
The complete experimental suite passes 6,184 with 56 ignored, and scoped
formatting/clippy validation passes. The restored-source suite passes 6,177
with 56 ignored. Every tracked non-evaluation file matches baseline main
`198eda719c104e719b28d5ebb8c6e0bc93a4dff7`, with no untracked non-evaluation
files or remaining new test module. Latest main is merged once before final
review; it remains that same baseline revision.

The complete candidate source, binaries, observers, locks, runners and 283
force inputs are frozen at 2026-10-17 16:33:10 UTC, before any fresh outcome.
Candidate source is `8ad61e178aeaa3db77dad1515de2b81f5261d11c`; binary SHA-256
is `2ae9e5f369639759dfb0025ed4e2b18fb96371cfdafd5f90ccbda992a7f84fe6`.
All candidate A/A controls finish before the fresh cohort begins. Inputs and
complete source blobs are reverified after all games. The immutable baseline's
raw `source` metadata remains `git-tree:cb721b4ed4aeae53d8d4dd22d09abcb16b860ec0`;
an initial analyzer commit-versus-tree expectation error is repaired explicitly
against that frozen identity, with its failed log retained. No game is rerun
or result metadata rewritten to repair this analysis assertion.

One checkpoint push violated repository wait discipline: its shell command
printed two in-flight body-triggered checks but failed to abort before pushing
the fixture correction. The old collaboration-policy and overwrite-guard runs
were cancelled by that push. The deviation is preserved in
`checkpoint-push-audit.json`; no force push, check bypass or experiment input
change followed. Subsequent writes must assert zero in-flight checks, and
ordinary final-head required checks still gate shipping.

## Reproduction

`2026-10-17-conquest-city-conversion.provenance.json.gz` stores complete raw
game outputs, commands, controls and prior reference bytes, full source
manifests, selected source bodies, probe inputs/locks, force inputs, failed and
successful validation logs, both analyzers and launch/verification receipts.
The experimental source remains identified by its exact Git commit/tree;
compiled binaries are recorded by hash, not bundled as executable payloads.
Archive hashes and sizes are in the companion verification receipt.

Every content-addressed blob is independently extracted and length/hash
checked. Baseline controls and candidate A/A are checked again from complete
raw bytes. Both derived analyses reproduce byte-identical output from the
extraction. This reproduces analysis, not simulation execution; it does not
claim that another machine rebuilt an identical binary. Python 3.12 is used
for the new freeze, runner, analysis and archive scripts.

The external study directory is
`civvis-tactics-results/2026-10-17/conquest-city-conversion`.
