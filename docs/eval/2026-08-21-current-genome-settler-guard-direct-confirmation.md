# Current-genome settler guard direct confirmation

_2026-08-21 · `f023a0145e86467c39132651dceb3ca11a43904b`_

## What was asked

The current-genome prior health check gave `settler-guard-holds` a
family-wise-positive lead. Does that flag improve the deployed native genome
when it is the only changed arm?

## How it was measured

This was a 300-map, six-player Pangaea foldover at Online speed, 250 turns,
and six city-states. Every major was treated, with civilizations shuffled per
map. The ledger's `best` genome held every other native treatment at its
deployment state; the two arms differed only in `settler-guard-holds`.

```bash
target/ci/gene_screen \
  --pairs 300 --players 6 --width 60 --height 38 --city-states 6 \
  --speed online --map pangaea --turns 250 \
  --genes settler-guard-holds --baseline best --field advanced \
  --design foldover --all-seats --randomize-civs \
  --start-seed 57010000 --jobs 4 \
  --out /tmp/civvis-p9-settler-guard-holds-direct.jsonl
```

All 600 games completed, yielding 1,800 treated-seat pairs. The raw arm
genomes differ at the flag's index, and the screen applies that desired bit
after its setup bundle; the null is not caused by a ledger overwrite. The
machine-readable analysis is
[`2026-08-21-p9-settler-guard-holds-direct-6p-allseats-300-pairs.json`](../gene_screens/2026-08-21-p9-settler-guard-holds-direct-6p-allseats-300-pairs.json).

## What it measured

`settler-guard-holds` was exactly neutral: 16.7% wins on and 16.7% off,
for **+0.0 pp** (z = 0.00); score share was also **+0.00 pp** (z = 0.00).
The paired adjusted estimate was +0.0 +/-0.0 pp. All 1,800 matched arm rows
had identical recorded gameplay outcomes; only elapsed runtime differed.

That strict null contradicts the +4.0 pp broad prior-screen lead from the
health check. The direct arm isolates the flag in the deployment genome, so
it is the decision-bearing result; the broad screen was useful for ranking
the candidate, not for promotion.

## What was decided

No default changed. `settler-guard-holds` remains unresolved and off in the
ledger. Its prior 13,446-pair native screen was already null, and this
current-head direct check finds no measurable path to price. It should not
receive a matrix or live-withhold gate unless a future behavioral change makes
the isolated arm non-inert first.
