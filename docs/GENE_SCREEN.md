# Gene screen: pricing every treatment flag from one batch of games

`gene_screen` (`src/bin/gene_screen.rs`) treats every boolean treatment flag on
the Advanced controller as a **gene** — on or off — and prices all of them from
ONE batch of games instead of one arm per flag. It answers, per gene and with
an interval, the question every treatment PR ends on: *does the agent win more
with this on than off?*

```sh
cargo build --profile ci --bin gene_screen
target/ci/gene_screen --list                                # the genes, in bit order (64 on 2026-08-19)
target/ci/gene_screen --pairs 300 --anchor-pairs 20 --jobs 8 --out screen.jsonl
target/ci/gene_screen --analyze screen.jsonl [more.jsonl ...]  # re-read, merge, re-table
```

## ⚠ Overlap with `treatment_lottery`, and what should happen about it

`src/bin/treatment_lottery.rs` + `docs/TREATMENT_LOTTERY.md` landed on `main`
while this was in review, from another session, with **the same goal**: draw a
random withhold-vector per game, average marginally, price every treatment at
once, keep a per-game JSONL ledger. Neither knew about the other. This is
recorded here rather than quietly shipped beside it, because two tools for one
job is exactly the duplication `AGENTS.md` guards against.

**The substantive difference is the control arm.**

| | `treatment_lottery` | `gene_screen` |
|---|---|---|
| arm 2 of a pair | the **full bundle**, same seed and seat | the **exact complement genome**, same seed and seat |
| balance | in expectation, over the batch | **exact, inside every single pair** — each factor is on in exactly one arm |
| interactions | not recoverable | **free**, from the pair sums (see above) |
| outcomes | delta on win / score share | both, with 95% intervals, an MDE-at-80%-power line, and a family-wise bar |
| extras | fires-check (`moved`) | anchors, regime census, religion instrumentation, `--victories`, `--randomize-civs`, OLS over the sign matrix |

A foldover is the classical refinement of exactly this design, and the exact
per-pair balance is not a nicety: it is what removes the drawn vector's chance
imbalance from every factor's estimate, and it is the *reason* the interactions
fall out of the sums for free.

**Recommendation: one tool, not two.** The cheapest consolidation is to move the
complement arm into `treatment_lottery` — it is a one-line change to what its
control plays — and then port the analysis layer (intervals, resolution line,
interactions, anchors, census) onto its ledger. Whichever name survives is a
call for whoever owns the eval lane, not for either author to make unilaterally;
`AGENTS.md` says a semantic conflict goes to coordination rather than to a
silent resolution. Until then this file and `docs/TREATMENT_LOTTERY.md` should
both point at each other.

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
| `read` | `helps *`/`hurts *` at \|z\| ≥ 2, `HELPS **`/`HURTS **` past the family-wise 5% bar — on the win Δ first, then `share …` when the score-share z says more; `~` otherwise |

The header lines carry the treated seat's overall win rate against chance
(1/players), **how the games ended** (victory type, count, median turn — the
regime the table was measured in), the anchors if any were played, and a
**resolution line**: how many genes, the smallest win Δ and share Δ this run
resolves at 80% power, how many `*` rows |z| ≥ 2 flags by chance alone (≈ 2.6
of 57), and the family-wise bar (≈ |z| ≥ 3.33 for 57 genes).

⚠ The table is sorted by the win z, and on the first run every result past
the family-wise bar was on the **share** axis (`governor-every-lane` share
z −7.3 with a win Δ of −0.4 pp). Read the `read` column, not just the top of
the sort.

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
- `civvis::ai::PRODUCTION_OPT_INS` — off-by-default arms (`wonder-prereq-reach`,
  `apostle-promotion-by-role`, `joint-tactics`, `arrival-waves`); the gene *on*
  means enabling it. `joint-tactics` is the one `FIRAXIS_ONLY` tag that is not
  host-only at all — `advanced_joint_tactics` is production plus that flag, and
  `docs/TACTICS.md` §6 left its whole-game effect inconclusive — so it is
  listed as an opt-in and screened like one. `arrival-waves` is item 4 of
  `docs/LIVE_TACTICS.md` (§10), off everywhere until screened.

A treatment added to any of those tables reaches the genome without touching
`gene_screen.rs`; an engine repair with no `LIVE_TREATMENTS` row is a panic,
not a silent omission.

| flag | values | meaning |
|---|---|---|
| `--genes a,b,c` | tags or field names | screen only these; the rest are held at the baseline |
| `--baseline` | `repairs` (default) / `stock` | what un-screened engine repairs are held at: on (the `advanced_synergy` bundle) or off (production `advanced`) |
| `--field` | `advanced` (default) / `repairs` | the other majors: production `advanced`, or the native repair bundle |
| `--victories a,b,…` | all six by default | restrict the victory lanes, because **the regime decides which genes can act at all**. `--victories domination,score` gives the 31 war and siege genes a game that does not end by conversion at turn 149. Same spelling and same parser as `civvis --victories` |
| `--randomize-civs` | off by default | shuffle every seat's civilization per map. Stock seating is a FIXED civ per seat (Rome, Egypt, Greece, China, …), and on the first 250-pair run seats 0 and 2 won twice as often as seat 3 whoever sat there. The foldover cancels that for every per-gene contrast (both arms share the seat); the *field* is the same three civs every game unless this is on |
| `--all-seats` | off by default | **every major seat is its own test**: each draws its own genome (seat `s` from the seed stream at `pair·players + s`), and arm 2 complements *every* seat — so each gene is still on in exactly one arm of every seat's pair, and one game yields `players` observations instead of one. Outcomes within a game share a single winner, so the analysis **clusters by game pair** (`clustered_mean_se`; sandwich errors on the adjusted column) — the gain is real but less than ×players on the win axis. The field is the other treated majors: effects are averaged over random opposing genomes rather than against a fixed production field, which is a different (and more ecological) estimand — `--field` shapes only the anchors, which keep the classic single treated seat (an all-on-vs-all-off contrast where every seat flips is symmetric and measures nothing). Files record `all_seats` in the header and refuse to merge across modes |

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

## Per-civilization effects — `--by-civ <tag>`

Rows carry the seat's civilization (both arms of a pair share it — the roster
shuffle is seeded by the map seed), and `--analyze … --by-civ war-economy`
prints that one gene's paired contrast split by civ, clustered like everything
else. This is the subgroup the marginal table averages away: a flag can be
worth nothing on average and still be a real strategy for one civilization —
or the reverse. It is a subgroup scan with its own family-wise bar printed in
the header; treat a flag as where to point a run, not a finding. `--all-seats`
with `--randomize-civs` is what gives every civ enough labelled pairs for the
split to resolve anything.

## Interactions — the other half of every pair

```sh
target/ci/gene_screen --analyze screen.jsonl --interactions --top 20
```

A foldover splits the evidence in two, and the main table uses one half. Write
the outcome as `y = μ + Σβᵢxᵢ + Σγᵢⱼxᵢxⱼ` with `x ∈ {−1,+1}`; the second arm is
the exact complement, so every `xᵢ` flips:

- the **difference** `y(g) − y(ḡ)` keeps `2βᵢxᵢ` and **cancels every two-factor
  term** (`xᵢxⱼ − (−xᵢ)(−xⱼ) = 0`). That cancellation is why the main-effect
  table is clean — de-aliasing main effects from interactions is the classical
  reason to run a foldover at all.
- the **sum** `y(g) + y(ḡ)` cancels every main effect and keeps `2γᵢⱼxᵢxⱼ`.

So the interactions were never missing from these games; they sat in the half of
each pair the difference throws away, and reading them needs **no game
replayed**. Each `γᵢⱼ` is estimated marginally (57 genes have 1,596 two-factor
terms; no affordable run fits them jointly), and the printed figure is **how
much more one gene is worth when the other is on**.

⚠ The headline is a **count against an expectation**, not the top rows. 1,596
tests throw ~73 flags at |z| ≥ 2 with nothing whatever going on. The first
297-pair run printed *72 against 73 expected, 0 past the family-wise bar* — the
layer was indistinguishable from noise, and the tool says so in those words.
Two consequences worth keeping: at this size **no pairwise coupling among the
repairs is visible**, and any table of "top interactions" printed without that
line would read as a dozen findings every time it ran.

⚠ The map effect does **not** cancel in the sum the way it does in the
difference (a pair's sum is twice its map's difficulty plus the interaction
terms), so interactions are far noisier than main effects from the same run.

## Instrumentation: how a game was lost, not only that it was

Every row also carries `founded_religion`, `foreign_faith_cities` (our own
cities flying somebody else's faith at the end), `faith` still banked,
`inquisition` (whether the Inquisitor gate was ever unlocked), `techs` and
`military`. The table prints a **religion census** from them, because two
thirds of native games end by conversion and the rows previously could not say
one thing about how the losing seat stood in that race — including the
diagnostic split over the games actually lost to a rival's religion.

## What the first run taught (2026-08-19, 4p 60×38 Online-250, 300 pairs + 20 anchors)

Recorded in full in
`docs/eval/2026-08-19-gene-screen-random-genome-factorial-screen.md`. The parts
that change how the tool is read:

- **Native 4p games are a religion race.** 65% ended by conversion, median
  t148, a third before t150. The 31 war/siege genes sit at ~0 win Δ because the
  game is over before a siege matters — a fact about the regime, not a
  measurement of the repairs. Use `--victories` to give them a game.
- **Score share carries the signal; win rate barely moves.** ±1.50 pp against
  ±7.0 pp from identical games. All three results past the family-wise bar were
  on the share axis and two were invisible on the win axis
  (`governor-every-lane` −4.02 pp at z −8.34 with a win Δ of −1.3 pp).
- **The bundle buys cities it does not convert.** All-on against all-off over
  20 anchor pairs: **+3.45 cities (z +7.0)**, wins −20 pp but with an interval
  spanning zero. `wide-map-capacity` alone shows the same shape — +2.89 pp
  share at z +5.7, no win gain.
- **The interaction layer was noise**: 72 flags at |z| ≥ 2 against 73 expected.
- **Fixed seating is a confound for the field.** See `--randomize-civs`.
- The screen reproduced a known result from a new instrument:
  `governor-every-lane` here, against `advanced_every_lane` at −62 Elo compact /
  −95 deployment over 400 pairs per gate (PR #1955).

## What it is not

- Not `gene_census`, which asks whether a continuous `Weights` gene moves an
  outcome at all. The genes here are the boolean treatment flags.
- Not a promotion gate. `ai_eval --matrix` and the Elo ledgers stay the
  authority on whether an arm ships; a screen's `*` is where to point one.
- Not the deployment regime. Firaxis-only flags are excluded by construction;
  the live ladder (`docs/CIV6_LADDER.md`) prices those.
