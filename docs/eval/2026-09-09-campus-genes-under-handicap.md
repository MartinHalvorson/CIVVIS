# Campus genes under a rival handicap

_2026-09-09 · pre-registered before any game ran; source stamp recorded below_

## What was asked

The live Emperor seat forces four campus genes on through the
`~/.civvis-live-force-on` file: `campus-before-halfway`,
`campus-through-expansion`, `first-research-building-reserve` and
`boost-first-research-2`. The self-play screen (`GENE_HEURISTIC_RANKING.md`)
prices two of them as harmful — `campus-through-expansion` at rank 234 with
P(>0) = 16.8%, `first-research-building-reserve` at rank 218 with
P(>0) = 19.2% — and `campus-before-halfway` is unmeasured. That screen plays
six equal seats with no difficulty handicap, so a gene that costs production
early may price differently when the rivals carry Emperor bonuses and the
research race is the one the seat must win. The question: does each gene help
or hurt a measured seat whose rivals are handicapped, and should each stay
forced on the live seat?

## How it was measured

Protocol as in `docs/eval/2026-09-09-investment-discipline.md`: one gene per
probe, `gene_screen` at
`--difficulty emperor --rivals firaxis-mix --handicap rivals --rival-chairs 3
--p-on 0.5`, so three measured seats without the Emperor bonuses face three
boosted rival seats per game. Every other gene draws from the evaluator
baseline. This is a reach probe, not a promotion trial, and not the live
one-player/five-rival shape.

Pre-registered before execution, and not changed afterwards:

| Gene | Seed window | Games | Measured seats |
|---|---|---:|---:|
| `campus-before-halfway` | 109095000–109095005 | 6 | 18 |
| `campus-through-expansion` | 109096000–109096005 | 6 | 18 |
| `first-research-building-reserve` | 109097000–109097005 | 6 | 18 |
| `boost-first-research-2` | 109098000–109098005 | 6 | 18 |

Twenty-four games, 72 measured seats, 72 excluded rival seats. The sample
size will not be increased to obtain a favourable sign; at 18 seats per gene
the win column resolves only a delta of roughly 45 pp at 80% power, so the
readings below are directional at best and an inconclusive result is the
expected outcome. Two workers per probe, four workers total, so the host
stays usable. Build: `cargo build --release --features developer-tools
--bin gene_screen` from a clean worktree at the commit named in the
provenance block, so `tools/genes.py` accepts the source stamp.

Command per gene:

```
target/release/gene_screen --games 6 --target-games 6 --start-seed <first> \
  --jobs 2 --genes <tag> --p-on 0.5 --difficulty emperor --rivals firaxis-mix \
  --handicap rivals --rival-chairs 3 --out <rows.jsonl>
target/release/gene_screen --analyze <rows.jsonl> \
  --json docs/gene_screens/2026-09-09-campus-handicap-<tag>.json
```

## What it measured

_(filled in after the probes complete)_

## What was decided

_(filled in after the probes complete)_
