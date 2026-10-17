# Native siege-road memory rollout and failed replay control

## Status and scope

The persistent replay control **failed**. The registered A/B comparison is
therefore **not run**. All three consecutive, preselected Native attempts
are terminal; they are not a paired strength experiment.
No highest-difficulty win, conquest win, speed improvement, or causal
strength improvement is established.

This follows the city/unit identity-memory repair in PR #4084, revision
`7f5232e8a657478225a9d8b81bb62d2d486d6f7a`. This task changes evaluation
documentation only, not production code, treatment selection, Native policy,
difficulty, executable pins, or the verifier. The unattended Native seat
continues normally; no game is restarted for this study.

## Registered selection and gates

The adjacent registration JSON was written at **2026-10-17 04:50:22 UTC**
(October 16, 21:50:22 PDT), before the first selected attempt began at
04:50:45 UTC. It fixes:

- Every original state frame, in file order, from the beginning of
  `civvis-20261017T025117Z` through turn 150; no alternate world or suffix.
- An A/A control using the same frozen `53ee80a45` executable in both arms,
  its actual SHA-256 checked, and the original selected 271 forced treatments.
- A strict full-history gate: every complete orders/decision reply must match
  the original Native record, both children must exit zero, identities must
  match, and inputs must remain unchanged. Self-agreement alone is insufficient.
- A/B only if that gate passes. Failure is retained rather than tuning the
  input prefix, selecting another world, or silently restarting the study.
- The first three newly created normal Native run directories absent from
  the registration's existing-ID snapshot, ordered by run ID, regardless of
  source, settings, errors, termination, or result. No replacements.

Registration was local, not externally notarized. The first GitHub evidence
checkpoint was pushed after the first selected attempt had terminated. The
unchanged original registration and contemporaneous observer receipts are
retained; the JSON timestamp alone is not independent proof of registration.

## Persistent A/A result

The existing `tools/native_city_route_replay.py` harness processed **411**
frames in 25.281 seconds. Both children exited zero, all checked inputs were
unchanged, and all request identities were valid. The two identical arms
agreed on every complete reply and order. Nevertheless, only **328 of 411**
frames matched the recorded Native decisions: **83 mismatched**. The harness
history gate and the stronger registered gate both failed.

The original game's recorded source is `44629581fd661fdf03f94a3801689f615c09e40b`,
not the available frozen `53ee80a45` executable used for this A/A. Thus this
also tests an unproved cross-revision baseline-fidelity assumption. Matching
both replay arms does not establish equivalence to the original executable,
and the mismatches cannot all be attributed to event boundaries.

The earliest mismatch is index 0, turn 1, frame 0. Native recorded five
orders, including research, a warrior fortification, founding the capital,
and two policies. The replay instead returned no orders and the note
`no revealed terrain or no state yet`.

In the immutable original event stream, the first `state` occurs at line 67
(byte range 15074–22771), before the first `tiles` at line 70
(23330–28623) and `tiles_done` at line 71 (28623–28799). The replay harness
cuts a request prefix immediately after its state event. Thus its first
request lacks the terrain that the recorded Native decision used. The
original run does not retain an exact per-request input-read capture that
would resolve the actual Native read boundary.

This explains the first mismatch, **not necessarily all 83**. Do not treat
the failed control as a counterfactual policy result. A future fidelity
study needs independently specified request-read capture; this study does
not repair or tune the replay harness after seeing its outcome. It also
needs the exact original executable and settings as its control.

## Complete Native cohort

The observer's final receipt is **2026-10-17 05:16:34 UTC**. All three
selected games have final summaries and no live game/brain/decider process
for their exact run IDs. The unattended seat has moved to its next game;
that fourth attempt is not a replacement or an additional study row.

All three frozen executables report revision `7f5232e8a` and have actual
SHA-256 `5696eff093a6b71c3b734f1a79651505d03623e0d65c4afed2f20868237d7e7f`,
matching their provenance. All three have configured readback true,
Emperor, Gathering Storm, Tiny Pangaea, Online, Gran Colombia, Domination
target. Their independently recorded map/game seeds differ.

| Run | Terminal cause | Cities taken / lost | Kills / losses | Original capitals held at last own roster | Road stand-down lines |
| --- | --- | ---: | ---: | ---: | ---: |
| `civvis-20261017T045045Z` | Abandoned t150, production rank 4 | 0 / 4 | 29 / 37 | 0 | 0 |
| `civvis-20261017T045855Z` | Abandoned t150, production rank 3 | 0 / 0 | 54 / 29 | 1, home Bogotá | 0 |
| `civvis-20261017T050815Z` | Abandoned t150, production rank 3 | 0 / 0 | 20 / 22 | 1, home Bogotá | 0 |

The unchanged abandonment policy permits at most production rank 2 at
turn 150. None has a victory or defeat event. These are verifier-abandoned
attempts, **not completed game losses**, and no conquest victory or foreign
original-capital holding is observed. City counts use the existing Native
summary's audited ownership-transition accounting, not issued attack orders
or repeated occupation callbacks.

The journal contains no `Standing down a siege the army cannot reach`
line in any selected attempt. Therefore this cohort **does not demonstrate
runtime firing of the repaired memory path**. It also cannot establish that
the repair improved strength or caused these outcomes.

| Run suffix | Objective-force Siege target mentions, first–last observed turn | Stage / invest / reduce / take journal lines |
| --- | --- | ---: |
| `045045Z` | Ürümqi 77–111; Ulaanbaatar 112–119; Cumaná 134–148 | 47 / 5 / 1 / 0 |
| `045855Z` | Óbuda 89–121; Szeged 122–129; Nagoya 130–144; Sendai 145–150 | 50 / 14 / 1 / 0 |
| `050815Z` | Wroclaw 39–96; Kraków 112–115; Bydgoszcz 116–117; Nicomedia 120–150 | 84 / 7 / 6 / 0 |

Target spans summarize `for Siege NAME | objective` force mentions, not a
single canonical primary plan; gaps and multiple targets in one turn are
possible. Stage counts are journal lines, not unique turns, host-executed
actions, or captures. Complete turn-indexed mentions and stage lines remain
in the archive.

One concrete next investigation is assembling the actual force before an
early declaration, already the subject of PR #4036. The third game's
turn-39 Wroclaw journal reports five assigned units but only one staged;
turn 41 reserves a scout taker; by turn 46 it reports 100 walls and no
wall-breaker. This is a lead for inspecting the real opening and allocation
path, not proof of a policy defect or a justification to weaken danger or
declaration limits. The failed persistent control cannot supply its causal
counterfactual.

## Artifacts and validation

The local evidence root is
`~/civvis-tactics-results/2026-10-16/native-road-rollout/`.
`registration.json` is the original registration; `aa/` retains both complete
decision streams and journals plus `provenance.json` and
`native-control.json`. Original inputs are hash-frozen under the preceding
`native-muster-approach/frozen-original-inputs/` study. Completed cohort
attempts are copied into `native/<run>/` with before/source-after/copy hashes
equal; the raw streams are not committed to Git.

The adjacent `.result.json` records the final cohort, failed A/A gate,
unchanged registration digest, and compressed-archive digest. The adjacent
`.provenance.json.gz` contains the exact registration, A/A provenance and
all mismatch coordinates, the earliest complete mismatch, original source
summary and event ordering, both observer sources and restart receipt, final
observer state, complete cohort summaries, file manifests, independently
recomputed event/journal evidence, test receipts, and the archive builder's
source. Primary event/decision streams and executables remain external;
the archive is a compact audit receipt, not a self-contained replay bundle.

The final audit checks the selected IDs against every new normal directory,
all source/copy digests, actual frozen executable digests, independent
last-state capital/victory extraction, and journal firing counts. It also
checks that the A/A decision streams are byte-identical and that no A/B
output directory exists. The observer is now exited, not left polling.

The observer was hardened before any terminal cohort result to check actual
executable hashes and explicit local/team victory identity. An ambiguous
observer-only stop selector aborted without signaling; its shell continued
and briefly launched a second read-only observer. Both owned Python readers
were then stopped, with no terminal results yet, and one hardened reader
resumed. Native processes and selection rules were unchanged. Observer
versions and the restart receipt are retained; duplicate interim observer
receipts are not independent attempts.

`python3 -m unittest discover -s tools -p test_native_city_route_replay.py`
passes all **28** tests. That tests the harness implementation, not the
failed original-history fidelity assumption. The fresh local
`cargo test --profile ci --locked` run passes **6,155**, fails **0**, and
ignores **56** tests. The tested Rust source tree is unchanged from `7f5232e8a`
and remains identical after the final fetch/merge check. `git diff --check`
passes. Engine soak is not applicable to this documentation-only study.
