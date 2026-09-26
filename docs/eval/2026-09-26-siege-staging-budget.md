# Rejected siege-allocation floors — 2026-09-26

Neither tested allocation floor earned deployment. Four paired seeds per
variant produced zero wins, and a follow-up audit found zero captured major
cities or capitals. The candidate code and its regression tests are preserved
in this PR's earlier commits, but the final contribution changes only this
report. The underlying allocation/staging disagreement remains unresolved.

Native King / Gran Colombia / four-player Tiny Pangaea run
`civvis-20260926T173411Z` declared on Byzantium at turn 75, then took no
cities before losing to Science at turn 235. At turn 76 the objective board
reported Constantinople's siege supplied by two units against a 50-strength
requirement, with no Siege requisition. The tactical siege controller reported
55 staged strength against a 79-strength requirement and stayed in Stage.

The campaign requirement discounts defenders and city strength by the tech
edge, while the siege train's advance gate reads an undiscounted local bill.
If the allocator stops at its smaller requirement, the executor cannot cross
its own staging gate even when reserve bodies could supply the difference.

## Validation plan

Reproduce the disagreement in a world fixture, exercise real force allocation
and the siege state machine, then require the allocation budget to cover the
existing staging requirement whenever the siege doctrine is active. Preserve
the strategic budget when it is larger and when the doctrine is off. Do not
lower the tactical advance threshold.

Before the paired pilot, freeze an unmodified-policy binary from this task's
base. Compare before/after on four full fixed-profile pairs, seeds
37140000–37140003, using `early-conquest-opening` as the evaluator's existing
toggle. Each source version completes both toggle arms; compare matching arms
between source versions. No early stopping and no inference of a win benefit
from a unit test or score increase. Four pairs are a diagnostic pilot only.
Both versions use identical setup and live-policy bundles. The simulator's
opponents are CIVVIS controllers, not Firaxis AI.

## Validation so far

The regression failed on the baseline with `allocator supplied 20 but staging
requires 41.25`; the separate insufficient-army test also failed because the
board requested less strength than staging required. After the fix, all three
focused tests passed: a supplied force enters Invest, an undersupplied force
requests reinforcement, and the doctrine-off/larger-budget cases retain the
strategic requirement. The complete `cargo test --profile ci --locked` suite
passed. Two additional 250-turn-capped King/Pangaea smoke games completed
(seeds 926760–926761); these test completion, not focal domination performance.

Baseline source: `361de7fdb2ada13de0579f3ab998fd4c2669403e` (parent
`1db776f3667225ea7991387212363658621cb266`), with only the new regression tests
added before building the executable. Candidate policy source: `3e9983ed4`.
The evaluators were built using
`cargo build --profile ci --locked --features developer-tools --bin victory_eval`.

- Baseline executable SHA-256: `b4020cafcaece7bb82d4a24933f23ed4c8c37eb90d3de048227f4b62053372c5`
- Candidate executable SHA-256: `5715f544e2fa8eb0991c779972ccf71f69991a9b8b02e4205925eeb2c26e8903`

On `mbp-m5-max-128`, each frozen executable runs:

```sh
victory_eval --domination-pair early-conquest-opening --games 4 --start-seed 37140000 --out <fresh-output-file>
```

The separate outputs are
`~/civvis-simulation-results/siege-staging-before-20260926.jsonl` and
`~/civvis-simulation-results/siege-staging-after-20260926.jsonl`.

## First pilot: broader floor not accepted for deployment

Both versions completed all four pairs; within each source version the
early-conquest toggle arms were byte-identical, so both runners exited 2 as
documented. Across source versions, three seeds changed outcomes or action
counts. Neither version won a game. Foreign-city totals include minors and
must not be presented as capital captures.

| Seed | Before: loss / turn / score / foreign held | After: loss / turn / score / foreign held |
|---|---|---|
| 37140000 | Science / 222 / 974 / 1 | Religion / 191 / 862 / 2 |
| 37140001 | Science / 220 / 615 / 0 | Science / 220 / 615 / 0 |
| 37140002 | Religion / 176 / 472 / 1 | Science / 228 / 645 / 1 |
| 37140003 | Science / 223 / 586 / 0 | Culture / 156 / 356 / 0 |

This does not establish improvement: two changed games ended earlier, and
mean score fell 42.25. Inspection found that `siege_requirement` also serves
prewar readiness. The tactical doctrine operates only against cities already
at war, so changing the prewar budget is outside the defect being repaired.

## Scoped follow-up plan

Limit the floor to existing wars and add a regression proving both doctrine
flags preserve the peacetime requirement. Repeat all four pairs on the same
seeds against the original frozen baseline, retaining both arms and all
outcomes. Store the follow-up separately as
`~/civvis-simulation-results/siege-staging-wartime-20260926.jsonl`.
The first pilot remains a distinct treatment and is not pooled with this one.
All four focused tests pass for this variant. Follow-up executable SHA-256:
`c0f160cccfc58f5141ae1a94ab308c3a563c88424b5ba6342e0125e5c18cf935`.
The full-suite rerun passed. Scoped policy source: `1dde0a446`. All four
follow-up pairs completed, again with byte-identical toggle arms and exit 2.
Every recorded outcome matches the broader-floor table above; restricting
the floor to existing wars did not improve any measured result in this sample.

A separate read-only capital audit replays candidate seeds 37140000 and
37140002, the two seeds with foreign cities held. It links the same compiled
candidate library and adds only an observer distinguishing original major
owners from minors. Its results must match the candidate's turn, victory,
score and action count before its additional ownership fields are used.
Audit output: `~/civvis-simulation-results/siege-staging-capital-audit-20260926.jsonl`.
The audit completed and matched the candidate's recorded turn, victory, score
and action count for both seeds. It found only minor cities:

| Seed | Observed foreign city IDs | Observed major cities | Observed original major capitals |
|---|---|---:|---:|
| 37140000 | 21, 24 | 0 | 0 |
| 37140002 | 21 | 0 | 0 |

The other two seeds held no foreign cities at all. Thus neither variant
demonstrated major-city or capital progress on any of the four seeds. This
is a small diagnostic sample, not a statistical claim about the true win
rate. It does show why passing a consistency regression is insufficient to
justify this proposed fix: the same-seed games did not establish conquest
benefit, and two changed games ended earlier.

Decision: remove the experimental implementation and tests from the final
diff; retain their recoverable source commits (`3e9983ed4` and `1dde0a446`)
and this evidence. Do not alter the live bundle based on these trials. Further
work should investigate how reinforcements actually reach and attack a major
city, alongside the separately observed loss of native war-type information.
