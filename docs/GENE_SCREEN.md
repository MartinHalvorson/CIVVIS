# Gene screen: pricing every treatment flag from one batch of games

## The genome doctrine (operator, 2026-08-20)

The controller is treated as a **genome**: every feature is a gene, and genes
are tested **regularly**, not once at birth. The instrument is this screen —
very large randomized runs whose aggregate is the signal: each game seats
genomes drawn at random, so every OTHER gene differs from game to game and
averages out of every gene's own contrast (and the foldover makes that exact
per pair, not just in expectation). With `--all-seats`, **every player is a
test**, so one batch prices the whole genome at once.

The standing cadence this implies:

1. **Screen** the full genome after each batch of landed treatments — a
   gene's price is not a constant of nature (`wide-map-capacity` measured
   **−3.4 pp** wins with all six lanes live and **+19.2 pp** with only
   `domination,score`, from the same code; seeds 40000000../41000000..). That
   spread is why there is now exactly ONE screen: a column only means something
   against a fixed world.
2. **Repair** what measurably hurts, giving the gate back the premise it
   claimed (see the 2026-08-19 round: six repairs, each doc-commented with
   the number that motivated it).
3. **Re-screen the repaired genes on disjoint seeds** before believing the
   fix. A repair is a hypothesis until the screen says otherwise.
4. The matrix gate (`ai_eval`) remains the SHIP decision for promotions;
   the screen ranks and directs, at two orders of magnitude less cost per
   gene.

## ⭐ ONE SCREEN (operator, 2026-08-22)

*"lets drop all the native screen / war screen stuff. we should just have 1
screen - 6p continents map for now. keep it simple. each civ gets a separate
genome and we measure the win rates across for each gene across many games when
gene is on and when gene is off."*

| leg | value |
|---|---|
| majors | **6**, each carrying its own drawn genome (`--all-seats`, now the default) |
| map | **continents**, **74×46**, **9 city-states** — Civilization VI's own six-player row (`CIV6_MAP_SIZES` "small": 580 tiles and 1.5 city-states per major, three continents) |
| speed | **Online**, its own **250-turn** clock; a game that reaches it is a score victory, not a truncation |
| lanes | **all six**; no restricted-lane regime |
| design | foldover against the **best-genome** baseline, civs shuffled per map |

`gene_screen --pairs N --out rows.jsonl` — no profile flags — *is* the screen.
Every profile flag still exists, and every one of them turns a batch into a
**probe**: `tools/gene_ledger.py` refuses a source whose header does not match
this table, so the ledger cannot quietly hold two worlds in one column. The
shape lives in `SCREEN_PLAYERS`/`SCREEN_MAP`/… in `src/bin/gene_screen.rs` and
in `SCREEN` in `tools/gene_ledger.py`, and a test fails if the two drift apart.

**What this replaced.** Two regimes: `native` (all six lanes, six players) and
`war` (`--victories domination,score`, four players), the second one added
because the 31 war and siege genes were being asked what they contribute to a
game that ended by conversion at turn 149. Their columns were never comparable
— a four-player seat wins 1-in-4 by chance against 1-in-6 — and the war regime
never entered a default, so its rows are gone rather than carried. The war
question is answered by the map instead: see below.

### Why continents

Measured on the same seeds (140000000+, 60×38, six city-states, one screened
gene so the census reads the baseline genome, on `d713b019`):

| ending | pangaea 250 | continents 250 |
|---|---:|---:|
| religious | 48% (t174) | **28%** (t181) |
| score (at the clock) | 38% | **52%** |
| culture | 11% | **18%** |
| science | 2% | 1% |
| diplomatic | 1% | 0% |
| domination | 0% | 0% |

Pangaea was a religion race that decided half its games by conversion before
anything else could pay. Continents halves that grip and doubles the games
decided on score, for **+11.9% wall per game** (34.5 → 38.6 s at that size).
That is a real de-biasing of the instrument at a price worth paying.

⚠ Those numbers are from a 60×38 six-city-state probe. **At the screen's own
74×46 nine-city-state shape the de-biasing is much larger**, measured on this
branch (seeds 170000000+, 12 pairs = 24 games, 98 genes screened):

| ending | pangaea 60×38 | continents 60×38 | **the screen (continents 74×46)** |
|---|---:|---:|---:|
| score (at the clock) | 38% | 52% | **75%** (median t250) |
| culture | 11% | 18% | **17%** (median t221) |
| religious | 48% | 28% | **8%** (median t203) |

Room is what breaks a conversion race: on the six-player map Civ VI actually
uses for six players, the faith that swept 48% of Pangaea games takes 8%. ⚠ That
last column is **24 games**, a pilot reading and not a census — it fixes the
direction and the order of magnitude, not the percentages.

**Cost at the screen's shape: ~52 CPU-seconds per game** (24 games, 1,244 CPU-s,
98 genes screened, `--jobs 10`), against 38.0 s at 60×38. So a 10,000-game
batch is **~144 CPU-hours** — about 15 hours of wall clock at ten jobs. Budget
from this number, not from the older 60×38 ones.

⚠⚠ Science and diplomatic victories land at median **t283 and t285** — past the
standard clock — so at 250 turns they are 1–2% of endings. A science or congress
gene therefore **cannot pay through the win axis**: the seat it would have
carried to a science victory shows up as a score win or a score loss instead.
Those genes pay through **score share**, and the deployment rule reads the win
axis only. Read a lane gene's share column and its `share helps` verdict before
calling it inert.

### One gene is held out of the default screened set

`HELD_UNLESS_ASKED` in `gene_screen.rs` is `["joint-tactics"]`, and it is a
**cost** list, not a verdict. That gene costs **+27.3% ± 0.5% compute per
enabled major seat** (P10, 17,574 seat pairs; a direct 162-pair screen at seeds
120M read +22.5% ± 2.0%) where every other gene in P10 is inside ±1.6% and all
74 together sum to +6.0%. End to end, on the same 20 seed pairs: the full genome
runs **95.8 s/game**, and dropping this one gene runs **38.0 s/game** — a 10,000
game screen goes from **22.2 hours to 8.8**.

⚠ That is a cost argument and not a claim that the gene does nothing. Its win
columns are +3 / −4 — inside any band this instrument has printed — but P10
reads it `share HELPS **` at **share z +3.84**, past that screen's family-wise
bar of 3.403, the strongest share reading among the default-off genes. The
deployment rule reads the **win** axis only, so no number of screens can turn
this gene on through that share reading; what it is owed is a deliberate arm
(`--genes joint-tactics`), not a seat on every batch at 2.5× the bill. It is
default-off today, so holding it out changes nothing the agent plays.
⚠ Under `--design prior` a `helps` gene draws on for 90% of seats, and this one
is `helps` on its share axis — check the `prior:` column in `--list` before
running a prior-weighted screen that carries it.

Today's genome is the boolean treatment flags below. The growth direction is
"hundreds of genes": the remaining `enable_*`/`disable_*` toggle pairs in
`treatment_flags.rs` (182 exist), and — for the continuous `Weights` — the
binarized genes `gene_census` already studies.

`gene_screen` (`src/bin/gene_screen.rs`) treats every boolean treatment flag on
the Advanced controller as a **gene** — on or off — and prices all of them from
ONE batch of games instead of one arm per flag. It answers, per gene and with
an interval, the question every treatment PR ends on: *does the agent win more
with this on than off?*

```sh
cargo build --profile ci --bin gene_screen
target/ci/gene_screen --list                                # the genes, in bit order (64 on 2026-08-19)
target/ci/gene_screen --pairs 300 --anchor-pairs 20 --jobs 8 --out screen.jsonl
                                                            # ↑ THE screen: no profile flags
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
gene                 pairs   on%   off%   latest 10k    prior 10k   earlier 10k  all 95% CI       z   shareΔ     z    adjΔpp   read
muster-at-command-…  30000  31.0%  22.3% +8.9pp z+2.53 +8.4pp z+2.40 +8.7pp z+2.51 [ +6.0,+11.4] +6.28  +2.10pp +3.12  +8.1±1.4  helps **
```

| column | meaning |
|---|---|
| `on%` / `off%` | the treated seat's win rate (any victory) over the pairs where this gene was on / off — the same paired maps in both columns |
| `latest 10k` / `prior 10k` / `earlier 10k` | three newest-first, non-overlapping chronological replications. Each cell is that window's win `Δpp` / paired `z`; `—` means the file has not accumulated that window yet |
| `all 95% CI`, `z` | the pooled on − off estimate from every complete pair. `on% − off%` is the same pooled win `Δpp` |
| `shareΔ`, `z` | the same contrast on **score share** (treated score ÷ all majors' scores): continuous, so it resolves an edge at a fraction of the games a win count needs |
| `adjΔpp` | the win Δ from an OLS of every pair's difference on the whole ±1 sign matrix at once, so a gene is not credited with its neighbours' chance imbalance; printed once there are at least `2·genes+10` pairs |
| `read` | `helps *`/`hurts *` at \|z\| ≥ 2, `HELPS **`/`HURTS **` past the family-wise 5% bar — on the win Δ first, then `share …` when the score-share z says more; `~` otherwise |

The windows count **complete paired comparisons**, not raw arm rows. In an
`--all-seats` screen all seat pairs from one map remain together, because they
share a winner; therefore a nominal 10,000-pair boundary may be 10,002 (or a
smaller final window). This preserves the independence of the three
replications. The header prints each actual count. A pooled flag remains a
screening result; consistent direction across complete chronological windows
is the extra evidence to use before dropping a gene or changing the ledger.

The header lines carry the treated seat's overall win rate against chance
(1/players), **how the games ended** (victory type, count, median turn — the
world the table was measured in), the anchors if any were played, and a
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
- `civvis::ai::PRODUCTION_OPT_INS` — off-by-default arms
  (`apostle-promotion-by-role`, `joint-tactics`, …); the gene *on*
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
| `--baseline` | `best` (default) / `repairs` / `stock` | what un-screened genes are held at: the deployment genome (the ledger's defaults — see below), the genome's universe (every repair on), or production `advanced` (every repair off) |
| `--design` | `foldover` (default) / `prior` | how genomes are drawn: the balanced foldover above, or each arm independently from the ledger's prior (see *Prior-weighted screens* below) |
| `--p-helps`, `--p-hurts`, `--p-unresolved` | 0.9 / 0.1 / 0.5 | the on-probability a gene draws under `--design prior`, by its ledger verdict |
| `--field` | `advanced` (default) / `repairs` | the other majors: production `advanced`, or the native repair bundle |
| `--victories a,b,…` | **all six, and a batch that changes them is a probe** | restrict the victory lanes. **The lanes decide which genes can act at all**, which is why the screen leaves all six live and reads one world; `--victories domination,score` once gave the 31 war and siege genes a game that did not end by conversion at turn 149, and that second regime is what ONE SCREEN retired. Same spelling and same parser as `civvis --victories` |
| `--stock-civs` | civs are shuffled by default | stop shuffling every seat's civilization per map. Stock seating is a FIXED civ per seat (Rome, Egypt, Greece, China, …), and on the first 250-pair run seats 0 and 2 won twice as often as seat 3 whoever sat there. The foldover cancels that for every per-gene contrast (both arms share the seat); the *field* is the same three civs every game when this is on |
| `--single-seat` | every seat is a test by default | leave the classic one-treated-seat design. With all seats — the screen — **every major seat is its own test**: each draws its own genome (seat `s` from the seed stream at `pair·players + s`), and arm 2 complements *every* seat — so each gene is still on in exactly one arm of every seat's pair, and one game yields `players` observations instead of one. Outcomes within a game share a single winner, so the analysis **clusters by game pair** (`clustered_mean_se`; sandwich errors on the adjusted column) — the gain is real but less than ×players on the win axis. The field is the other treated majors: effects are averaged over random opposing genomes rather than against a fixed production field, which is a different (and more ecological) estimand — `--field` shapes only the anchors, which keep the classic single treated seat (an all-on-vs-all-off contrast where every seat flips is symmetric and measures nothing). Files record `all_seats` in the header and refuse to merge across modes |

## Profile and cost

Defaults: **the screen** — 6 majors, 74×46 continents, 9 city-states, **Online**
speed to its own 250-turn clock. That is Civilization VI's own six-player row
and the deployment shape `docs/EVAL.md` quotes, so the ledger is read from the
games the agent actually plays. Quote no number without its profile — and a
number from a probe is a number about that probe.

The screen's shape is dearer than the 60×38 Pangaea one every recorded source
was played at: ~1.5× the tiles, three more city-states at ≈2.7% per turn each,
and continents at +11.9% per game. Measure a probe at the screen's own shape
before budgeting a batch rather than scaling the old figures. `--jobs` spreads it;
rows are flushed as games finish, so `--analyze` on the file reads a run in
progress, and `--append` with a disjoint `--start-seed` grows a run across
sessions. Genomes are drawn from `(start seed, pair)`, so a run reproduces
exactly and two seed windows draw disjoint genomes.

The same run now prices the runtime cost of every gene without adding a timer
to any heuristic and without replaying a game:

- **compute cost** is the on/off percent change in wall seconds per completed
  turn. It removes a gene's effect on how many turns the game lasts and asks
  whether each simulated turn itself became dearer.
- **time cost** is the on/off percent change in whole-game wall seconds. This is
  the throughput cost an operator pays, including a gene that makes games end
  earlier or later.

Positive costs are slower and negative costs are faster. The analysis takes the
log ratio inside each same-map game pair, regresses it on every randomized gene
at once, and includes an arm-order intercept so machine-load drift cannot ride a
small chance genome imbalance. An all-seats game has one timing, not six: its
per-seat gene signs are summed and that timing enters the fit once, making the
coefficient the incremental cost of enabling the gene for one major. Reported
uncertainty is one HC1 heteroskedasticity-robust standard error, so long and
short games need not have the same timing variance. This paired, scale-free fit
is both more stable than averaging raw seconds and effectively free: `secs`,
`turn`, and the genomes were already in every JSONL row. Old rows with
absent/zero timing remain readable and produce an unknown cost rather than a
false zero.

## The rows file

The first line is a header (`kind: header`, the gene order, the screened set,
the profile); every other line is one game:

```json
{"kind":"game","pair":0,"arm":1,"seed":26081900,"seat":0,"genome":"0010100110…","win":true,
 "winner":0,"victory":"score","turn":250,"score":1067,"score_share":0.4496,"rank":1,
 "cities":11,"alive":true,"secs":116.3}
```

Interactions (epistasis), subgroup tables (by seat, victory type, map), and a
fitted logistic — plus the compute/time cost estimates above — are all
re-analyses of these rows and never need a game replayed. `--analyze` refuses to
merge files written at different profiles or gene orders — a merged table would
mix two experiments.

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
`military`. The table prints a **religion census** from them, because conversion decided two thirds of games
on the Pangaea instrument (28% at the screen's shape) and the rows could not say
one thing about how the losing seat stood in that race — including the
diagnostic split over the games actually lost to a rival's religion.

## What the first run taught (2026-08-19, 4p 60×38 Online-250, 300 pairs + 20 anchors)

Recorded in full in
`docs/eval/2026-08-19-gene-screen-random-genome-factorial-screen.md`. The parts
that change how the tool is read:

- **4p Pangaea games are a religion race.** 65% ended by conversion, median
  t148, a third before t150. The 31 war/siege genes sit at ~0 win Δ because the
  game is over before a siege matters — a fact about the map and the lanes, not
  a measurement of the repairs. This is what the `--victories domination,score`
  regime existed to answer; ONE SCREEN answers it with continents instead, where
  conversion takes 28% of endings and score takes 52%.
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

## The gene ledger: the defaults are the best genome, and the best genome is data

Operator directive 2026-08-20: *let the defaults for the genes reflect our best
genome — unhelpful genes can default off — so our verification games use our
best genome. When we test, still test and try to improve the less helpful
genes.*

Operator directive 2026-08-22, which is now the rule: *genes can default on if
both the last 10k and 10k prior columns are positive, or if the average of the
two columns is >15 and neither is less than −10. If exactly one column is
filled, the gene can default on when that reading is >20; otherwise it defaults
off.*
Those are the two columns `HEURISTIC_GENE_RANKING.md` prints — wins added per
10,000 on-arm seats at the gene's measured on-rate, `(win_on − 1/players) ×
10,000` —
from the latest two screens that priced the gene. The verdicts below
still record what the screen *proved*, and the screen still prints them; they
no longer decide what ships, so a gene can be `helps` and off (its readings do
not clear the rule) or `hurts` and on (its win columns do).

Operator directive 2026-08-22, later the same day: *we should default every gene
to off that has Diff < 0.* *Diff* is the ranking's own column — the pooled on
win rate minus the pooled off win rate in percentage points, over **every**
screen that priced the gene, each weighted by its on-arm seats. It is the **whole**
on−off difference, twice the scale of a win column beside it, and it is now a
veto: a gene whose record is negative defaults off however its two win columns
read. The veto is one-way — a positive record promotes nothing on its own,
because the columns still have to clear their bars — and it is the one clause
that lets a screen older than the last two speak. **31 genes on, was 34**: it
turned off `war-economy` (+38/+8 over a 2026-08-20 reading of −3.84 pp),
`siege-commitment` (+1/+3 over −0.80 pp) and `apostle-promotion-by-role`
(+14/+12 over −0.83 pp). No measurement moved; each of the three carries
positive recent columns over one old screen it has not made back.

Until then "on by default" meant somebody had written `self.enable_x()` into
the bundle, and the phase-1 anchors had measured that all-on bundle at **7.5%
wins against 27% for all-off** (4p classic, 200 anchor pairs). Now:

- **`docs/gene_ledger.json`** records the screen's own profile, then per gene
  what the newest screen measured, the verdict that follows, the two win
  columns (`wins_last_10k`, `wins_prior_10k`) and the `default_on` they decide;
  **`src/ai/advanced/gene_ledger_table.rs`** is the same
  table generated into Rust. `tools/gene_ledger.py --write --source <analysis>
  …` builds both from `gene_screen --analyze --json` outputs (the analyses
  themselves are tracked under `docs/gene_screens/`);
  `tools/test_gene_ledger.py` fails if either file has drifted from the
  recorded sources. Later sources override earlier ones per gene, so a repaired
  gene's re-screen replaces its pre-repair number while the rest of the old
  screen stands. Each source carries the `shape` it was played at: `standard`
  is the screen, `legacy` is a reading kept as history (every source today —
  the Pangaea screens the current defaults stand on). A new source that is not
  `standard` is refused unless `--legacy-shape` says otherwise.
- **Verdict rules** (`tools/gene_ledger.py`, repeated in
  `src/ai/advanced/gene_ledger.rs`): `helps` = win z ≥ 2 with share z > −2, or
  share z ≥ 2 with win z > −2 — the screen's own `*` flag; `hurts` the mirror;
  `unresolved` otherwise, including a gene whose axes disagree past |z| ≥ 2
  (`conflict`) and a gene no screen has measured. Past the family-wise bar is
  recorded as `family_wise`, not required: with sixty-odd genes that bar would
  leave three on. The newest screen that priced the gene supplies the verdict.
- **The deployment rule** (`default_from_columns` in
  `tools/gene_ledger.py`, mirrored as `columns_default_on` in
  `src/ai/advanced/gene_ledger.rs`, and re-derived from the generated table by
  `the_default_follows_the_win_columns`): **on** when both win columns
  are positive, or when their average is above +15 with neither below −10;
  with exactly one populated column, **on** when it is above +20; **off**
  otherwise. That clause is `default_from_win_columns` /
  `win_columns_default_on`, and only the latest two readings reach it. Then
  **off whatever it says when `win_diff_pp` is negative** — the pooled on−off
  difference over the whole record, recorded beside the columns in
  `docs/gene_ledger.json` and in the generated Rust table so the re-derivation
  reads the same number the decision was taken on. Both arms of a screen carry
  the same number of seat observations, so the 1-in-`players` chance base
  cancels inside each screen and the pooled figure is an on-arm-seat-weighted
  average of per-screen differences —
  comparable across shapes and player counts in a way a raw win rate is not.
  ⚠ It has no recency discount: a gene repaired since its worst screen stays
  vetoed until its newer games outweigh that screen's.
- **The deployment genome.** `AdvancedAi::enable_live_bridge` and
  `enable_engine_repairs` now end with `apply_gene_ledger`: every live or
  production treatment the ledger does not default on is withheld, every
  opt-in it defaults on is enabled, and a flag the screen cannot price
  (the Firaxis-only flags) is left as the bundle set it. A **screenable gene
  nobody has screened yet ships off; one screened once ships on only above
  +20.** The
  `_universe` twins (`enable_live_bridge_universe`, `enable_engine_repairs_universe`)
  set every flag and skip the ledger — they are what this screen starts from
  (it then sets each gene to its drawn state) and what the membership tests
  read. The live seat's `genome` event lists `treatments` as deployed and
  `ledger_withheld` beside it; `gene_screen --list` shows each gene's
  universe state, ledger verdict, default and prior.
- **First ledger (2026-08-20)**, from the 6p 60k native screen (13,446
  seat-pairs, `docs/gene_screens/2026-08-20-p4-…`), the 4p war screen (3,300
  seat-pairs, `…p2-war-…`) and the repaired genes' war re-screen (`…p3b-…`):
  **10 help** (on: barbarian-scouts-are-scouts, bounded-recovery,
  buildings-before-projects, garrison-under-fire, loyalty-rate-alarm,
  recorded-tactical-step, siege-muster, siege-tracks-wall, war-reinforcement,
  wide-map-capacity), **11 hurt** (off), **43 unresolved** (off). The live
  bundle therefore plays those ten plus the host-only flags. What the ledger
  bought is measured by this screen's anchors under `--baseline best` — arm 0
  the all-on universe, arm 1 the best genome, same maps: **250 pairs, 4p
  all-six, seeds 52M: all-on 18.4% wins / 20.6% share, best genome 31.2% /
  26.7%, paired win Δ +12.8 pp ± 3.3 for the best genome** (round:
  `docs/eval/2026-08-20-the-bridge-talks-more-than-once-a-turn.md`).

- **The war column, re-priced (2026-08-21).** Every gene's war verdict now
  comes from one screen against the best genome
  (`docs/gene_screens/2026-08-21-s8-war-rerank-vs-best-4p-allseats.json`:
  4p all-seats, `domination,score`, 5,844 seat-pairs, ±2.0 pp), replacing the
  pre-repair p2/p3b rows. It turned on `score-horizon`, `joint-tactics` and
  `blind-objective-strength` (unresolved natively, helps at war) and held
  off `siege-role`, `housing-districts`, `settler-site-agreement`,
  `holy-lane-parity` and `inquisition-on-threat` (unresolved natively, hurt
  at war); `wide-map-capacity` reads **+15.6 pp** at war. A screen played on
  an older build can carry a gene whose code has since been removed; the
  tool now drops those rows and says so, because the Rust table refuses a tag
  the registry does not know.

- **The default rule replaced the verdict (2026-08-22).** No screen was re-run
  and no measurement moved; the ledger simply re-decided every default from
  the two native win columns. **22 genes on, was 20.** On: `religion-sues-peace`
  (+29/+25), `one-launch-pad` (+24/+23), `whole-turn-backtrack-guard` (+23/+39),
  `siege-tracks-wall` (+21/+51), `strategic-wonders` (+21/+21), `strike-opening`
  (+21/+20), `war-patience` (+20/+3), `blind-objective-units` (+4/+6) and
  `war-reinforcement` (−5/+49, average +22). Off: `founder-temple` (+48/–) and
  `idle-faith-patronage` (+23/–), each one native screen short of a prior
  column; `siege-is-progress` (+14/−64), `war-economy` (+8/−192),
  `amenity-project-preemption` (−4/+33), `army-target-weighs-enemy` (−4/−33)
  and `joint-tactics` (−4/−18). Note `war-patience`: it ships with a `hurts`
  verdict, family-wise on the **share** axis at native share z −3.43, because
  the rule reads the win axis only.

- **A strong first reading may ship provisionally (2026-08-22).** After P10
  moved the current genome to 26 defaults, the operator extended the rule:
  exactly one populated win column defaults on when it is above +20. No
  measurement moved. Seven genes switch on, taking the genome to **33**:
  `great-person-housing` (+78/–), `settler-threat-detour` (+50/–),
  `governor-victory-lanes` (+46/–), `settle-sooner` (+41/–),
  `raid-pillage-prizes` (+30/–), `builder-worked-tile-priority` (+24/–) and
  `opportunistic-war` (+23/–). The boundary is strict: +20/– remains off, and
  a second reading replaces this provisional clause with the two-column rule.

- **A negative record vetoes the columns (2026-08-22).** The operator: *we
  should default every gene to off that has Diff < 0.* No screen was re-run and
  no measurement moved; every gene's pooled on−off difference — the ranking's
  *Diff*, already printed there — is now recorded as `win_diff_pp` beside its
  win columns and vetoes the default when it is negative. **31 genes on, was
  34**: off go `war-economy` (columns +38/+8, record −0.78 pp),
  `siege-commitment` (+1/+3, −0.21 pp) and `apostle-promotion-by-role`
  (+14/+12, −0.06 pp). Nothing switched on: the veto is one-way. Each of the
  three is condemned by the same screen, `2026-08-20-p4`, where they read
  −3.84, −0.80 and −0.83 pp; their two newer screens are positive but smaller,
  and the pooled figure weights by games rather than by recency, so a repaired
  gene stays vetoed until its newer games outweigh its worst screen. That is a
  deliberate reversal of "an older bad screen is history, not a veto", which
  was the rule between 2026-08-22's two directives. **This moved the incumbent**
  every Elo result recorded against the 33- and 34-gene genomes is filed
  against. `holy-lane-parity`, promoted by #2307 the same day, is untouched:
  its record is +1.08 pp.

- **One screen, and the war rows are gone (2026-08-22).** The operator's
  directive above collapsed the instrument to a single shape and this ledger to
  a single measurement per gene. `docs/gene_ledger.json` lost its `war` block,
  its `deciding_regime` and its per-source `regime`; it gained the screen's own
  profile and a `shape` on every source. **No win column and no default moved**
  — the war regime never supplied a column — so the deployment genome is the
  same 33 genes. What changed is seven verdicts that had been decided at war and
  now read `unresolved` (`barbarian-scouts-are-scouts`, `blind-objective-strength`,
  `peacetime-deterrence`, `housing-districts`, `housing-research`,
  `inquisition-on-threat`, `settler-site-agreement`) and `holy-lane-parity`,
  whose `conflict` flag was the war screen's `share hurts` disagreeing with the
  native win reading. Verdicts do not decide defaults, so nothing shipped moved
  with them. The four war sources — `p2`, `p3b`, `s3`, `s8` — are dropped from
  the ledger; their analysis files stay under `docs/gene_screens/` as history.
  ⚠ Every remaining source is `legacy`: 60×38 Pangaea. The current defaults
  stand on that instrument until the first standard screen re-prices them.

- **Ten more genes left the code (2026-08-21).** A second application of the
  directive behind the #2235 cull — the bottom of `HEURISTIC_GENE_RANKING.md`
  leaves the repository — removed `holy-lane-parity`, `camp-reach`,
  `wonder-prereq-reach`, `ranged-line-of-sight`, `housing-buildings`,
  `muster-at-command-radius`, `barbarian-walls-one-tier`,
  `idle-walkers-close-the-pipeline`, `suzerain-cards` and `siege-role`, with
  their `live_without_*` arms and the `advanced_holy_lane`,
  `advanced_holy_lane_v0` and `advanced_wonder_reach` arms that set their
  fields. **Every one was already held off by the ledger**, so the deployment
  genome is unchanged and every screen's "off" arm is what now ships; what is
  gone is the ability to turn them on. `holy-lane-parity` and `siege-role`
  are measured `hurts` at war; the other eight sit inside their screen's noise
  band on wins and are recorded as directive removals, not measured harms.
  ⚠ The band quoted at the time, ±110/10k, was the on−off **difference**'s band
  read against a **column**, which is half that difference — see the correction
  below. The eight are inside the corrected ±56 band too, so the reading stands;
  what does not is calling anything up to ±110 noise.
  `recon-flight` followed a day later (#2271) — see below.

- **The noise band was quoted at twice its scale (2026-08-22).** The ranking's
  win columns are `(win_on − chance) × 10,000`, and a foldover puts the on and
  off arms symmetric about chance — so a column is **half** its screen's on−off
  difference and carries half its error. The ±110/10k the ranking header quoted
  (and #2266 used to justify eight removals) is the difference's 80%-power band:
  correct for `win_delta_pp`, twice too wide for the column printed beside it.
  Nothing about a screen changed; the sentence judging it did. The header now
  derives each native screen's own band from that screen's own errors —
  `2026-08-22-p10` resolves **±51**, `2026-08-21-p7` ±56, the single-gene
  `2026-08-21-s7` ±29 — and `tools/heuristic_gene_ranking.py::column_se` owns
  the arithmetic, next to the `wins_per_10k` it halves, so the printed band and
  the printed column cannot drift apart. What this changes in practice: a column
  between ±51 and ±110 used to read as noise and does not. `holy-lane-parity`'s
  +63 in P10 is the live case — see #2299.

- **The removal ledger is priced on native screens again (2026-08-22).** #2235
  deleted its eight genes' rows out of `2026-08-19-p2`, `2026-08-20-p3b` and
  `2026-08-20-p4`; #2266 left its ten "as played", which is the policy. The
  eight rows are restored (306 lines, additions only, every surviving row
  byte-identical), so every gene in *Removed from the code* is now priced on a
  **native** screen at the same 1-in-6 chance base. Before this, `siege-muster`
  had no native row left and was listed at **+5** from a 4-player *war* screen
  where chance is 1-in-4 — a number that read as "removed while helping" and is
  really −26 at p4. `tools/gene_ledger.py` filters unregistered tags, so the
  deployment ledger is byte-identical either way.

- **`holy-lane-parity` defaults ON (2026-08-22).** The operator took the call
  #2299 left open. Its direct Pangaea confirmation enters the ledger as
  **legacy history**, not a second standard-shaped screen; its columns are
  `[+63 prior, +99 last]`, both positive, and the rule defaults it on:
  **34 genes, was 33**. Exactly one ledger row changed and exactly one default
  moved. ⚠ This **moves the incumbent** every recorded Elo result is filed
  against — the deployment genome now plays a gene it did not play, so
  `--deployment-comparison` diverges from the previous head by design. It is
  the first gene to be culled, restored and promoted; the round is
  `docs/eval/2026-08-22-holy-lane-parity-direct-confirmation.md`.

- **`holy-lane-parity` came back, and its direct arm confirms it (2026-08-22).**
  The first gene to return from a cull. #2266 removed it on **−27** from the
  four-gene `s6` screen — whose column band is **±64**, so that was a null and
  not a reading. P10's binary predates the cull by 1h43m, so P10 priced the gene
  after its code was gone: **+63 at z +3.48**, past P10's family-wise bar of
  3.403 and the only such reading among the nineteen genes in the removal
  ledger. #2299 restored the code and ran the arm the cull never got to —
  1,200 map pairs on seeds 110M, every other treatment held at the deployment
  genome, all 2,400 games complete, 7,200 treated-seat pairs:
  **+99 wins/10k, z +4.05, 95% CI [+51, +147]**, `HELPS **`, against a run that
  resolves ±68. Two independent instruments on two disjoint seed windows now
  agree. Score share is null (+0.08 pp, z +1.23) and cost is nil
  (+0.49% ± 0.31% per turn). ⚠ Following the P9 precedent the direct screen was
  initially a note, not a ledger source, so the gene came back into the ranking
  **off**. The operator's later call records it as legacy history: both columns
  are positive and the gene defaults on at **rank 1**, without mislabelling a
  60×38 Pangaea result as the standard screen. `docs/eval/2026-08-22-holy-lane-parity-direct-confirmation.md`
  holds the numbers and the two things it does not settle (850 is an upper
  bound, not a tuned value; the historical war regime reads `share hurts` at z
  −2.26).

- **`recon-flight` leaves production too (2026-08-21).** It was held out of
  the cull above because `promoted_policy_envoy` turns it on, as one leg of
  the recon quartet promoted at +35 Elo (#1923). The operator's reading is the
  right one, and it is this document's own instrument that answers it: a
  screened gene's arms are drawn on a seat built from `AdvancedAi::new()`, so
  **every screen already played "production with it" against "production
  without it"** — 15,000 native seat-pairs and 5,844 war ones, and it read
  negative on all four axes (native −0.52 pp win / −0.03 pp share, war
  −0.55 / −0.24) without ever resolving. `docs/eval/README.md`'s rule finishes
  it: a composite gate licenses the composite, never its parts, and the ledger
  had already unpicked three of the quartet's four legs for deployment. The
  **frozen rating anchor is untouched** — `advanced_v1` is `legacy()`, which
  never routed through `promoted_policy_envoy`, and
  `advanced_v1_plays_the_same_game_it_always_did` stays green. Against that
  anchor over the same 60 maps, production reads +47 Elo-equivalent with the
  flag and **+58 without** — no detectable regression, at a resolution
  (±102) far too coarse to call it a gain.

- **The operator's view of the same seat-pair data** is `HEURISTIC_GENE_RANKING.md`
  at the repository root — every screenable gene ranked by wins added per
  10,000 six-player on-arm seats, each from the latest native screen that measured
  it, with the war figure and the ledger's default beside it. It is generated
  (`python3 tools/heuristic_gene_ranking.py --write`) and
  `tools/test_heuristic_gene_ranking.py` fails when it is older than the
  ledger's sources; regenerate it in the same change that adds a source.

⚠ Two consequences to know. A `live_without_<gene>` arm for a gene the
ledger already holds off is identical to `live` — the screen is that gene's
instrument now. And a treatment PR no longer ships its flag on: it ships it
into the universe, screens it (a few hundred pairs resolve ±3 pp), and the
ledger turns it on when its native win columns clear the rule, provisionally
including a first reading above +20.

## Prior-weighted screens: the helpful genes play most of the time, and are still priced

The foldover gives every gene exactly half its games on. The operator's ask
was different: *in the large batch tests a helpful gene may be activated in
90% of tests; we should still compare the win rate of the 90% vs the 10%, and
for a helpful gene the 90% should win more.* That is `--design prior`:

- Each arm of a pair is drawn **independently** from the prior — a gene on
  with p = 0.9 if the ledger says it helps, 0.1 if it hurts, 0.5 if
  unresolved or unmeasured (`--p-helps/--p-hurts/--p-unresolved` move them).
  Both arms still share the map and the seat, and the genomes still reproduce
  from `(start seed, pair, arm)`. The header records `design` and the
  per-gene `prior`.
- The per-gene **Δ is the marginal on-versus-off contrast** over every game —
  the 90% against the 10% — with errors clustered by game (an all-seats game's
  rows share a winner). The table's count column becomes `on n/off n`, because
  the arms are no longer balanced and the off arm of a helper is small: at
  p = 0.9, pricing a helper to the same resolution needs about 2.8× the games
  of a foldover (1/(0.9·0.1) against 1/(0.5·0.5)). That is the cost of
  playing the best genome most of the time, and the table's resolution line
  says what it bought.
- **`adjΔpp` is the map-paired OLS** on the arms' differences: `y₀ − y₁` on
  `x₀ − x₁ ∈ {−1, 0, +1}`, zero for every gene the two arms agree on, so each
  gene is priced from the pairs that differ on it with the rest of the genome
  differenced out and the map cancelled exactly.
- The foldover stays the default and the instrument of record for a gene's
  first price; the prior design is the *batch* instrument — the large runs
  that play the deployment genome, verify it, and keep pricing the less
  helpful genes at one half.

## A Δ of exactly zero is a gene that never fired, not a null

`step-and-reassess` (2026-08-20, `docs/LIVE_TACTICS.md` §11) first screened
**+0.0 [+0.0, +0.0]** on both axes over 204 pairs: every pair's two games
ended identically. That is not "no effect" — a gene with any reach at all
moves at least the score share of some game — it is the signature of a gene
whose code path is never entered in the regime the screen plays. The cause
was structural: its first cut lived only on the parallel unit planner, and
the only thing that installs a `WorkPool` is the interactive `civvis --jobs`
CLI; every evaluator, `gene_screen` included, and the live decider run units
serially. The repaired gene carries a serial leg and the next 41 pairs already
differed. Read the interval's width before the sign: a zero-width interval is
a fires-check failure, and the fix is in the gene, not in more pairs.

### And since 2026-08-23 a gate says so, instead of four documents

That paragraph was written down here; `treatment_flags.rs` wrote the same
warning from the other direction — an `ENGINE_REPAIR_TREATMENTS` tag whose
enable is missing from the bundle is off in **both** arms, "the two arms play
byte-identical games", and *"three tags reached the tables before this line and
burned 30 games saying nothing"*; `ai_eval`, `battle_bench`, `doctrine_arena`
and `gene_census` each implement a fires-check for their own instrument. What
none of that was, was a **gate**. Nothing stopped a tag reaching the three gene
tables with no evidence it fires, and nothing failed when a committed screen
contained a zero-width row. `competition-victory-points` is in the tables today
and cannot fire at all: `native_competitions` is `false` in `GameOptions` and no
screen sets it.

`tools/gene_fires.py --max 0` is that gate, wired into `cargo-test` beside
`civvis_inert.py --max 0`, which is the ratchet it is modelled on. It discovers
the gene set exactly the way `gene_table()` does — the engine repairs resolved
through `LIVE_TREATMENTS`, plus both production tables, so a new gene reaches
the gate without touching the tool — and reads each gene's own rows out of
`docs/gene_screens/**/*.json`. **A gene is proven when some committed row has a
non-zero paired statistic**, which is a number read out of an artifact rather
than a sentence somebody wrote. The cheapest artifact that carries it is a
single-gene probe, because `--genes <tag>` holds everything else at the
baseline and any divergence between the arms is then that gene and nothing
else:

```bash
target/ci/gene_screen --pairs 3 --jobs 6 --genes <tag> \
  --baseline best --field advanced --design foldover --all-seats \
  --randomize-civs --start-seed <seed> --out target/<tag>.jsonl
target/ci/gene_screen --analyze target/<tag>.jsonl \
  --json docs/gene_screens/fires/<tag>.json
```

Three pairs is enough, because the question is qualitative. ⚠ Look at the score
share as well as wins: `coupled-expansion`'s probe read a win Δ of exactly zero
and a share Δ of +0.29 pp — a gene that fired and did not change who won, which
is firing. Those probes are **not ledger sources**: they set no profile of their
own, they are three pairs, and `tools/gene_ledger.py` takes its sources by name.

A gene that cannot be made to fire takes a waiver in
`tools/gene_fire_waivers.json` with the reason, in the shape
`tools/inert_waivers.json` uses — except that the reason is enforced, and a
waiver goes **stale** the moment its gene is proven or leaves the tables, which
fails the same ratchet. The list can only shrink.

## The toggles no screen can reach, and why each one is not a gene

`docs/EVAL_STATUS.md`'s *Genome coverage* deliberately publishes an over-count:
it makes no attempt to separate a behaviour worth pricing from host-only
plumbing, so the number is a ceiling on the debt rather than a floor. A ceiling
nobody has examined is the same shape as the defect the section exists for, so
since 2026-08-23 every toggle on that list carries a line saying why it is not a
gene, in `docs/genome_reach_debt.json`, and
`tools/test_genome_reach_debt.py` requires the file to cover **exactly** the
computed list — a toggle that gains a gene row must lose its entry, and a new
unreachable toggle must gain one. The residual is examined work.

The count went **165 / 100 reachable / 65 unreachable → 166 / 114 / 52** when
fourteen of them became opt-in genes — eighteen rows were written and the fires
gate above refused four on their own probes. What the remaining 52 are:

| group | n | why not a gene |
|---|---:|---|
| `bundle` | 6 | It turns a group of other genes on. `gene_screen` already builds its treated seat from `enable_engine_repairs_universe`; a row would vary everything inside it and file the sum under one tag. |
| `host-only` | 12 | Cannot fire on a native board, each with its reason already recorded beside its tag in `FIRAXIS_ONLY_TREATMENTS`. `step-and-reassess` is the founding example of the paragraph above. |
| `live-bridge-row` | 16 | **Screenable would mean withheld.** It has a `LIVE_TREATMENTS` row, so a `PRODUCTION_OPT_INS` row flips `ledger_default_on` from `None` to `Some(false)` and `apply_gene_ledger` takes it out of the live bridge. Its host-only classification is an argument about which rivals the weights were bred against, not a claim it cannot fire — so the move it wants is the one `culture-coverage` made, out of `FIRAXIS_ONLY_TREATMENTS` into an `ENGINE_REPAIR_*` half, taken deliberately with the bridge change owned. |
| `production-on` | 6 | Production ships it ON, so its door is `PRODUCTION_TREATMENTS` — and that row is **not** neutral: `apply_gene_ledger` disables every production treatment whose `ledger_default_on` is `Some(false)`, which is exactly a screenable tag with no ledger row. Adding it would switch a shipped behaviour off; `open_water_navy` alone was promoted at +61 Elo-equivalent. It needs its first screen row before it can have a gene row. |
| `configured-on` | 5 | On in `AdvancedAi::configured` but not in `promoted_policy_envoy`, so `production_bundle_rows_are_real` rejects the row as written, and it carries the same hazard as the group above. |
| `infrastructure` | 2 | No decision content: a cache lifetime, and a controller-wide mode that is itself an aggregate over genes that already exist. |
| `already-on` | 1 | `adjacent_camp_clear` is on in `BasicAi::new`, so an opt-in row would be on in both arms and screen as exactly inert. |
| `does-not-fire` | 4 | A gene row was written for it here and `tools/gene_fires.py` refused it: a single-gene probe over 12 map pairs left both arms byte-identical. The row was removed rather than shipped to return a zero-width interval. |

⚠ The `production-on` and `live-bridge-row` rows are the finding worth
carrying out of this: **making a shipped behaviour screenable is not a
measurement-only change.** The ledger's default rule reads "unmeasured ⇒ off",
which is the right rule for a behaviour that was already off and the wrong one
for twenty-two that were on. Twenty-two genes' worth of reach is available for
the price of deciding, per behaviour, whether it is allowed to go off while its
first screen runs — and that is a genome decision for the operator, not a
side effect of a row.

## What it is not

- Not `gene_census`, which asks whether a continuous `Weights` gene moves an
  outcome at all. The genes here are the boolean treatment flags.
- Not a promotion gate. `ai_eval --matrix` and the Elo ledgers stay the
  authority on whether an arm ships; a screen's `*` is where to point one.
- Not the deployment regime. Firaxis-only flags are excluded by construction;
  the live ladder (`docs/CIV6_LADDER.md`) prices those.
