# Current-genome prior health check

_2026-08-21 · `ab087a700d5a6a96141117270b2bf72bee1adb15`_

## What was asked

After the ledger had selected its twelve helpers and eleven holds, which
remaining native treatment deserved a clean, dedicated confirmation? The
question was deliberately about routing the next test, not changing a default
from a many-gene screen.

## How it was measured

`gene_screen` drew both arms independently from the ledger prior — 11 known
hurts at p=0.10, 46 unresolved genes at p=0.50, and 12 helpers at p=0.90 —
then measured each seat's marginal on-versus-off contrast. The two arms of
each map used the same six civilizations in a shuffled seating order.

```bash
target/ci/gene_screen \
  --pairs 300 --players 6 --width 60 --height 38 --city-states 6 \
  --speed online --map pangaea --turns 250 \
  --baseline best --field advanced --design prior \
  --all-seats --randomize-civs \
  --start-seed 56010000 --jobs 4 \
  --out /tmp/civvis-p8-current-genome-prior.jsonl
```

All 600 games completed: 1,800 treated-seat pairs across 69 native genes.
Religious victories were 59% of games (median turn 152), score 27%, culture
9%, science 5%, and diplomatic six games. The run resolves a win difference
of +/-3.3 points and a score-share difference of +/-0.53 points at 80% power;
the 69-gene family-wise 5% bar is |z| >= 3.38. The machine-readable analysis
is [`2026-08-21-p8-current-genome-prior-6p-allseats-300-pairs.json`](../gene_screens/2026-08-21-p8-current-genome-prior-6p-allseats-300-pairs.json).

## What it measured

| gene | win delta | score-share delta | read |
|---|---:|---:|---|
| `settler-guard-holds` | **+4.0 pp** [+1.8, +6.3], z +3.50 | +0.50 pp, z +2.86 | HELPS past the family-wise win bar; adjusted whole-genome estimate +4.2 +/-1.6 pp |
| `theology-for-founders` | **-4.0 pp** [-6.3, -1.8], z -3.48 | -0.24 pp, z -1.32 | HURTS past the family-wise win bar |
| `civilian-rescue` | -3.9 pp [-6.2, -1.6], z -3.34 | -0.61 pp, z -3.36 | harmful lead just short of the family-wise bar |
| `district-coverage` | +2.5 pp [+0.4, +4.6], z +2.29 | +0.44 pp, z +2.56 | screen lead, below the family-wise bar |
| `idle-faith-patronage` | -5.1 pp [-9.6, -0.7], z -2.26 | -0.95 pp, z -2.74 | conflicts with its dedicated 6,000-pair positive result |

The last row is the important calibration: the p=0.90 helper is rare in its
off arm and the broad prior field is intentionally interaction-heavy. Its
negative result cannot overturn the dedicated screen that made
`idle-faith-patronage` a helper. The same rule protects every older,
higher-resolution ledger row.

## What was decided

No default changed. `settler-guard-holds` stays unresolved and off under the
ledger despite being the only family-wise positive lead: its earlier 13,446
native-pair screen was null, and `gene_screen` is a ranking instrument, not a
promotion gate. It is the next direct arm to price on current head, using
`--genes settler-guard-holds --baseline best` before a matrix/live-withhold
check.

`theology-for-founders` was already off; this result supports holding it out,
but does not replace its targeted 6,000-pair null. `civilian-rescue` and the
other screen flags likewise remain unresolved. The analysis is intentionally
not inserted into `docs/gene_ledger.json`: allowing a 300-map,
prior-interaction health check to supersede dedicated multi-thousand-pair
screens would make the ledger less, not more, reliable.
