# claude-evolver, 2026-07-31 — strategy-evolution session artifacts

Durable copy of a session whose scratchpad
(`/private/tmp/claude-501/-Users-martin/788e5b60-…/scratchpad`) will not
survive. The write-ups that matter are in the repository, not here:
`docs/EVAL.md` (the dated section "why the shipped genome's sign flips between
the promotion profiles"), `docs/PLAN_CITY_TARGET.md`, and the amendment to
`docs/GENOME.md` — all on PR **#684**.

## What is here

| file | what it is |
|---|---|
| `SYNTHESIS.md` | the whole session in one page |
| `r3-shippable-best.json` | the candidate genome: gen-14 with its eleven economy+expansion genes reverted to `Weights::default()`, **all forty written explicitly** so a future change to `Weights::default()` cannot move it silently |
| `CHECKSUMS.txt` | md5 of every artifact that was measured |
| `prereg-*.md` | the five pre-registrations, also filed as `/Users/martin/civvis-*-preregistration.md` |
| `pr-body-champion.md` | a prepared PR body for replacing the shipped champion, **if and only if** the gate passes |
| `logs/` | every `ai_eval` log the conclusions rest on |

## The candidate in one table

Against stock `advanced`, `ai_eval`, arms differing on `weights` alone:

| profile | shipped champion | `r3` candidate |
|---|---|---|
| compact 4p 24×16 Standard/500, seed 61,000,000 | 56.9%, +48 | 55.6%, +39 |
| deployment 6p 74×46 Online/250 | 42.5%, −53 (seed 61,000,000) | 57.5%, +53 (seed 66,000,000) |

`r3-shippable-best.json` was verified **byte-identical in play** to the form
the gate measured: both were run on the same five-map prefix and their
evaluator output diffed clean.

The gate is `ai_eval advanced_evolved advanced --matrix --pairs 120 --jobs 5
--seed 67000000`, run from a directory staging the candidate at
`evolved/best.json`. ⚠ `MATRIX_PROFILE_SEED_STRIDE` is 1,000,000, so its
**deployment** child runs on seed **68,000,000** — the same prefix the
`g44-41` screen uses. Those two arms are therefore *paired on shared maps*, not
independent, and neither may be quoted as a replication of the other.

## Reading order for anyone picking this up

1. `SYNTHESIS.md`.
2. `docs/EVAL.md`'s dated section, for the evidence and the caveats.
3. `prereg-policy-deck-extension.md` — the other promotable candidate, whose
   recorded 300-map run is +30 at deployment and inconclusive only on the
   fixed-*n* Wilson bound.
4. The memory note `civvis-champion-does-not-transfer-to-deployment` for what
   is still running and what to resume.

⚠ Nothing here is promoted. A champion replacement touches 38 evaluator arms,
the league seeding and the embedded fallback, so every strength number measured
against `advanced_evolved` before such a change is measured against a different
agent after it.
