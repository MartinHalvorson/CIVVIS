# Aluminum colonies beside allied city-states

Native King run `civvis-20260926T211003Z`, pinned to `72b0d8bee`, knew
Advanced Flight by turn 158 but had no Aluminum income. Its first turn-160
board has no known deposit on our own or our suzerains' land. Two unclaimed
deposits, offset `(1,17)` and `(2,18)`, are model-legal city sites, fourteen
route steps from Settler 8650756. Both are five or six tiles from Geneva
and Kabul, whose suzerain is our seat. The blanket six-tile city-state
settlement exclusion removes them before scoring.

An exploratory counterfactual suppressing only the minor city centers'
exploration records ranks those deposits first and second, at 193.65 and
175.24, versus the original best site's 138.86. This is diagnostic only;
altering that knowledge is not the proposed policy. Speculative founding
forecasts -21.18 Loyalty/turn at `(1,17)` and +2.59 at `(2,18)`. The first
must remain excluded. Both have fully explored nine-tile neighborhoods;
neither has an unresolved major border within five tiles. A friendly
88-HP Musketman is four tiles from the second deposit.

## Preregistered scope and validation

Consider a narrow exception to the city-state buffer for an unclaimed,
revealed city-center deposit needed by the committed Domination bomber
wing. Keep the ordinary founding spacing and terrain checks, current
hostility checks, full frontier protection, nonnegative speculative Loyalty,
and nearby healthy land defender requirement. Every city-state whose buffer
covers the site must currently be ours to command. Route selection, escort
movement, failed-host-site memory and target reservations remain unchanged.
There is no new settlement score bonus or forced march.

Freeze baseline production source `3e11ca75a` and an orders CLI before editing
policy. First reproduce the excluded safe resource site in a failing unit
test; keep the existing generic city-state buffer regression passing. Check
refusals for missing need, commitment, visibility, safety and permission.
Compare the same immutable first turn-160 prefix with late-start CLIs using
the recorded native forced-policy bundle. Preserve every proposed order and
journal. Fresh-controller proposals do not reproduce persistent native
memory and do not imply native execution.

The immutable prefix SHA-256 is
`6ab6f570fa12ee6fd8232d6149895e11563cc17e462ec837281711d73d851d4f`.
Artifacts live under `~/civvis-tactics-results/2026-09-26/aluminum-colony-sites/`,
with the read-only prefix and exploratory probes in the neighboring
`air-readiness/211003-turn160-frame0/` directory.

Future simulator evaluations use the operator's newly requested Prince
difficulty, Simon Bolivar, Tiny Pangaea and Domination objective. Historical
native evidence above remains King. A profile-selection change is owned by
the siege-progress evaluation task; no King-only simulator run will be
relabeled or used as a substitute for the new requested profile.

Before policy editing, freeze the evaluator harness from
`9dda6319d413cb11bc5fdda55dfa487a6c1f4c11` in an external artifact directory.
Compile this identical harness against each source's CI-profile library; the
wrapper only forwards arguments. The harness's two source files, forced-policy
bundle and wrapper hashes are recorded in the artifact manifest. Run two
complete pairs per source, without replacing seeds or stopping for outcomes:

```text
--domination-pair air-surge-2 --difficulty prince --games 2 --start-seed 37870000 --out SOURCE-pairs.jsonl
```

This explicitly sets both major and barbarian difficulty to Prince. Other
profile dimensions remain four majors, six city-states, 60x38 Tiny Pangaea,
Online, all victories and a 250-turn clock. The off arm stays as a control;
all four focal results per source are retained. Rivals use CIVVIS policies,
not Firaxis AI. This small diagnostic cannot establish consistent wins or
justify a deployment promotion.

## Intermediate observations retained

The regression failed before policy editing because the six-tile buffer
excluded the otherwise supported resource site. The first candidate removed
that exclusion in the test, but the fixture's flat, unimproved grassland
failed the ordinary settlement value floor. Giving the fixture productive
hills made it a worthwhile site without changing the production scoring.
Five focused tests then passed, including nineteen refusal cases, negative
Loyalty, cached arrival and stalled founding. The existing ordinary six-ring
city-state buffer test also passed.

Candidate `5226f52e8` left the actual turn-160 replay unchanged. Both southern
Aluminum deposits were explored but outside current sight. Requiring current
sight before even planning the trip therefore hid both candidates again.
The refinement uses the recorded, explored resource for planning and retains
the fresh eligibility check before founding. It does not grant sight or
ignore newly observed ownership, hostility or Loyalty. The initial candidate
and its complete paired results remain in the artifact directory.

The identical external evaluator harness passed all nine tests, including
actual major-player and barbarian Prince-rule readback. Baseline completed
both registered pairs: seed 37870000 won Science off and Score on; seed
37870001 lost Score off and Culture on. There were no Domination victories.
