# Early growth capacity under the enabled live controller

Do not retain this runtime policy as a production improvement. Across four
Emperor and four Deity consumed maps, the candidate and control produced exactly
the same **183,726 actions and eight final worlds**. This is a coverage diagnostic,
not a negative strength estimate: zero fresh strength samples were played, and
the policy might affect other maps. There is no demonstrated gain here, no
high-level production parity, and no actual Firaxis verification.

## Configuration and reproducibility

The public targeted Domination focal seat calls `enable_live_bridge()` before
play. The probe asserts `wide_map_capacity` is enabled in that seat and disabled
in the fixed stock adaptive rivals. Its setup file reads the enabled focal
field back; a deployment tag alone is insufficient. Native simulation uses
four majors, 60×38 Pangaea, six minors, Online speed, a 150-turn cap, all victory
conditions, Gran Colombia in the handicap-exempt focal seat, and the stated
difficulty for the other seats and barbarians.

The control source is `8ece0460541b09da9a09dba65b903183cb935e30`. Its library
was built from an immutable archive with the current locked dependencies.
Every control action log and final world exactly reproduces the corresponding
properly enabled live replay from the prior setup audit. This equivalence was
checked first on Emperor 61007001 and Deity 61007101, then on the other six
maps. It does not transfer earlier stock-plus-ledger strength claims to live.

The candidate runtime source is
`256b2c4e2a76203917017f8178494c475ca518c4`. A later commit fixes an artificial
test's assumption that Granary maintenance must be positive; the policy already
reads the loaded rule, whose maintenance is zero. Later evaluator changes also
leave runtime code unchanged. Both arms link the same probe against their own
fresh frozen library dependencies. The diagnostic manifests record compiler,
source, executable and dependency hashes before play.

| Difficulty | Consumed seeds | Pairs | Changed action logs | Changed final worlds |
| --- | --- | ---: | ---: | ---: |
| Emperor | 61007000–61007003 | 4 | 0 | 0 |
| Deity | 61007100–61007103 | 4 | 0 | 0 |

The initial two pairs and supplemental six pairs were separately frozen before
their executions. No fresh pilot or confirmation was run. Proposed unused ranges
61007400–61007707 were not reserved or played. Outcomes and missing checkpoints
remain in the evidence; games ending before the cap are not silently excluded.

## Archived policy

The prototype considers an empty, housing-bound queue in explicit Domination
during the first 150 Standard turns. It reserves a Granary only if projected
additional citizens repay at least 110% of its remaining Production cost within
60 Standard turns, with greater final population and actual projected building
completion. It prices the empire's production, including modeled amenity
reallocation, while holding other populations, improvements and future actions
fixed. It preserves existing commitments, early defense and expansion reserves,
requires fielded defenders and cash runway, and rejects nearby visible hostiles,
recent attacks, severe amenity deficits and bankruptcy.

Future citizen assignments use an explicit model counterfactual. Observed host
assignments remain unchanged, and observed current production caps construction
pace on a host mirror. This is still a forecast; the test does not prove that
Firaxis will choose those future assignments. The policy changes no engine
yields, costs, rules, difficulty bonuses or protected live-controller setup.

The archived source and tests are recoverable from Git history and
`2026-10-04-production-growth-capacity-prototype.patch`. The zero-context patch
passes an application check against the immutable baseline with
`git apply --check --unidiff-zero`. Its hash and the decision are recorded in
`2026-10-04-production-growth-capacity-archive.json`.

## Validation and evidence

The initial four focused tests passed, including comparison with actual engine
growth and completion turns, an accepted reservation reaching the governor,
rejection of growth without Production, and preservation of threats, queues and
expansion. A six-test rerun passed five and found the maintenance assumption in
the sixth fixture. That assumption was corrected without changing policy. The corrected prototype
passed CI’s Rust test step on checkpoint `2fdc5f04c`; the remaining CI steps and
own-target full suite were still running when the runtime policy was removed.
Full prototype and final-source suite results are recorded separately when complete.

The paired evaluator was checked with copies of consumed games. It rejects a
false live repair field, truncated checkpoint output and unchanged actions as
an improvement. It retains early world endings and fails coverage and additional
elimination checks. These deliberate corruptions execute no games and are not
strength samples. No prospective numeric screen was applied to the consumed
diagnostic results.

Raw setup, city, action and final-world artifacts are preserved at
`/Users/martbot-mbp-m5-max-128/civvis-production-evidence/2026-10-04/capacity/diagnostic`.
The diagnostic summary includes all 64 raw file hashes, every final outcome,
action counts, and current-control equivalence to previous live replays. Frozen
executables and dependency snapshots are preserved in the adjacent `artifacts`
directory. Tracked CSVs and logs retain both arms, including early endings.

This experiment establishes reproducible absence of action coverage on these
eight maps. It does not identify which guard or forecast rejected every possible
investment, and does not establish that housing is unimportant. A subsequent
production experiment should demonstrate changed, useful build choices under
the enabled live setup before claiming a production gain.
