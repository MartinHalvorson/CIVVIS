# Pre-registration — governor-victory-lanes direct confirmation arm

Written **before** the batch starts, 2026-08-23, on `mbp-m5-max-128`.

## Why
The first standard-shape whole-genome screen (3,937 map pairs / 23,622 matched
seat comparisons per gene, seeds 141000000-141003936, source b3ad9f00,
published in PR #2323) reads `governor-victory-lanes` at **-4.73 pp,
win z -15.37** — while the gene ships **default ON**, ranked 6th at +46 on the
retired legacy 60x38 Pangaea instrument. Every ledger source today is legacy.
This arm is independent confirmation on disjoint maps, and it is a single-gene
direct screen rather than a whole-genome foldover, so the reading is not
conditioned on 98 other genes being randomised.

## The design, fixed in advance
- Binary: release `gene_screen` built from origin/main `17a27004d04936bf737cbada42137024e8020d44`.
- Command: `gene_screen --pairs 600 --start-seed 150000000 --genes governor-victory-lanes --out gvl-direct.jsonl`
  (bare defaults ARE the standard screen: 6 majors, 74x46 Continents, 9
  city-states, Online to its own 250-turn clock, all six lanes, best-genome
  baseline, all-seats foldover, shuffled civs.)
- **Pre-registered target: 600 complete map pairs**, seeds 150000000-150000599
  — disjoint from the whole-genome screen's 141000000-141003936, so this is new
  evidence and not a re-read of the same maps.
- Analysis: `gene_screen --analyze gvl-direct.jsonl`, reporting on-off win
  Δpp, win z, share Δ and share z, over complete pairs only.

## The decision rule, fixed in advance
- **Confirmed harmful** if the on-off win Δpp is negative with |win z| >= 3.
- **Not confirmed** if |win z| < 3, whatever the sign — and in that case the
  whole-genome screen's single largest reading did not survive a direct arm,
  which bears on every other conclusion drawn from that batch and must be
  reported as prominently as a confirmation would be.
- **Contradicted** if the Δpp is positive with win z >= +3.
- Reporting actual-vs-intended pair count is mandatory whatever happens; a run
  that stops early is a partial screen and says so.

## Power
At z -15.37 on 3,937 map pairs, 600 pairs detects an effect one third that size
at |z| ~ 4. A single-gene screen also pairs far tighter than a whole-genome one
(the ranking's own table: 1 gene / 6,000 seat pairs gave +/-29 at pairing gain
3.32x, against +/-51 for 75 genes on 17,574), so this is generously powered.

## What it does NOT decide
Nothing here flips a default. The published rule decides that from the ledger,
and entering the standard screen as a ledger source is a separate change.

## Binary provenance, captured 2026-08-23 (post-hoc but pre-completion)

PR #2331 (`8162f7a9`) landed a build stamp *after* this batch started, so this
run's artefact carries no in-band `build` block and `tools/gene_ledger.py` will
refuse it as unstamped. Recording the equivalent by hand, while the binary is
still on disk and the tree still clean:

- built from commit `17a27004d04936bf737cbada42137024e8020d44`
- `binary_sha256` `bc57c795ed1a6c968932614c56acd51f8be7d9d737b9bfa40a96e58d77bc11b5`
- build tree `/Users/martin/civvis-batch-runner`, **0 dirty files** at build and
  at capture
- seeds 150000000-150000599, single gene `governor-victory-lanes`, standard shape

Entering this as a ledger source therefore requires the documented escape,
`--unverified-build`, with this file as the stated reason — NOT a silent pass.
The escape exists for exactly this case (a binary predating the guard); the
honest thing is to use it and record why, not to re-run three hours of compute
to obtain a stamp for a result whose provenance is already written down.
