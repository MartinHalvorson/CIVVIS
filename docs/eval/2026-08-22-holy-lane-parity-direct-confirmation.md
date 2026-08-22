# holy-lane-parity direct confirmation

_2026-08-22 · `docs/gene_screens/2026-08-22-h1-holy-lane-parity-direct-6p-allseats-1200-pairs.json`_

## What was asked

`holy-lane-parity` left the code on 2026-08-21 with the bottom ten of the
ranking (#2266), recorded there at **−27** and listed as a directive removal
rather than a measured harm. Two things then happened that the cull could not
have seen.

The −27 came from `2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs`,
a four-gene screen whose column band is **±64** (#2300). It was a null, not a
reading against the gene.

And P10 was already in flight when the cull merged: its simulation binary is
`d23f92d944cd889aa4c9dfe58c37aceb8e55eabd`, built 1h43m before `77332750`, so
it still carried all 75 genes and priced this one after its code was gone —
**+63 wins/10k at z +3.48**, past P10's own family-wise bar of 3.403. Of the
nineteen genes in *Removed from the code*, it is the only family-wise reading.

A whole-genome screen ranks candidates; it does not promote them. The question
this run answers is the one #2266 never got to ask: **does the flag improve the
deployed native genome when it is the only changed arm?**

## How it was measured

1,200 map pairs, six-player Pangaea foldover, Online speed, 250 turns, six
city-states, every major treated with civilizations shuffled per map. The
ledger's `best` genome held every other native treatment at its deployment
state; the two arms differ only in `holy-lane-parity`. Seeds 110,000,000–
110,001,199 are disjoint from every screen in the ledger, and from P10's
100,000,000–100,002,962 in particular, so this is an independent sample and not
a re-read of the same maps.

```bash
target/ci/gene_screen \
  --genes holy-lane-parity --baseline best --field advanced --design foldover \
  --players 6 --city-states 6 --all-seats --randomize-civs \
  --pairs 1200 --start-seed 110000000 --jobs 12 \
  --out ~/civvis-gene-screens/holy-lane-parity-direct.jsonl
```

All 2,400 games completed — nothing was excluded and the run was not stopped
early. That is 7,200 treated-seat pairs. The binary was
`fddc10bd6f93e28571086b31f4a3591663fe69e074e4ba39c3cc4ab72a6416a5`; rebuilt from
this branch it replays seeds 110000000–110000002 to 36 byte-identical seat rows
across both arms, so the result reproduces from the committed source.

## What it measured

| axis | on | off | paired Δ |
|---|---:|---:|---|
| wins | **17.65%** | 15.68% | **+1.97 pp ± 0.49**, z **+4.05** |
| score share | — | — | +0.08 pp ± 0.06, z +1.23 |

The analysis records a paired share Δ rather than per-arm share rates; the
treated seat's overall share is 16.7% against an equal share of 16.7%.

As the ranking's column: **+99 wins per 10,000**, standard error 24.3, 95% CI
**[+51, +147]**. This run resolves ±68 at 80% power, so the reading clears its
own band by half as much again. `HELPS **`.

**Two independent instruments now agree.** P10 randomized 75 genes over seeds
100M and read +63 [CI +27, +99]; this run randomized one over seeds 110M and
reads +99 [CI +51, +147]. Different estimands — P10 averages over random
opposing genomes, this one measures the flag inside the genome that ships — and
the intervals overlap across most of their length.

**Wins move; score share does not.** +0.08 pp on share (z +1.23) is a null. The
gene converts games rather than accumulating score, which is consistent with its
mechanism: it does not make the empire bigger, it makes a Religion empire reach
its own victory condition sooner.

**The regime explains the size.** In this profile religion ends **39%** of games
(median turn 170) and 4,710 games are lost to a rival's faith — 4.4 of 5.1
cities flipped, 571 faith still banked, and 40% of those seats had founded a
religion of their own. A
Religion empire was pricing its own district at `(Religion, holy_site)` **210**
while a Culture empire pays `(Culture, theater_square)` **850** for its own —
a quarter — in the lane that decides two games in five.

**Cost is nil.** Compute +0.49% ± 0.31% per turn, whole-game time +1.07% ± 0.70%.

## What this does not settle

- **850 is an upper bound, not a proposal.** It is `(Culture, theater_square)`'s
  own figure, taken because it is the largest own-lane value in
  `strategic_family`, so that a null would retire the axis rather than leave
  "maybe a smaller number would have worked". It is not null, so the axis is
  open and the number is now the obvious thing to tune. This run says the
  direction pays; it does not say 850 is where to stop.
- **The historical war regime disagrees on the share axis.** In
  `2026-08-21-s8-war-rerank-vs-best-4p-allseats` (4p, `domination,score`, 5,844
  pairs) the gene reads +5 on wins but **share hurts, z −2.26**. The one-screen
  ledger intentionally excludes that four-player regime, so it does not set a
  current `conflict` field or a deployment column. Religion cannot win in that
  regime; keep it as context rather than pooling it with the six-player history.

## What was decided

The gene is back in the code, and **it defaults on.** The operator took the
call this page was written to leave open (2026-08-22): *"sure we can have this
default on."*

Recording this direct 60×38 Pangaea result as a **legacy** ledger source gives
the gene columns `[+63 prior, +99 last]` — both positive — and the 2026-08-22
rule defaults it **on**. The ledger diff is exactly one row changed and exactly
one default moved: `holy-lane-parity` `false` → `true`, `default_on` **33 →
34**. No other gene's verdict, columns or default moved.

⚠ **This moves the incumbent.** Every recorded Elo result is filed against the
deployment genome, and the genome now plays a gene it did not play before, so
`ai_eval live advanced --deployment-comparison` diverges from `main` by
design — the previous PR's byte-identical run was the evidence that *restoring*
the code changed nothing, and that property is deliberately given up here.

Its `helps` verdict is family-wise on the direct win axis. The historical war
screen's `share hurts` reading is not part of the one-screen ledger; the rule
reads the two positive six-player win columns.

## What is still open

- **850 is an upper bound, not a tuned value.** It is
  `(Culture, theater_square)`'s own figure, taken so a null would retire the
  axis rather than leave "maybe a smaller number would have worked". It is not
  null, so the number is now the obvious thing to screen — `advanced_holy_lane_v0`
  (the pre-shipment `d_holy`) is the second cell of that 2×2 and is registered.
- **The historical war regime.** A screen restricted to `domination,score` is a
  game religion cannot win, so it cannot replace the standard screen. If it
  re-runs against the current genome, its share reading is diagnostic history,
  not a deployment input.
