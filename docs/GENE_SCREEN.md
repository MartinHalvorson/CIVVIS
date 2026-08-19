# Gene screen: pricing every treatment flag from one batch of games

`gene_screen` (`src/bin/gene_screen.rs`) treats every boolean treatment flag on
the Advanced controller as a **gene** — on or off — and prices all of them from
ONE batch of games instead of one arm per flag. It answers, per gene and with
an interval, the question every treatment PR ends on: *does the agent win more
with this on than off?*

```sh
cargo build --profile ci --bin gene_screen
target/ci/gene_screen --list                                # the 57 genes, in bit order
target/ci/gene_screen --pairs 300 --anchor-pairs 20 --jobs 8 --out screen.jsonl
target/ci/gene_screen --analyze screen.jsonl [more.jsonl ...]  # re-read, merge, re-table
```

## Why a screen and not fifty-seven arms

The repository's existing instrument is the withholding arm: `live` against
`live_without_<flag>` (`src/elo.rs`), one arm per treatment, forty to two
hundred maps each. Two things about it are expensive:

1. **It prices one gene per batch.** Fifty-seven genes at the 200 maps a
   +40-Elo edge needs (`docs/eval/2026-08-18-a-forty-map-screen-cannot-see-a-forty-elo-change.md`)
   is 11,400 games.
2. **It prices each repair against the all-on background.**
   `AdvancedAi::enable_engine_repairs` says it plainly: the repairs are
   serially coupled — readiness gates the march, the march gates the siege —
   so withholding one from a bundle that keeps the other fifty-six prices a
   link inside an otherwise-whole chain.

The classical answer is a **random-balance / fractional-factorial screen** with
a **foldover**:

- Every game seats one *treated* major whose genome is drawn at random (each
  screened gene on with probability ½), against a stock field.
- Games come in **pairs**: the second game of a pair replays the SAME map seed
  and seat with the **complement** genome. So every gene is on in exactly one
  arm of every pair, the map's own difficulty cancels out of every per-gene
  difference, and the design is balanced by construction rather than by luck.
- Every game informs every gene. `N` pairs give each gene `N` games on and `N`
  off. The per-gene effect is the mean paired difference in the outcome,
  averaged over random backgrounds — the average marginal effect, not the
  all-on-minus-one one.

`N = 300` pairs prices 57 genes with 600 games. The same 600 games spent on
withholding arms would price three.

## What one row of the table means

```
gene                 pairs   on%   off%   Δpp    95% CI       z   shareΔ     z    adjΔpp   read
muster-at-command-…    300  31.0%  22.3%  +8.7 [ +1.9,+15.5] +2.51  +2.10pp +3.12  +8.1±3.5  helps *
```

| column | meaning |
|---|---|
| `on%` / `off%` | the treated seat's win rate (any victory) over the pairs where this gene was on / off — the same 300 maps in both columns |
| `Δpp`, `95% CI`, `z` | on − off in points, from the paired differences (each pair contributes one on-arm and one off-arm) |
| `shareΔ`, `z` | the same contrast on **score share** (treated score ÷ all majors' scores): continuous, so it resolves an edge at a fraction of the games a win count needs |
| `adjΔpp` | the win Δ from an OLS of every pair's difference on the whole ±1 sign matrix at once, so a gene is not credited with its neighbours' chance imbalance; printed once there are at least `2·genes+10` pairs |
| `read` | `helps *`/`hurts *` at \|z\| ≥ 2 on the win Δ; `HELPS **`/`HURTS **` past the family-wise 5% bar; `~` otherwise |

The header lines carry the treated seat's overall win rate against chance
(1/players), the anchors if any were played, and a **resolution line**: how many
genes, the smallest win Δ and share Δ this run resolves at 80% power, how many
`*` rows |z| ≥ 2 flags by chance alone (≈ 2.6 of 57), and the family-wise bar
(≈ |z| ≥ 3.33 for 57 genes).

⚠ **`~` means unresolved at this size, never "no effect."** A screen's job is
to rank and to say what it could see; a `*` is a candidate for a dedicated
arm, not a promotion. Read the resolution line before reading the table.

## Anchors

`--anchor-pairs N` adds `N` pairs whose arms are **all screened genes on** and
**all off**, on the same seeds/seats as fresh maps. They are excluded from the
per-gene estimates and reported separately: the bundle-versus-stock contrast
that the marginal table cannot give (a marginal Δ is an average over half-on
backgrounds; the anchor is the actual all-or-nothing choice).

## Which genes, and against whom

`--list` prints the genome order. It is **discovered from the repository's own
tables**, never listed by hand:

- `civvis::elo::ENGINE_REPAIR_TREATMENTS` — every live-bridge repair that fixes a
  CIVVIS engine defect, i.e. `LIVE_BRIDGE_TREATMENTS` minus
  `FIRAXIS_ONLY_TREATMENTS`. Host-only flags (`land-grab`, `explore-commit`,
  `bank-envoys`, `fog-land-capacity`, …) read the Civ VI mirror's state and are
  inert on a native board; screening them would measure noise and report it as
  noise, so they are excluded rather than measured. Screening the deployment
  bundle in the host regime is a different instrument (`tools/civ6_treatment_census.py`, the ladder).
- `civvis::ai::PRODUCTION_TREATMENTS` — what production itself turns on
  (`strategic-wonders`); on in both baselines.
- `civvis::ai::PRODUCTION_OPT_INS` — off-by-default arms (`wonder-prereq-reach`);
  the gene *on* means enabling it.

A treatment added to any of those tables reaches the genome without touching
`gene_screen.rs`; an engine repair with no `LIVE_TREATMENTS` row is a panic,
not a silent omission.

| flag | values | meaning |
|---|---|---|
| `--genes a,b,c` | tags or field names | screen only these; the rest are held at the baseline |
| `--baseline` | `repairs` (default) / `stock` | what un-screened engine repairs are held at: on (the `advanced_synergy` bundle) or off (production `advanced`) |
| `--field` | `advanced` (default) / `repairs` | the other majors: production `advanced`, or the native repair bundle |

## Profile and cost

Defaults: 4 majors, 60×38 Pangaea, 6 city-states, **Online** speed to its own
250-turn clock (the same 567-tiles-per-player density as the deployment shape,
which is `--players 6 --width 74 --height 46 --city-states 9`). Quote no number
without its profile — `docs/EVAL.md` records why.

A game at the default profile costs about **two CPU-minutes**, so a 300-pair
screen is ~20 CPU-hours: hours on one machine, not minutes. `--jobs` spreads it;
rows are flushed as games finish, so `--analyze` on the file reads a run in
progress, and `--append` with a disjoint `--start-seed` grows a run across
sessions. Genomes are drawn from `(start seed, pair)`, so a run reproduces
exactly and two seed windows draw disjoint genomes.

## The rows file

The first line is a header (`kind: header`, the gene order, the screened set,
the profile); every other line is one game:

```json
{"kind":"game","pair":0,"arm":1,"seed":26081900,"seat":0,"genome":"0010100110…","win":true,
 "winner":0,"victory":"score","turn":250,"score":1067,"score_share":0.4496,"rank":1,
 "cities":11,"alive":true,"secs":116.3}
```

Interactions (epistasis), subgroup tables (by seat, victory type, map), and a
fitted logistic are all re-analyses of these rows and never need a game
replayed. `--analyze` refuses to merge files written at different profiles or
gene orders — a merged table would mix two experiments.

## What it is not

- Not `gene_census`, which asks whether a continuous `Weights` gene moves an
  outcome at all. The genes here are the boolean treatment flags.
- Not a promotion gate. `ai_eval --matrix` and the Elo ledgers stay the
  authority on whether an arm ships; a screen's `*` is where to point one.
- Not the deployment regime. Firaxis-only flags are excluded by construction;
  the live ladder (`docs/CIV6_LADDER.md`) prices those.
