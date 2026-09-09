# Campus genes under a rival handicap

_2026-09-09 · `7dc0e3d68d5a` · pre-registered before any game ran_

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
stays usable. Build from a clean worktree at the commit named in the
provenance block, so `tools/genes.py` accepts the source stamp:
`cargo build --release --features developer-tools --bin gene_screen`.

Command per gene:

```
target/release/gene_screen --games 6 --target-games 6 --start-seed <first> \
  --jobs 2 --genes <tag> --p-on 0.5 --difficulty emperor --rivals firaxis-mix \
  --handicap rivals --rival-chairs 3 --out <rows.jsonl>
target/release/gene_screen --analyze <rows.jsonl> \
  --json docs/gene_screens/2026-09-09-campus-handicap-<tag>.json
```

## What it measured

Provenance: every artifact stamps source `7dc0e3d68d5a` (build-tree, clean),
gene set sha `fd813871d25a`, binary sha `7edc864adc7c`, `gene_screen` built
with `--features developer-tools` from this branch. All four probes completed
6 of 6 games, 18 of 18 measured seats, seeds exactly as reserved. Artifacts:
`docs/gene_screens/2026-09-09-campus-handicap-<tag>.json` (complete
analyzer output); raw rows stayed in the session scratchpad.

Deltas are on minus off across the 18 measured seats of each probe; SEs are
the analyzer's clustered-by-game errors for win and share and its own
per-seat errors for the science columns. `science_pace` is Δ techs at turn
150; `techs_end`/`science_end` are Δ techs and Δ accumulated science at the
end of the game.

| Gene | On / off | Win Δ ± SE (pp) | Share Δ ± SE (pp) | science_pace Δ ± SE | techs_end Δ ± SE | Read |
|---|---:|---:|---:|---:|---:|---|
| `campus-before-halfway` | 11 / 7 | +3.90 ± 21.58 | +2.97 ± 2.30 | **+3.78 ± 0.99** (z +3.8) | +6.79 ± 4.89 | ~ |
| `campus-through-expansion` | 8 / 10 | +12.50 ± 20.25 | **+5.27 ± 2.21** (z +2.4) | +1.33 ± 1.43 | **+14.92 ± 5.08** (z +2.9) | share helps (thin) |
| `first-research-building-reserve` | 11 / 7 | -10.39 ± 16.85 | -1.34 ± 1.79 | -0.44 ± 2.52 | +3.97 ± 5.56 | ~ |
| `boost-first-research-2` | 11 / 7 | +3.90 ± 21.93 | -2.81 ± 2.98 | +0.10 ± 2.03 | -1.38 ± 6.85 | ~ |

Win rates on/off: `campus-before-halfway` 18.2% / 14.3%;
`campus-through-expansion` 12.5% / 0.0%; `first-research-building-reserve`
18.2% / 28.6%; `boost-first-research-2` 18.2% / 14.3%. The win column resolves
only 47–61 pp at 80% power in each probe, so no win reading here is a result;
the share and science columns (resolving 5–8 pp and ~1–2.5 techs) are the ones
that carry any signal. Endings ran 50–67% science in three of the four
batches, so the probes exercised the research race they were meant to.
`science_end` agrees in sign with `techs_end` in every probe
(`campus-through-expansion` +113 ± 37 science, z +3.1; the other three inside
one SE).

## What was decided

Nothing is promoted and no ledger source is added: the batch changes the
difficulty, rival and handicap legs, so `tools/genes.py` refuses it as a
ledger source by design. The force file was not edited by this change. The
per-gene reading on the question asked — should each stay forced on the
live seat — is:

- **`campus-before-halfway` — stay forced.** The only column with power,
  science pace, reads +3.8 techs at turn 150 (z +3.8) with share and end
  techs both positive. Nothing here says it hurts a handicapped seat; it is
  the one campus gene the self-play screen never measured, and this is its
  first reading.
- **`campus-through-expansion` — stay forced.** Under a rival handicap it
  reads the opposite sign to the self-play rank-234 row: +5.3 pp share past
  the family-wise bar, +14.9 techs and +113 science at the end. The self-play
  penalty is not evidence against it on the live shape; a Science seat
  pricing its Campus early is exactly the spending a handicapped race
  rewards. Thin (18 seats), but every column points the same way.
- **`first-research-building-reserve` — does not earn its forced seat on
  measurement.** Win, share and science pace all read negative here
  (inconclusive individually) and the self-play screen prices it at
  P(>0) = 19%. Its remaining justification is the live observation that
  drove the force (9 Campus districts, 3 science buildings, 2026-08-30),
  which no screen can see. Keep it only on that actuation argument and
  re-verify that gap on the live seat; it is the first of the four to drop
  if the seat needs the production back. Do not add games to this probe to
  change the sign.
- **`boost-first-research-2` — no evidence either way.** Every column is
  inside one SE of zero; share is mildly negative. It stays forced only on
  the v1/v2 argument in its registry comment, not on anything measured here.

Limitations: six games and 18 measured seats per gene, one gene per probe
against the evaluator baseline (not the nine-gene live force file, so
interactions between the forced genes are unmeasured), three measured seats
against three handicapped rivals rather than the live one-against-five
shape, the evaluator's Firaxis mix rather than the Firaxis AI, and the
250-turn Online clock rather than the live game's. A positive reading here is
not a live improvement; a live improvement still needs retained Civilization
VI outcomes on the Emperor rung.
