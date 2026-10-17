# Unwalled melee-finisher endurance accounting

## Question and scope

An unwalled city cannot make a city-center ranged strike, but defending a
melee attack still deals a reply. The entry budget already charges that reply
to a fit member whose repeated attacks leave the required survivor health.
It filters resting, recovering and survivor-floor-rejected members out of
recurring damage, however, and also accidentally filters their melee reply out
of endurance. With `unwalled-city-spares-the-train` enabled, a rejected melee
finisher can therefore contribute its entire health reserve at the artificial
one-health-per-turn denominator.

The repair separates the unfiltered melee exchange cost from the filtered
recurring-damage contribution, using the unfiltered cost only when the existing
entry read spares the train. It does not make a recovering member attack,
change the finishing blow, alter the ready threshold, add a treatment, change
deployment defaults or modify ordinary war declarations. Production/non-entry,
swarm-off, spares-off, standing-wall and fit-active-member behavior stay as
before. This is a correctness claim about the budget, not a measured strength
improvement.

## Native lead is not causal evidence

The fixed previous rollout's `civvis-20261017T050815Z` declared a defensive
faith war on Poland at turn 39 and immediately condemned a Polish spreader.
That declaration achieved its immediate purpose; it was not evidence of a
failed ordinary conquest-declaration gate. At turn 41, Scout 262146 attacked
Wroclaw from (40,16) toward (41,16), dealt 27 and took 45 damage, ending at 55
health. The city healed 20 before the next opening snapshot. Planning journal
repetitions are not extra host attacks.

These observations motivated checking wartime accounting. They do not identify
the exact historical siege-force membership or prove that this defect caused
that abandoned game's failure. The previous historical replay's failed Native
equivalence gate remains failed; it does not authorize a causal A/B claim.

## Red control

Unmodified production with regression-only commit `b0a6af22a` fails exactly the
four predicted cases: resting finisher cost, false-ready entry, full-health
recovering finisher cost and a fit member that cannot retain the reply's
survivor floor. The module reports 22 passed, 4 failed, 1 ignored; all three
new positive/control cases pass without the repair. The failing command exits
101 and its complete log is retained.

| Fixture | Old endurance | Exchange-cost endurance |
| --- | ---: | ---: |
| Resting member | 31.666667 | 3.633333 |
| Full-health recovering member | 82.666667 | 5.333333 |
| Reply-floor-rejected members | 110.000000 | 1.100000 |

The resting fixture's capture budget is 18 turns. Old accounting makes that
appear ready against `0.8 * 31.666667`; charging the exchanges correctly makes
it unready against `0.8 * 3.633333`. No threshold or fixture is tuned after
seeing the red result.

The repaired focused command passes 26 tests with 0 failures and 1 existing
ignored test. Thus all four formerly failing controls pass alongside the
unchanged fit-swarm, swarm-disabled, production and spares-disabled controls.
The only source file differing between the frozen evaluator revisions is
`src/ai/advanced/victory_conversion.rs`, with eight lines added and eight
removed. No extra damage calculation or allocation is introduced by the
separation; this is a structural observation, not a runtime benchmark.

The full `cargo test --profile ci --locked` run passes 6,162 tests with
0 failures and 56 ignored. Incremental
`python3 tools/rust_quality.py --base origin/main --head HEAD`, scoped
`rustfmt --edition 2021 --check` and `git diff --check origin/main...` pass.
The pre-repair GitHub checkpoint independently fails on the expected resting
and recovering controls and passes Rust quality. Hosted nextest stops early:
3,220 passed, 2 failed, 2,940 not run because of fail-fast, 52 skipped, exit
100. Only the local red run executes all four failing controls. No CI failure
is waived. Fetching and merging the latest `origin/main` leaves the validated
production tree unchanged.

## Validation protocol

The companion registration fixes four expected-red regression controls, three
positive/control cases and all 32 simulator games before outcomes. GitHub
checkpoint `77378a67f` publishes that workload before simulation execution.
The first local regression-only command began before that checkpoint, under
the earlier local registration; do not call it an externally preregistered
regression test.

The simulator readout is a source-by-policy matrix: unmodified and repaired
production, each with focal `unwalled-city-spares-the-train` OFF/ON, at four
fixed Immortal and four fixed Deity seeds, through the existing natural
250-turn clock. Opponents are adaptive CIVVIS, not Firaxis; the focal seat is
handicap-exempt under the unchanged harness profile. Identical OFF outcomes
across sources are only an outcome-level negative control, not canonical-action
identity. Every no-contrast exit, game outcome and failure must remain visible.

The unchanged compiled focal bundle contains 283 tags, including the positive
budget, counts-what-fires, projects-the-seal, swarm and spares flags. Rival
controllers go through the unchanged deployment ledger, in which both swarm
and spares are screenable opt-ins outside the deployment genome and operator
pins. They start OFF; the source repair is not an unacknowledged rival-policy
treatment. The source/ledger/bundle audit is retained with the binary hashes.

## Complete hard-rung simulator result

All four source-by-rung jobs terminated before the first outcome read. All 32
games completed with an actual rival victory, at turns 129–171; none was a
short-clock abandonment. Every focal game lost: 28 rival Science victories and
4 rival Culture victories across both sources. No focal game ever held a
foreign city, major or minor. This is explicit negative conquest evidence.
These are paired executions from eight fixed seeds, not 32 independent maps.

| Source | Rung | Spares | Wins / Domination | Own capital held at end | End turns |
| --- | --- | --- | --- | --- | --- |
| Baseline | Immortal | OFF | 0/4 / 0/4 | 2/4 | 138, 154, 171, 133 |
| Baseline | Immortal | ON | 0/4 / 0/4 | 1/4 | 133, 157, 156, 133 |
| Candidate | Immortal | OFF | 0/4 / 0/4 | 2/4 | 138, 154, 171, 133 |
| Candidate | Immortal | ON | 0/4 / 0/4 | 1/4 | 133, 157, 156, 133 |
| Baseline | Deity | OFF | 0/4 / 0/4 | 2/4 | 142, 144, 139, 129 |
| Baseline | Deity | ON | 0/4 / 0/4 | 2/4 | 142, 144, 139, 129 |
| Candidate | Deity | OFF | 0/4 / 0/4 | 2/4 | 142, 144, 139, 129 |
| Candidate | Deity | ON | 0/4 / 0/4 | 2/4 | 142, 144, 139, 129 |

The Immortal policy OFF/ON action streams differ on three of four seeds in
each source, and both jobs exit 0. All four Deity policy pairs have identical
canonical action streams within their source. Both Deity jobs therefore exit
2 under the existing no-contrast guard. Those failed contrast gates are
retained, not waived or rerun with different seeds. Their completed outcomes
are descriptive only; they are not a successful Deity treatment screen.

All eight OFF outcomes match across sources in every reported field. All
eight ON outcomes also match. The tool does not persist cross-source canonical
actions, so neither observation proves cross-source action identity. No
positive effect on wins, conquest, survival or speed is established.

### The repair does execute in complete games

ON journals change actual endurance numbers in four of eight seeds, across
16 journal lines, without changing the recorded readiness answers. For
example, Immortal seed 43240002 at turn 47 reads Mbanza Mbamba as 25.1 turns
against 189.0 endurance before the repair and 161.0 afterward: still ready in
both. The nearby muster separately holds at 120 strength against a bill of
131. Immortal seed 43240000 at turn 78 changes the Panama City endurance from
672.2 to 607.3, again leaving its 2.0-turn budget ready.

Six OFF journals match byte-for-byte. The other two differ only in the order
of an existing refused-move diagnostic within the same turn; their exact
full-line multiplicities, including turn labels, match. ON journal content
likewise matches after explicitly removing only numeric endurance values and
ignoring line order. Raw hashes and all differences are retained: this
diagnostic comparison is not an action-equivalence substitute.

The accounting repair is supported, but it does not solve the demonstrated
conquest failure. The next investigation should trace effective coordinated
pressure and force arrival after an entry already reads ready, alongside the
economic race lost to rival Science/Culture. Lowering readiness thresholds or
claiming that this repair creates higher-difficulty wins is not supported.

## Reproducibility and limits

- Baseline evaluator source: `77378a67ff2a727bb15659cb32520551f7d12ea9`.
- Candidate evaluator source: `66199bcfdb7f9c6055aeb874858f1f27b06b4a4d`.
- Baseline executable SHA-256:
  `23c4554d8407cd2e3fa686f533580e746381c18bf0fbffc13235ce556be5401e`.
- Candidate executable SHA-256:
  `14c53a5fe9c38217cce08f0728301648a9ec1a63e4c6ac661b46ea1546208776`.
- Compact results: `2026-10-16-siege-swarm-endurance.result.json`.
- Complete rows, registrations, receipts, commands, audit source, test logs,
  Native lead extracts and hashed journal comparisons:
  `2026-10-16-siege-swarm-endurance.provenance.json.gz`.
- Raw external evidence and frozen executables remain under
  `~/civvis-tactics-results/2026-10-16/siege-swarm-endurance/`.

These are CI-profile, ARM64 simulator games against CIVVIS, not Firaxis Native
verification wins. Concurrent compilation and rungs make them unsuitable for
a speed estimate. The accounting correctness result does not constitute a
strength improvement or a gene-promotion screen. No Native service is
manually restarted or reconfigured; no verification setting, pin, difficulty
or safety threshold is changed for this task.
