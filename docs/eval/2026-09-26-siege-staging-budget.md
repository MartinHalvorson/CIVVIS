# Siege allocation versus staging strength — 2026-09-26

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

Pilot results remain pending. On `mbp-m5-max-128`, each frozen executable runs:

```sh
victory_eval --domination-pair early-conquest-opening --games 4 --start-seed 37140000 --out <fresh-output-file>
```

The separate outputs are
`~/civvis-simulation-results/siege-staging-before-20260926.jsonl` and
`~/civvis-simulation-results/siege-staging-after-20260926.jsonl`.
