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

## Revised native-board replay

Source `afa7f85fd` passes six focused tests and the changed-line Rust quality
check. On the unchanged turn-160 board, its actual site ranking contains
31 sites instead of 30: `(2,18)` is newly admitted at rank one, value 175.24.
The unstable `(1,17)` remains absent. No score bonus was added.

The full late-start controller now selects offset `(2,18)` for Settler
8650756, a fourteen-step escorted expedition. Baseline selected `(40,10)`,
an eighteen-step expedition. Both controllers assign a guard and wait for it
to stack; neither emits a movement order for the Settler on that frame.
This establishes changed destination planning, not an executed trip, a
founded native city or received native Aluminum.

| Frozen orders CLI | SHA-256 |
|---|---|
| Baseline | `459f01c229a922afd303e68c0b4eff836dfacb81775f26879f2274d3ca7616cc` |
| Initial candidate | `197b8539dedff3aa0fadcac82617a2f9e6c0096e5882eb2296e6eb1257de7353` |
| Charted-site candidate | `31e0e6ceba1843ad7d7dcdf3a22ca129c8dbf01b5232c4111712e75537e2f58b` |

The full `cargo test --profile ci --locked` suite at `afa7f85fd` passed
4,334 tests with 53 existing ignores. The source and harness freezes above
precede integration of subsequent independent mainline fixes, so the recorded
before/after comparison isolates the colony change.

## Complete Prince comparison

Baseline, initial candidate and charted-site candidate all completed both
registered pairs. All four focal outcome records are identical across the
three sources, including action counts and conquest telemetry. There were
no crashes, substituted seeds or early stops.

| Seed | Air-surge-2 arm | Turn | Score | Focal outcome | Major cities observed held | Capitals held at end |
|---|---|---|---|---|---|---|
| 37870000 | off | 243 | 1221 | Science win | 2 | 0 |
| 37870000 | on | 250 | 1487 | Score win | 7 | 1 |
| 37870001 | off | 250 | 806 | Score loss | 0 | 0 |
| 37870001 | on | 167 | 668 | Culture loss | 0 | 0 |

None is a Domination win. This comparison shows no outcome benefit from the
colony change. The narrower native-board planning correction is established
by the unchanged-prefix replay and its retained escort requirement; a native
colony, Aluminum income and stronger conquest results still need observation.

| Frozen evaluator | SHA-256 |
|---|---|
| Baseline | `230ed168cc7e25926b053645d1598ccfc4bdbc058885ca6d4f8a45c352f12ed3` |
| Initial candidate | `eef1ac81faafbcfc09e7719993b001a870a08c40b78dcd4659fee0c32af259ab` |
| Charted-site candidate | `7a0b297a317f54b30a4adda2b85e2bd34e68b47c5649c40be45711cc261eadf5` |

Main was integrated afterward through `e58d4a7c4`, including the Bomber queue
reservation and independent Builder-raider policy. No engine or Lua source,
live deployment defaults, promotion ledger or active native runtime changed
in this colony patch.

After integration, the full suite passed 4,337 tests with 53 existing
ignores. Changed-line Rust quality and whitespace checks passed.

## Knowledge check before merge

Final review found that the arrival-only refusal inspected the underlying
resource without first checking its revealing technology. An additional
regression failed before Radio: the simulator knew a hidden Aluminum deposit
and changed its founding refusal. The merge was held before landing. The
correction requires both the revealing technology and charted terrain before
that specialized refusal can apply. Ordinary founding safeguards continue to
handle every other site. A second control covers uncharted terrain.

The earlier frozen candidates and complete results remain above. Before
running the corrected comparison, register a new two-seed block, 37870002–3,
with the same external Prince harness and all other profile settings unchanged.
Both the baseline and corrected candidate include main through `e58d4a7c4`;
the baseline is a separate detached, read-only checkout with no source edits.
This keeps the independent queue and Builder changes out of the difference.
Both sources must complete both pairs, with all outcomes retained. The
protocol is also frozen in `knowledge-guard-protocol.json` in the artifact
directory. These small diagnostic runs remain insufficient for promotion or
a claim of consistent Domination wins.


The knowledge-corrected source `04b41131ff808c6235fe989d691c9ee11d6e8965`
passes seven focused tests and the full suite: 4,338 passed with 53 existing
ignores. Changed-line Rust quality and whitespace checks pass. Its frozen
turn-160 replay still chooses Aluminum `(2,18)`, route fourteen, and waits
for the assigned escort. The correction preserves the intended planning
change without using unrevealed resources or uncharted terrain.

Both sources completed both new pairs without crashes, substituted seeds or
early stops. Their complete pair records are identical, including action
counts and conquest telemetry:

| Seed | Air-surge-2 arm | Turn | Score | Focal outcome | Major cities observed held | Capitals held at end |
|---|---|---|---|---|---|---|
| 37870002 | off | 250 | 1266 | Score win | 3 | 0 |
| 37870002 | on | 238 | 1687 | Science win | 13 | 2 |
| 37870003 | off | 159 | 361 | Religious loss | 0 | 0 |
| 37870003 | on | 183 | 403 | Religious loss | 0 | 0 |

Again, zero Domination wins and no measured outcome benefit. The thirteen
major cities and two capitals in one arm also occur on the baseline and
cannot be attributed to this patch. All raw records and comparison checks
are retained in `knowledge-guard/` beside the original artifacts.

| Corrected comparison artifact | SHA-256 |
|---|---|
| Baseline evaluator (`e58d4a7c4`) | `681099adf3a31dd836ea0234c8fdaffc9c2674f369298c5bdfa8be593024dabb` |
| Corrected evaluator (`04b41131f`) | `53b68cf5085ae72e77e62f3fb091b71e7c5d151ca3afdd23b978be20281a8cd9` |
| Corrected orders CLI (`04b41131f`) | `19dc2a1128b96bcc896d26274a3503f0561f8c90e6f0a853e1324e9cb6851314` |

These source freezes precede any subsequent mainline integration. This patch
establishes guarded destination planning only; native Aluminum income,
Bomber use and consistent Domination wins remain unproven.
